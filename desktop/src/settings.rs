//! Eagle's device-local settings pattern, limited to the last opened library.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize, Serialize)]
struct Settings {
    library_path: Option<PathBuf>,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde_json::Value>,
}

fn path() -> Result<PathBuf> {
    Ok(
        directories::ProjectDirs::from("app", "GameSync", "GameSync")
            .context("Settings directory is unavailable")?
            .config_dir()
            .join("settings.json"),
    )
}

fn load() -> Result<Settings> {
    match fs::read(path()?) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(error.into()),
    }
}

pub fn last_library() -> Result<Option<PathBuf>> {
    Ok(load()?.library_path)
}

/// Save after a successful open. Never replace unreadable settings with defaults.
pub fn remember_library(library: &Path) -> Result<()> {
    let mut settings = load()?;
    settings.library_path = Some(library.into());
    let path = path()?;
    let parent = path.parent().context("Settings have no directory")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec_pretty(&settings)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}
