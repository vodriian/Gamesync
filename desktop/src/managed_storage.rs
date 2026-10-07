//! Internal storage stays separate from sample data. Existing libraries are never moved or deleted.
use crate::model::Game;
use anyhow::{Context, Result};
use gamesync_desktop::{
    library::{CollectionDefinition, LibraryStore},
    library_reader::LoadedLibrary,
    record_store::RecordStore,
    records::{GameData, PlatformMinutes, SteamData, SteamMetadata, WishlistEntry},
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
    Ok(data_dir()?.join(if sample {
        "Sample.library"
    } else {
        "Steam.library"
    }))
}

/// Persistent, isolated sample storage for native recommendation review.
pub fn play_now_demo_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("PlayNowDemo.library"))
}

/// Only the dedicated Best on demo resets when opened.
pub fn best_on_demo_path() -> Result<PathBuf> {
    let root = data_dir()?.join("BestOnDemo.library");
    if root.exists() {
        std::fs::remove_dir_all(&root).context("Could not reset the Best on demo")?;
    }
    Ok(root)
}

fn data_dir() -> Result<PathBuf> {
    if let Some(root) = crate::settings::preview_dir() {
        return Ok(root);
    }
    let dirs = directories::ProjectDirs::from("app", "GameSync", "GameSync")
        .context("App storage is unavailable")?;
    Ok(dirs.data_dir().to_owned())
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
            owned: sample
                .record
                .as_ref()
                .and_then(|record| record.game.steam.as_ref())
                .is_some_and(|steam| steam.owned),
            last_played: None,
            platform_minutes: Default::default(),
            wishlist: None,
            metadata: None,
            extra: Default::default(),
        });
        game.personal.status = sample.status.clone();
        game.personal.rating = sample.rating;
        game.personal.favorite = sample.favorite;
        game.personal.tags = sample.tags.clone();
        if let Some(record) = &sample.record {
            game.personal.play_now = record.game.personal.play_now.clone();
            game.personal.hidden = record.game.personal.hidden;
        }
        // Best on demo samples carry a fixture assessment; other samples have none.
        game.suitability = sample
            .record
            .as_ref()
            .and_then(|record| record.game.suitability.clone());
        // A sample without bundled artwork, such as the fictional Best on
        // blocker example, keeps the missing-cover fallback.
        if let Some(bytes) = crate::assets::Assets.load(&sample.cover).ok().flatten() {
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
    wishlist: Option<SampleWish>,
}

#[derive(Deserialize, Clone, Copy)]
struct SampleWish {
    priority: u32,
    #[serde(default)]
    removed: bool,
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
        // Each value fills once: a later sample edit is never overwritten.
        let wish = sample.wishlist.filter(|_| steam.wishlist.is_none());
        if steam.metadata.is_some() && wish.is_none() {
            continue;
        }
        let fill_metadata = steam.metadata.is_none();
        store.update_steam(record.game_id, steam.app_id, |steam| {
            if let Some(wish) = wish {
                steam.wishlist = Some(WishlistEntry {
                    priority: wish.priority,
                    added: now - i64::from(wish.priority + 1) * 7 * 86_400,
                    removed: wish.removed,
                });
            }
            if !fill_metadata {
                return;
            }
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
    fn play_now_samples_are_recommendable_after_storage_loading() -> Result<()> {
        use gamesync_desktop::recommendations::*;
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().join("PlayNow.library");
        let cache = temporary.path().join("cache");
        std::fs::create_dir(&cache)?;
        prepare(&root, &crate::fixtures::play_now_games()?)?;
        let mut reader = LibraryReader::open(&root, &cache)?;
        let mut loaded = reader.refresh()?;
        fill_samples(&root, &loaded)?;
        loaded = reader.refresh()?;
        let candidates: Vec<_> = loaded
            .games
            .iter()
            .map(|record| Candidate {
                record,
                status_eligible: loaded
                    .manifest
                    .definitions
                    .status(&record.game.personal.status)
                    .is_some_and(|status| status.recommendation_eligible),
                installed: None,
                blocked: false,
            })
            .collect();
        let context = Context {
            energy: Effort::Medium,
            ..Default::default()
        };
        let selection = rank(&candidates, &context, 1_000);
        assert!(selection.ranked.len() >= 3);
        assert_eq!(selection.rejected.get(&Rejection::Hidden), Some(&1));
        assert!(selection.ranked.iter().all(|pick| loaded
            .games
            .iter()
            .find(|game| game.game_id == pick.id)
            .unwrap()
            .game
            .steam
            .as_ref()
            .unwrap()
            .owned));
        Ok(())
    }

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
        let wishlisted: Vec<_> = filled
            .games
            .iter()
            .filter(|g| g.game.wishlisted())
            .map(|g| g.game.title.as_str())
            .collect();
        assert_eq!(wishlisted.len(), 3);
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

    #[test]
    fn best_on_demo_store_keeps_assessments_and_saves_preferences() -> Result<()> {
        use gamesync_desktop::suitability::SetupPreference;
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().join("BestOnDemo.library");
        let cache = temporary.path().join("cache");
        std::fs::create_dir(&cache)?;
        let samples = crate::fixtures::best_on_games()?;
        // Includes the fictional blocker example, which has no bundled cover.
        prepare(&root, &samples)?;
        let mut reader = LibraryReader::open(&root, &cache)?;
        let loaded = reader.refresh()?;
        assert_eq!(loaded.games.len(), samples.len());
        assert!(loaded.games.iter().all(|g| g.game.suitability.is_some()));
        let blocked = loaded
            .games
            .iter()
            .find(|g| g.game.title.starts_with("Arena Lab"))
            .unwrap();
        assert!(blocked.game.personal.cover.is_none());

        let mut personal = blocked.game.personal.clone();
        personal.setup_preference = Some(SetupPreference::Pc);
        RecordStore::open(&root)?.edit_personal(blocked.game_id, blocked.revision_id, personal)?;
        let saved = reader.refresh()?;
        let blocked = saved
            .games
            .iter()
            .find(|g| g.game_id == blocked.game_id)
            .unwrap();
        assert_eq!(
            blocked.game.personal.setup_preference,
            Some(SetupPreference::Pc)
        );
        assert!(blocked.game.suitability.is_some());
        Ok(())
    }
}
