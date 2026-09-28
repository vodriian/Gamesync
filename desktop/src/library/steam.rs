//! Account binding and provider imports share the manifest lock.
use super::*;
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

    /// Import an owned game. Buying a wishlisted game keeps its record and
    /// personal data; the wishlist value is cleared.
    pub fn import_steam(
        &self,
        account: &str,
        game: &crate::steam::OwnedGame,
    ) -> Result<crate::records::GameRevision> {
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
            account,
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
        &self,
        account: &str,
        item: &crate::steam::WishlistItem,
        name: &str,
    ) -> Result<crate::records::GameRevision> {
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
            account,
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

    /// Current records that have Steam data. Unreadable or conflicting games
    /// are skipped; callers must not treat a missing game as removed.
    pub fn steam_records(&self) -> Result<Vec<crate::records::GameRevision>> {
        let store = crate::record_store::RecordStore::open(&self.root)?;
        let mut issues = Vec::new();
        let mut records = Vec::new();
        for (id, paths) in crate::record_store::discover(&self.root, &mut issues)? {
            let snapshot = store.inspect_candidates(id, &paths)?;
            if let Some(record) = snapshot.current() {
                if record.game.steam.is_some() && !record.deleted {
                    records.push(record.clone());
                }
            }
        }
        Ok(records)
    }

    /// Serialize provider creation with definition edits and local sync
    /// processes. One Steam App ID maps to one game identity.
    fn upsert_steam(
        &self,
        account: &str,
        title: &str,
        status: impl FnOnce(&super::LibraryDefinitions) -> String,
        create: crate::records::SteamData,
        update: impl FnOnce(&mut crate::records::SteamData),
    ) -> Result<crate::records::GameRevision> {
        let app_id = create.app_id;
        let files = self.files();
        let _lock = files.lock()?;
        let snapshot = self.inspect()?;
        let manifest = snapshot
            .current()
            .context("Resolve library definitions before syncing")?;
        ensure!(
            manifest.definitions.steam_account.as_deref() == Some(account),
            "Steam account does not match this library"
        );
        let store = crate::record_store::RecordStore::open(&self.root)?;
        let mut issues = Vec::new();
        let candidates = crate::record_store::discover(&self.root, &mut issues)?;
        ensure!(
            issues.is_empty(),
            "Library files need attention before importing"
        );
        let mut matches = Vec::new();
        for (id, paths) in candidates {
            let records = store.inspect_candidates(id, &paths)?;
            if records
                .revisions
                .values()
                .any(|r| r.game.steam.as_ref().is_some_and(|s| s.app_id == app_id))
            {
                ensure!(
                    records.issues.is_empty() && records.current().is_some(),
                    "Steam game has conflicting or incomplete versions"
                );
                matches.push(id);
            } else {
                // An unreadable identity could be this game; do not create a duplicate.
                ensure!(
                    records.issues.is_empty(),
                    "Unreadable game files prevent safe Steam matching"
                );
            }
        }
        ensure!(
            matches.len() <= 1,
            "Multiple games use this Steam App ID. Resolve the duplicate first"
        );
        if let Some(id) = matches.first() {
            return store.update_steam(*id, app_id, update);
        }
        let mut data = crate::records::GameData::new(title);
        data.personal.status = status(&manifest.definitions);
        data.steam = Some(create);
        // Same app imported on two devices has one identity, then normal branch review.
        let id = Uuid::new_v5(&manifest.library_id, format!("steam:{app_id}").as_bytes());
        store.create_identified(id, data)
    }
}
