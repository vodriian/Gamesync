//! Eagle's device-local settings pattern, limited to the last opened library.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    #[default]
    Name,
    Status,
    Hours,
    Collection,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    #[default]
    None,
    Status,
    Collections,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct LibraryDisplay {
    pub sort: SortBy,
    pub descending: bool,
    pub group: GroupBy,
}

#[derive(Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub library_display: LibraryDisplay,
    pub library_path: Option<PathBuf>,
    #[serde(default = "system_theme")]
    pub theme: String,
    #[serde(default)]
    pub appearance: Option<crate::appearance::Appearance>,
    #[serde(default)]
    pub reduce_motion: bool,
    #[serde(default)]
    pub show_hidden_games: bool,
    #[serde(default)]
    pub last_sync: BTreeMap<String, u64>,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde_json::Value>,
}

fn system_theme() -> String {
    "system".into()
}
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

fn path() -> Result<PathBuf> {
    Ok(
        directories::ProjectDirs::from("app", "GameSync", "GameSync")
            .context("Settings directory is unavailable")?
            .config_dir()
            .join("settings.json"),
    )
}

pub fn load() -> Result<Settings> {
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
    update(|settings| settings.library_path = Some(library.into()))
}

/// Serialize read-modify-write operations so appearance and sync timestamps cannot race.
pub fn update(change: impl FnOnce(&mut Settings)) -> Result<()> {
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Settings are unavailable"))?;
    let mut settings = load()?;
    change(&mut settings);
    let path = path()?;
    let parent = path.parent().context("Settings have no directory")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec_pretty(&settings)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_display: LibraryDisplay::default(),
            library_path: None,
            theme: system_theme(),
            appearance: None,
            reduce_motion: false,
            show_hidden_games: false,
            last_sync: Default::default(),
            extra: Default::default(),
        }
    }
}

/// Shared footer and Settings wording for the last completed Steam sync.
pub fn sync_label(time: Option<u64>) -> String {
    time.map(|time| {
        let age = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs().saturating_sub(time));
        if age < 60 {
            "Last Steam sync: just now".into()
        } else if age < 3600 {
            format!("Last Steam sync: {} min ago", age / 60)
        } else if age < 86400 {
            format!("Last Steam sync: {} hours ago", age / 3600)
        } else {
            format!("Last Steam sync: {} days ago", age / 86400)
        }
    })
    .unwrap_or_else(|| "No Steam sync time recorded".into())
}

impl Settings {
    pub fn appearance(&self) -> crate::appearance::Appearance {
        self.appearance
            .clone()
            .unwrap_or_else(|| crate::appearance::Appearance::from_legacy(&self.theme))
    }
}

#[cfg(test)]
mod appearance_migration_tests {
    use super::*;
    #[test]
    fn appearance_round_trip_retains_legacy_and_unrelated_settings() {
        let mut old:Settings=serde_json::from_str(r#"{"library_path":null,"theme":"Tokyo Night","reduce_motion":true,"future_setting":{"enabled":true}}"#).unwrap();
        let appearance = old.appearance();
        assert_eq!(appearance.dark_scheme.as_str(), "notion");
        old.appearance = Some(appearance.clone());
        let bytes = serde_json::to_vec(&old).unwrap();
        let reopened: Settings = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reopened.appearance(), appearance);
        assert_eq!(reopened.theme, "Tokyo Night");
        assert!(reopened.reduce_motion);
        assert_eq!(reopened.extra["future_setting"]["enabled"], true);
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    #[test]
    fn display_preferences_round_trip_and_old_settings_keep_defaults() {
        let old: Settings =
            serde_json::from_str(r#"{"library_path":null,"future_setting":42}"#).unwrap();
        assert_eq!(old.library_display, LibraryDisplay::default());
        let mut settings = old;
        settings.library_display = LibraryDisplay {
            sort: SortBy::Hours,
            descending: true,
            group: GroupBy::Collections,
        };
        let saved = serde_json::to_vec(&settings).unwrap();
        let restored: Settings = serde_json::from_slice(&saved).unwrap();
        assert_eq!(restored.library_display, settings.library_display);
        assert_eq!(
            restored.extra.get("future_setting"),
            Some(&serde_json::json!(42))
        );
    }
}
