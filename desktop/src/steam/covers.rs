//! Cover refresh always downloads anew and publishes a new media path for cache invalidation.
use super::{
    client::{validate_banner, validate_cover},
    SteamClient,
};
use crate::{
    record_store::RecordStore,
    records::{GameRevision, SteamMetadata},
};
use anyhow::{ensure, Context, Result};
use std::{fs, io::Write, path::Path};
use uuid::Uuid;

/// No API key is needed for public Steam artwork. Personal covers retain priority.
pub fn refresh(root: &Path, game_id: Uuid) -> Result<GameRevision> {
    let snapshot = RecordStore::open(root)?.inspect(game_id)?;
    let game = snapshot
        .current()
        .context("Resolve this game's conflict before refreshing its cover")?;
    let app_id = game
        .game
        .steam
        .as_ref()
        .context("This game is not linked to Steam")?
        .app_id;
    let client = SteamClient::new()?;
    let revision = save(root, game_id, app_id, &client.cover(app_id)?)?;
    // The landscape cover is optional; a failure keeps the new portrait.
    match client
        .banner(app_id)
        .and_then(|bytes| save_banner(root, game_id, app_id, &bytes))
    {
        Ok(revision) => Ok(revision),
        Err(error) => {
            log::warn!("Could not refresh the landscape cover: {error:#}");
            Ok(revision)
        }
    }
}

pub(super) fn save(root: &Path, game_id: Uuid, app_id: u32, bytes: &[u8]) -> Result<GameRevision> {
    validate_cover(bytes)?;
    let relative = write_media(root, &format!("steam-{app_id}"), bytes)?;
    RecordStore::open(root)?.update_steam(game_id, app_id, |steam| {
        let metadata = steam.metadata.get_or_insert_with(SteamMetadata::default);
        metadata.cover = Some(relative);
        metadata.cover_complete = true;
    })
}

pub(super) fn save_banner(
    root: &Path,
    game_id: Uuid,
    app_id: u32,
    bytes: &[u8],
) -> Result<GameRevision> {
    validate_banner(bytes)?;
    let relative = write_media(root, &format!("steam-{app_id}-banner"), bytes)?;
    RecordStore::open(root)?.update_steam(game_id, app_id, |steam| {
        steam
            .metadata
            .get_or_insert_with(SteamMetadata::default)
            .banner = Some(relative);
    })
}

/// Write a new, uniquely named file in media/ and return its portable path.
fn write_media(root: &Path, prefix: &str, bytes: &[u8]) -> Result<String> {
    let relative = format!("media/{prefix}-{}.jpg", Uuid::new_v4());
    let media = root.join("media").canonicalize()?;
    ensure!(
        media.starts_with(root.canonicalize()?),
        "Media folder points outside the library"
    );
    let mut staged = tempfile::NamedTempFile::new_in(&media)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    staged
        .persist_noclobber(root.join(&relative))
        .map_err(|error| error.error)?;
    fs::File::open(&media)?.sync_all()?;
    Ok(relative)
}
