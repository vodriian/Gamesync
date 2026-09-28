//! Internal storage stays separate from sample data. Existing libraries are never moved or deleted.
use crate::model::Game;
use anyhow::{Context, Result};
use gamesync_desktop::{
    library::LibraryStore,
    record_store::RecordStore,
    records::{GameData, SteamData},
};
use gpui::AssetSource;
use std::path::{Path, PathBuf};

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
        assert!(LibraryReader::open(&primary, &cache)?
            .refresh()?
            .games
            .is_empty());
        Ok(())
    }
}
