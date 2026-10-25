//! One identity scan per import session; guarded writes preserve concurrent edits.
use super::*;
use crate::{record_store::RecordStore, records::GameRevision};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
};

#[cfg(test)]
thread_local! { static FULL_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn full_scan_count() -> usize {
    FULL_SCANS.get()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SteamChange {
    New,
    Updated,
    Unchanged,
}
pub(crate) struct SteamImportResult {
    pub record: GameRevision,
    pub change: SteamChange,
}

/// Serialize Steam identity creation without holding the manifest lock during
/// requests. Each record write still reads and validates its current state.
pub(crate) struct SteamImport<'a> {
    library: &'a LibraryStore,
    account: String,
    store: RecordStore,
    identities: BTreeMap<u32, BTreeSet<Uuid>>,
    records: BTreeMap<Uuid, GameRevision>,
    _lock: File,
}

impl LibraryStore {
    /// Bind one account once. Disconnecting a device never removes this association.
    pub fn bind_steam(&self, expected: Uuid, account: &str) -> Result<LibraryRevision> {
        let snapshot = self.inspect()?;
        let current = snapshot
            .current()
            .context("Resolve library definitions before connecting Steam")?;
        ensure!(
            current.revision_id == expected,
            "Library changed. Read it again"
        );
        if let Some(existing) = &current.definitions.steam_account {
            ensure!(
                existing == account,
                "This library belongs to another Steam account"
            );
            return Ok(current.clone());
        }
        let mut definitions = current.definitions.clone();
        definitions.steam_account = Some(account.into());
        self.edit(expected, definitions)
    }

    pub fn import_steam(
        &self,
        account: &str,
        game: &crate::steam::OwnedGame,
    ) -> Result<GameRevision> {
        Ok(self.steam_import(account)?.import_steam(game)?.record)
    }
    pub fn import_wishlist(
        &self,
        account: &str,
        item: &crate::steam::WishlistItem,
        name: &str,
    ) -> Result<GameRevision> {
        Ok(self
            .steam_import(account)?
            .import_wishlist(item, name)?
            .record)
    }
    pub(crate) fn steam_import(&self, account: &str) -> Result<SteamImport<'_>> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(".steam-import.lock"))?;
        lock.try_lock().context("Another Steam import is running")?;
        let files = self.files();
        let _manifest_lock = files.lock()?;
        let manifest = self.inspect()?;
        ensure!(
            manifest
                .current()
                .context("Resolve library definitions before syncing")?
                .definitions
                .steam_account
                .as_deref()
                == Some(account),
            "Steam account does not match this library"
        );
        let store = RecordStore::open(&self.root)?;
        #[cfg(test)]
        FULL_SCANS.set(FULL_SCANS.get() + 1);
        let mut issues = Vec::new();
        let candidates = crate::record_store::discover(&self.root, &mut issues)?;
        ensure!(
            issues.is_empty(),
            "Library files need attention before importing"
        );
        let mut identities = BTreeMap::<u32, BTreeSet<Uuid>>::new();
        let mut records = BTreeMap::new();
        for (id, paths) in candidates {
            let snapshot = store.inspect_candidates(id, &paths)?;
            ensure!(
                snapshot.issues.is_empty(),
                "Unreadable game files prevent safe Steam matching"
            );
            // History and archived/conflicting identities also prevent duplicate creation.
            for record in snapshot.revisions.values() {
                if let Some(steam) = &record.game.steam {
                    identities.entry(steam.app_id).or_default().insert(id);
                }
            }
            if let Some(record) = snapshot.current() {
                records.insert(id, record.clone());
            }
        }
        Ok(SteamImport {
            library: self,
            account: account.into(),
            store,
            identities,
            records,
            _lock: lock,
        })
    }
}

impl SteamImport<'_> {
    pub(crate) fn records(&self) -> Vec<GameRevision> {
        self.records
            .values()
            .filter(|record| record.game.steam.is_some() && !record.deleted)
            .cloned()
            .collect()
    }
    /// Import an owned game. Buying a wishlisted game keeps its record and
    /// personal data; the wishlist value is cleared.
    pub fn import_steam(&mut self, game: &crate::steam::OwnedGame) -> Result<SteamImportResult> {
        let steam = crate::records::SteamData {
            app_id: game.appid,
            description: None,
            playtime_minutes: game.playtime_forever,
            owned: true,
            last_played: game.last_played(),
            platform_minutes: game.platform_minutes(),
            wishlist: None,
            metadata: None,
            extra: Default::default(),
        };
        self.upsert_steam(
            &game.name,
            |definitions| definitions.default_status.clone(),
            steam,
            |steam| {
                steam.playtime_minutes = game.playtime_forever;
                steam.last_played = game.last_played();
                steam.platform_minutes = game.platform_minutes();
                steam.owned = true;
                steam.wishlist = None;
            },
        )
    }

    /// Import a wishlisted game as a record that is not owned. An owned
    /// record wins: its wishlist value stays empty.
    pub fn import_wishlist(
        &mut self,
        item: &crate::steam::WishlistItem,
        name: &str,
    ) -> Result<SteamImportResult> {
        let entry = crate::records::WishlistEntry {
            priority: item.priority,
            added: item.date_added,
            removed: false,
        };
        let steam = crate::records::SteamData {
            app_id: item.appid,
            description: None,
            playtime_minutes: 0,
            owned: false,
            last_played: None,
            platform_minutes: Default::default(),
            wishlist: Some(entry),
            metadata: None,
            extra: Default::default(),
        };
        self.upsert_steam(
            name,
            // Wishlist games start as "Want to play" when the library has it.
            |definitions| {
                definitions
                    .status("wanted")
                    .map_or_else(|| definitions.default_status.clone(), |s| s.key.clone())
            },
            steam,
            |steam| {
                if !steam.owned {
                    steam.wishlist = Some(entry);
                }
            },
        )
    }

    fn upsert_steam(
        &mut self,
        title: &str,
        status: impl FnOnce(&super::LibraryDefinitions) -> String,
        create: crate::records::SteamData,
        update: impl FnOnce(&mut crate::records::SteamData),
    ) -> Result<SteamImportResult> {
        let app_id = create.app_id;
        let files = self.library.files();
        let _lock = files.lock()?;
        let snapshot = self.library.inspect()?;
        let manifest = snapshot
            .current()
            .context("Resolve library definitions before syncing")?;
        ensure!(
            manifest.definitions.steam_account.as_deref() == Some(self.account.as_str()),
            "Steam account does not match this library"
        );
        let ids = self.identities.get(&app_id).cloned().unwrap_or_default();
        ensure!(
            ids.len() <= 1,
            "Multiple games use this Steam App ID. Resolve the duplicate first"
        );
        let (record, change) = if let Some(id) = ids.first() {
            let (record, changed) = self.store.update_steam_with_change(*id, app_id, update)?;
            (
                record,
                if changed {
                    SteamChange::Updated
                } else {
                    SteamChange::Unchanged
                },
            )
        } else {
            let mut data = crate::records::GameData::new(title);
            data.personal.status = status(&manifest.definitions);
            data.steam = Some(create);
            let id = Uuid::new_v5(&manifest.library_id, format!("steam:{app_id}").as_bytes());
            (self.store.create_identified(id, data)?, SteamChange::New)
        };
        self.identities
            .entry(app_id)
            .or_default()
            .insert(record.game_id);
        self.records.insert(record.game_id, record.clone());
        Ok(SteamImportResult { record, change })
    }
}
