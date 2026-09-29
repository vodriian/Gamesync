//! Data sync in the running app. The library refresh loop runs one round
//! after each disk read, so every round sees fresh local values.
use anyhow::{Context as _, Result};
use gamesync_desktop::{
    library_reader::LoadedLibrary,
    settings,
    sync::{
        apply_library, apply_settings, project, wall_ms, Conflict, DeviceInfo, FieldKey,
        SyncEngine, Target,
    },
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;

/// One open engine. `closed` stops a round that waits for the lock while
/// the user stops sync, so it cannot save the state again.
pub struct Handle {
    engine: Mutex<SyncEngine>,
    closed: AtomicBool,
}

/// Sync status shared by the refresh loop, Settings, and the sidebar.
#[derive(Default)]
pub struct SyncState {
    pub handle: Option<Arc<Handle>>,
    /// The folder the user selected. Kept while the folder is offline.
    pub folder: Option<PathBuf>,
    pub device: Option<Uuid>,
    pub devices: Vec<(Uuid, DeviceInfo)>,
    pub conflicts: Vec<Conflict>,
    pub last_round_ms: Option<u64>,
    /// Changes received in the last round that received any.
    pub last_received: Option<(u64, usize)>,
    pub waiting: usize,
    pub pending: usize,
    pub issues: Vec<String>,
}

impl SyncState {
    pub fn enabled(&self) -> bool {
        self.folder.is_some()
    }

    pub fn apply(&mut self, report: Report) {
        self.device = Some(report.device);
        if !report.devices.is_empty() {
            self.devices = report.devices;
        }
        self.conflicts = report.conflicts;
        self.last_round_ms = Some(report.at_ms);
        if report.received > 0 {
            self.last_received = Some((report.at_ms, report.received));
        }
        self.waiting = report.waiting;
        self.pending = report.pending;
        self.issues = report.issues;
    }
}

pub struct Report {
    pub device: Uuid,
    pub devices: Vec<(Uuid, DeviceInfo)>,
    pub conflicts: Vec<Conflict>,
    pub at_ms: u64,
    pub received: usize,
    pub waiting: usize,
    pub pending: usize,
    /// Library values written. The caller reads the library again.
    pub library_changed: bool,
    /// Shared settings written. The caller applies them to the open windows.
    pub settings_changed: bool,
    pub issues: Vec<String>,
}

/// Device-local sync state, outside the sync folder and the library.
fn state_path() -> Result<PathBuf> {
    Ok(
        directories::ProjectDirs::from("app", "GameSync", "GameSync")
            .context("App storage is unavailable")?
            .data_dir()
            .join("sync-state.json"),
    )
}

pub fn device_info(settings: &settings::Settings) -> DeviceInfo {
    DeviceInfo {
        name: settings
            .device_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(host_name),
        platform: std::env::consts::OS.into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        last_seen_ms: 0,
        extra: Default::default(),
    }
}

/// A readable default device name. The OS command exists on all targets.
pub fn host_name() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|name| name.trim().trim_end_matches(".local").to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "This device".into())
}

/// Open sync. `create` makes new sync data in a folder without it.
pub fn open(folder: &Path, create: bool) -> Result<Arc<Handle>> {
    let info = device_info(&settings::load()?);
    let engine = SyncEngine::open(folder, &state_path()?, info, create)?;
    Ok(Arc::new(Handle {
        engine: Mutex::new(engine),
        closed: AtomicBool::new(false),
    }))
}

fn lock(handle: &Handle) -> Result<std::sync::MutexGuard<'_, SyncEngine>> {
    handle
        .engine
        .lock()
        .map_err(|_| anyhow::anyhow!("Sync stopped after an internal error. Restart GameSync."))
}

/// One round. `loaded` must be read from disk after the previous round.
pub fn run_round(handle: &Handle, root: &Path, loaded: &LoadedLibrary) -> Result<Report> {
    let mut engine = lock(handle)?;
    anyhow::ensure!(!handle.closed.load(Ordering::SeqCst), "Sync is stopped");
    let local = project(
        &loaded.manifest.definitions,
        &loaded.games,
        &settings::load()?,
    );
    let at_ms = wall_ms();
    let round = engine.round(&local, at_ms);
    let mut issues = round.issues.clone();

    let library = apply_library(root, &loaded.games, &round.apply);
    issues.extend(library.issues);
    let mut done = library.done;
    let library_changed = !done.is_empty();

    let mut settings_changed = false;
    if round.apply.iter().any(|a| a.key.0 == Target::Settings) {
        let mut applied = None;
        settings::update(|s| applied = Some(apply_settings(s, &round.apply)))?;
        if let Some(applied) = applied {
            settings_changed = !applied.done.is_empty();
            done.extend(applied.done);
            issues.extend(applied.issues);
        }
    }
    engine.confirm(&round.apply, &done)?;
    let devices = match engine.devices() {
        Ok(found) => found.devices,
        Err(_) => Vec::new(),
    };
    Ok(Report {
        device: engine.device(),
        devices,
        conflicts: round.conflicts,
        at_ms,
        received: round.received,
        waiting: round.waiting,
        pending: library.pending,
        library_changed,
        settings_changed,
        issues,
    })
}

/// Write the user's choices. The next round applies them locally.
pub fn resolve(handle: &Handle, choices: Vec<(FieldKey, Value)>) -> Result<Vec<String>> {
    let mut engine = lock(handle)?;
    let mut issues = Vec::new();
    for (key, value) in choices {
        issues.extend(engine.resolve(&key, value, wall_ms()).issues);
    }
    Ok(issues)
}

pub fn rename(handle: &Handle, name: String) -> Result<()> {
    settings::update(|s| s.device_name = Some(name))?;
    let info = device_info(&settings::load()?);
    lock(handle)?.set_info(info, wall_ms())
}

/// Stop sync on this device. Local data and the folder stay. The device
/// state is removed, so a later join starts from nothing known.
pub fn stop(handle: Option<&Handle>) -> Result<()> {
    let _guard = match handle {
        Some(handle) => {
            let guard = lock(handle)?;
            handle.closed.store(true, Ordering::SeqCst);
            Some(guard)
        }
        None => None,
    };
    match std::fs::remove_file(state_path()?) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Could not remove sync state"),
    }
    settings::update(|s| s.sync_folder = None)
}
