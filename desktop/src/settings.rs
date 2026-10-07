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

/// A library presentation saved independently for each sidebar section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryView {
    Cards,
    #[default]
    Grid,
    Table,
    Board,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct LibraryDisplay {
    pub sort: SortBy,
    pub descending: bool,
    pub group: GroupBy,
    /// Board columns put manually placed games first. When false, Board
    /// follows `sort` fully; manual positions stay saved for later.
    pub board_manual: bool,
    /// Grid cells show the game title under the cover.
    pub grid_title: bool,
    /// Grid cells show status and rating, or the price in Wishlist.
    pub grid_metadata: bool,
}

impl Default for LibraryDisplay {
    /// Manual order is on, so settings from before this field keep the board
    /// that the user arranged.
    fn default() -> Self {
        Self {
            sort: SortBy::default(),
            descending: false,
            group: GroupBy::default(),
            board_manual: true,
            grid_title: true,
            grid_metadata: true,
        }
    }
}

/// One AI provider added on this device. Its API key is in the OS credential
/// store, never here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct AiProvider {
    /// The chosen model name. None until the user picks one.
    pub model: Option<String>,
    /// The server address for a local provider such as Ollama.
    pub endpoint: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub library_display: LibraryDisplay,
    /// Device-local view choice for each library section.
    #[serde(default)]
    pub section_views: BTreeMap<String, LibraryView>,
    pub library_path: Option<PathBuf>,
    /// One device-local timer; never synchronized as Steam playtime.
    #[serde(default)]
    pub active_play: Option<crate::recommendations::ActivePlay>,
    #[serde(default = "system_theme")]
    pub theme: String,
    #[serde(default)]
    pub appearance: Option<crate::appearance::Appearance>,
    #[serde(default)]
    pub omarchy_mode: bool,
    #[serde(default)]
    pub reduce_motion: bool,
    #[serde(default)]
    pub show_hidden_games: bool,
    #[serde(default)]
    pub last_sync: BTreeMap<String, u64>,
    /// Expanded sidebar smart groups on this device. Empty keeps all collapsed.
    #[serde(default)]
    pub smart_groups_open: Vec<crate::smart::SmartKind>,
    /// Profile country found during Steam sync, for store prices.
    #[serde(default)]
    pub detected_country: Option<String>,
    /// The user's choice; it wins over the detected country.
    #[serde(default)]
    pub store_country: Option<String>,
    /// The folder the user selected for data sync. None means sync is off.
    #[serde(default)]
    pub sync_folder: Option<PathBuf>,
    /// This device's name for other devices. None uses the host name.
    #[serde(default)]
    pub device_name: Option<String>,
    /// Best on rules. Shared across devices through data sync.
    #[serde(default)]
    pub best_on_rules: crate::suitability::Rules,
    /// ProtonDB enrichment on this device. Each device downloads its own
    /// data, so this does not sync.
    #[serde(default)]
    pub protondb: bool,
    /// AI providers added on this device, by provider id. Device-local, like
    /// their keys.
    #[serde(default)]
    pub ai_providers: BTreeMap<String, AiProvider>,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde_json::Value>,
}

fn system_theme() -> String {
    "system".into()
}
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

/// Optional isolated storage for native demo checks; normal launches ignore it.
pub fn preview_dir() -> Option<PathBuf> {
    std::env::var_os("GAMESYNC_PREVIEW_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

fn path() -> Result<PathBuf> {
    if let Some(root) = preview_dir() {
        return Ok(root.join("settings.json"));
    }
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
            section_views: Default::default(),
            library_path: None,
            active_play: None,
            theme: system_theme(),
            appearance: None,
            omarchy_mode: false,
            reduce_motion: false,
            show_hidden_games: false,
            last_sync: Default::default(),
            smart_groups_open: Vec::new(),
            detected_country: None,
            store_country: None,
            sync_folder: None,
            device_name: None,
            best_on_rules: Default::default(),
            protondb: false,
            ai_providers: Default::default(),
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
    /// The store country for wishlist prices.
    pub fn price_country(&self) -> String {
        self.store_country
            .clone()
            .or_else(|| self.detected_country.clone())
            .unwrap_or_else(|| crate::prices::DEFAULT_COUNTRY.into())
    }

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
        assert!(!old.omarchy_mode);
        old.appearance = Some(appearance.clone());
        old.omarchy_mode = true;
        let bytes = serde_json::to_vec(&old).unwrap();
        let reopened: Settings = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reopened.appearance(), appearance);
        assert_eq!(reopened.theme, "Tokyo Night");
        assert!(reopened.omarchy_mode);
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
        assert!(old.library_display.board_manual);
        assert!(old.library_display.grid_title && old.library_display.grid_metadata);
        assert!(old.section_views.is_empty());
        assert!(old.ai_providers.is_empty());
        let mut settings = old;
        settings.library_display = LibraryDisplay {
            sort: SortBy::Hours,
            descending: true,
            group: GroupBy::Collections,
            board_manual: false,
            grid_title: false,
            grid_metadata: true,
        };
        settings
            .section_views
            .insert("favorites".into(), LibraryView::Cards);
        let ollama = AiProvider {
            model: Some("llama3.2".into()),
            endpoint: Some("http://localhost:11434".into()),
        };
        settings
            .ai_providers
            .insert("ollama".into(), ollama.clone());
        let saved = serde_json::to_vec(&settings).unwrap();
        let restored: Settings = serde_json::from_slice(&saved).unwrap();
        assert_eq!(restored.library_display, settings.library_display);
        assert_eq!(
            restored.section_views.get("favorites"),
            Some(&LibraryView::Cards)
        );
        assert_eq!(restored.ai_providers.get("ollama"), Some(&ollama));
        assert_eq!(
            restored.extra.get("future_setting"),
            Some(&serde_json::json!(42))
        );
    }
}
