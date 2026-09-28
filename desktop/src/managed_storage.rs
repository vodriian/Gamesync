//! Internal storage stays separate from sample data. Existing libraries are never moved or deleted.
use crate::model::Game;
use anyhow::{Context, Result};
use gamesync_desktop::{
    library::{CollectionDefinition, LibraryStore},
    library_reader::LoadedLibrary,
    record_store::RecordStore,
    records::{GameData, PlatformMinutes, SteamData, SteamMetadata},
};
use gpui::AssetSource;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub fn path(sample: bool) -> Result<PathBuf> {
    // Retain access to a previously configured library without exposing a picker.
    if !sample {
        if let Some(path) = crate::settings::last_library()?.filter(|path| path.exists()) {
            if LibraryStore::open(&path).is_ok() {
                return Ok(path);
            }
            log::warn!(
                "Previous storage is incomplete; keeping it untouched and using app storage."
            );
        }
    }
    let dirs = directories::ProjectDirs::from("app", "GameSync", "GameSync")
        .context("App storage is unavailable")?;
    Ok(dirs.data_dir().join(if sample {
        "Sample.library"
    } else {
        "Steam.library"
    }))
}

pub fn prepare(root: &Path, samples: &[Game]) -> Result<()> {
    if root.exists() {
        LibraryStore::open(root)?;
        return Ok(());
    }
    let parent = root.parent().context("App storage has no parent")?;
    std::fs::create_dir_all(parent)?;
    // Publish only a complete store. A failed seed cannot leave a half-empty demo.
    let staging = tempfile::tempdir_in(parent)?;
    let staged = staging.path().join("library");
    let library = LibraryStore::create(
        &staged,
        if samples.is_empty() {
            "My games"
        } else {
            "Sample games"
        },
    )?;
    let store = RecordStore::open(library.root())?;
    for sample in samples {
        let mut game = GameData::new(sample.title.clone());
        game.steam = Some(SteamData {
            app_id: u32::try_from(sample.id.as_u128()).context("Invalid sample app ID")?,
            description: Some(sample.description.clone()),
            playtime_minutes: sample.playtime_minutes,
            owned: false,
            last_played: None,
            platform_minutes: Default::default(),
            metadata: None,
            extra: Default::default(),
        });
        game.personal.status = sample.status.clone();
        game.personal.rating = sample.rating;
        game.personal.favorite = sample.favorite;
        game.personal.tags = sample.tags.clone();
        if let Some(bytes) = crate::assets::Assets.load(&sample.cover)? {
            let cover = format!("media/{}.jpg", sample.id);
            std::fs::write(staged.join(&cover), bytes)?;
            game.personal.cover = Some(cover);
        }
        store.create(game)?;
    }
    std::fs::rename(staged, root)?;
    Ok(())
}

/// Demo-only Steam values from `fixtures/games.json`. They give every Home
/// panel and smart group something to show without a Steam account.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SampleSteam {
    genres: Vec<String>,
    tags: Vec<String>,
    last_played_days_ago: Option<i64>,
    platform_minutes: PlatformMinutes,
    collections: Vec<String>,
}

#[derive(Deserialize)]
struct SampleEntry {
    id: u32,
    #[serde(default)]
    sample_steam: SampleSteam,
}

fn sample_values() -> Result<BTreeMap<u32, SampleSteam>> {
    let entries: Vec<SampleEntry> = serde_json::from_str(include_str!("../fixtures/games.json"))
        .context("Could not read the bundled sample values")?;
    Ok(entries
        .into_iter()
        .map(|e| (e.id, e.sample_steam))
        .collect())
}

/// Fill sample values that a sample store does not have yet, including stores
/// created by older versions. Only games without Steam metadata change, and
/// collections are added only to a store that has none, so sample edits stay.
/// Returns true when anything was written.
pub fn fill_samples(root: &Path, loaded: &LoadedLibrary) -> Result<bool> {
    let values = sample_values()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let store = RecordStore::open(root)?;
    let mut changed = false;
    for record in &loaded.games {
        let Some(steam) = &record.game.steam else {
            continue;
        };
        let Some(sample) = values.get(&steam.app_id) else {
            continue;
        };
        if steam.metadata.is_some() {
            continue;
        }
        store.update_steam(record.game_id, steam.app_id, |steam| {
            steam.last_played = sample.last_played_days_ago.map(|days| now - days * 86_400);
            steam.platform_minutes = sample.platform_minutes;
            steam.metadata = Some(SteamMetadata {
                genres: sample.genres.clone(),
                tags: sample.tags.clone(),
                details_complete: true,
                tags_complete: true,
                ..Default::default()
            });
        })?;
        changed = true;
    }
    if !loaded.manifest.definitions.collections.is_empty() {
        return Ok(changed);
    }
    let names: BTreeSet<&String> = values.values().flat_map(|v| &v.collections).collect();
    if names.is_empty() {
        return Ok(changed);
    }
    let ids: BTreeMap<&String, Uuid> = names.into_iter().map(|n| (n, Uuid::new_v4())).collect();
    let mut definitions = loaded.manifest.definitions.clone();
    definitions
        .collections
        .extend(ids.iter().map(|(name, id)| CollectionDefinition {
            id: *id,
            name: (*name).clone(),
            archived: false,
            extra: Default::default(),
        }));
    LibraryStore::open(root)?.edit(loaded.manifest.revision_id, definitions)?;
    for record in &loaded.games {
        let Some(sample) = record
            .game
            .steam
            .as_ref()
            .and_then(|steam| values.get(&steam.app_id))
            .filter(|sample| !sample.collections.is_empty())
        else {
            continue;
        };
        // The Steam fill above may have written a newer revision.
        let snapshot = store.inspect(record.game_id)?;
        let current = snapshot
            .current()
            .context("Sample game has conflicting versions")?;
        let mut personal = current.game.personal.clone();
        personal
            .collections
            .extend(sample.collections.iter().filter_map(|name| ids.get(name)));
        store.edit_personal(record.game_id, current.revision_id, personal)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamesync_desktop::library_reader::LibraryReader;

    #[test]
    fn sample_storage_is_separate_and_reopening_preserves_edits() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let primary = temporary.path().join("Steam.library");
        let samples = temporary.path().join("Sample.library");
        let cache = temporary.path().join("cache");
        std::fs::create_dir(&cache)?;
        prepare(&primary, &[])?;
        prepare(&samples, &crate::fixtures::games()?)?;
        let mut reader = LibraryReader::open(&samples, &cache)?;
        let before = reader.refresh()?;
        assert_eq!(before.games.len(), 12);
        let game = &before.games[0];
        let mut personal = game.game.personal.clone();
        personal.notes = "Keep this sample edit".into();
        RecordStore::open(&samples)?.edit_personal(game.game_id, game.revision_id, personal)?;
        prepare(&samples, &crate::fixtures::games()?)?;
        let after = reader.refresh()?;
        assert_eq!(after.games.len(), 12);
        assert!(after
            .games
            .iter()
            .any(|g| g.game.personal.notes == "Keep this sample edit"));

        // Sample values fill once and keep the sample edit.
        assert!(fill_samples(&samples, &after)?);
        let filled = reader.refresh()?;
        assert!(!fill_samples(&samples, &filled)?);
        assert_eq!(filled.manifest.definitions.collections.len(), 2);
        let balatro = filled
            .games
            .iter()
            .find(|g| g.game.title == "Balatro")
            .unwrap();
        let steam = balatro.game.steam.as_ref().unwrap();
        assert!(steam.last_played.is_some());
        assert_eq!(steam.metadata.as_ref().unwrap().tags[0], "Card Game");
        assert_eq!(balatro.game.personal.collections.len(), 2);
        assert!(filled
            .games
            .iter()
            .any(|g| g.game.personal.notes == "Keep this sample edit"));
        assert!(LibraryReader::open(&primary, &cache)?
            .refresh()?
            .games
            .is_empty());
        Ok(())
    }
}
