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

    /// Serialize provider creation with definition edits and local sync processes.
    pub fn import_steam(
        &self,
        account: &str,
        game: &crate::steam::OwnedGame,
    ) -> Result<crate::records::GameRevision> {
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
            if records.revisions.values().any(|r| {
                r.game
                    .steam
                    .as_ref()
                    .is_some_and(|s| s.app_id == game.appid)
            }) {
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
            return store.update_steam(*id, game.appid, |steam| {
                steam.playtime_minutes = game.playtime_forever;
                steam.owned = true;
            });
        }
        let mut data = crate::records::GameData::new(&game.name);
        data.personal.status = manifest.definitions.default_status.clone();
        data.steam = Some(crate::records::SteamData {
            app_id: game.appid,
            description: None,
            playtime_minutes: game.playtime_forever,
            owned: true,
            metadata: None,
            extra: Default::default(),
        });
        // Same app imported on two devices has one identity, then normal branch review.
        let id = Uuid::new_v5(
            &manifest.library_id,
            format!("steam:{}", game.appid).as_bytes(),
        );
        store.create_identified(id, data)
    }
}
