//! Data sync in the running app. The library refresh loop runs one round
//! after each disk read, so every round sees fresh local values.
use anyhow::{Context as _, Result};
use gamesync_desktop::{
    credentials::{CredentialStore, OsCredential},
    library_reader::LoadedLibrary,
    settings,
    sync::{
        apply_library, apply_settings, project,
        secrets::{ExposeSecret as _, SyncKey, SYNC_KEY, VALUE_FIELD, WRAPPED_FIELD},
        wall_ms, Apply, Conflict, DeviceInfo, FieldKey, Projection, SyncEngine, Target,
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
    /// The sync key on this device, after the passphrase was entered once.
    key: Mutex<Option<SyncKey>>,
}

/// The synced Steam Web API key. AI provider keys use the same path later.
const STEAM_KEY: &str = "steam_api_key";

/// Whether this device can send and receive API keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyState {
    /// No device has turned on key sync.
    #[default]
    Off,
    /// Key sync is on; this device needs the passphrase once.
    Locked,
    /// Another device replaced the sync key; enter the passphrase again.
    Replaced,
    /// Two devices turned on key sync at the same time; see the review.
    Review,
    Unlocked,
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
    pub keys: KeyState,
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
        self.keys = report.keys;
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
    /// An API key from another device was saved on this device.
    pub keys_changed: bool,
    pub keys: KeyState,
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
    // A locked credential store leaves keys locked; the rest of sync works.
    let key = OsCredential::sync_key(engine.sync_id())
        .and_then(|entry| entry.get())
        .ok()
        .flatten()
        .and_then(|text| SyncKey::from_secret(&text).ok());
    Ok(Arc::new(Handle {
        engine: Mutex::new(engine),
        closed: AtomicBool::new(false),
        key: Mutex::new(key),
    }))
}

fn sync_key_target() -> Target {
    Target::Secret(SYNC_KEY.into())
}

fn key_state(engine: &SyncEngine, key: Option<&SyncKey>) -> KeyState {
    match engine.value(&sync_key_target(), WRAPPED_FIELD) {
        Some(wrapped) => match key {
            Some(key) if key.matches(wrapped) => KeyState::Unlocked,
            Some(_) => KeyState::Replaced,
            None => KeyState::Locked,
        },
        None if engine.has_conflict(&sync_key_target(), WRAPPED_FIELD) => KeyState::Review,
        None => KeyState::Off,
    }
}

/// Add sealed API keys to the local values. Only an unlocked device reads
/// them; otherwise their targets stay absent and received keys wait.
fn project_keys(local: &mut Projection, key: &SyncKey, library: uuid::Uuid) -> Result<()> {
    let target = Target::Secret(STEAM_KEY.into());
    let secret = OsCredential::steam(library)?.get()?;
    local.targets.insert(target.clone());
    if let Some(secret) = secret {
        local
            .fields
            .insert((target, VALUE_FIELD.into()), key.seal(STEAM_KEY, &secret)?);
    }
    Ok(())
}

/// Save received API keys. Returns the indexes written.
fn apply_keys(
    values: &[Apply],
    key: Option<&SyncKey>,
    library: uuid::Uuid,
    issues: &mut Vec<String>,
) -> Vec<usize> {
    let mut done = Vec::new();
    for (index, item) in values.iter().enumerate() {
        let Target::Secret(name) = &item.key.0 else {
            continue;
        };
        // The protected sync key is read from the folder, never stored here.
        if name == SYNC_KEY {
            done.push(index);
            continue;
        }
        let (Some(key), true) = (key, name == STEAM_KEY) else {
            continue;
        };
        let result = OsCredential::steam(library).and_then(|entry| {
            if item.value.is_null() {
                entry.remove()
            } else {
                entry.set(key.open(&item.value)?.expose_secret())
            }
        });
        match result {
            Ok(()) => done.push(index),
            Err(error) => issues.push(format!("Steam API key: {error:#}")),
        }
    }
    done
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
    let mut local = project(
        &loaded.manifest.definitions,
        &loaded.games,
        &settings::load()?,
    );
    let key_guard = handle
        .key
        .lock()
        .map_err(|_| anyhow::anyhow!("Sync keys are unavailable. Restart GameSync."))?;
    let library_id = loaded.manifest.library_id;
    let mut issues = Vec::new();
    let unlocked = key_guard
        .as_ref()
        .filter(|key| key_state(&engine, Some(key)) == KeyState::Unlocked);
    if let Some(key) = unlocked {
        if let Err(error) = project_keys(&mut local, key, library_id) {
            issues.push(format!("Could not read the Steam API key: {error:#}"));
        }
    }
    let at_ms = wall_ms();
    let round = engine.round(&local, at_ms);
    issues.extend(round.issues.iter().cloned());

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
    let keys = apply_keys(&round.apply, unlocked, library_id, &mut issues);
    let keys_changed = keys
        .iter()
        .any(|i| round.apply[*i].key.0 != sync_key_target());
    done.extend(keys);
    engine.confirm(&round.apply, &done)?;
    let key_state = key_state(&engine, key_guard.as_ref());
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
        keys_changed,
        keys: key_state,
        issues,
    })
}

/// Turn on key sync with a new sync key. Scrypt runs before the engine lock.
pub fn enable_keys(handle: &Handle, passphrase: &str) -> Result<()> {
    {
        let engine = lock(handle)?;
        anyhow::ensure!(
            key_state(&engine, None) == KeyState::Off,
            "Key sync is already on. Enter its passphrase to unlock this device."
        );
    }
    let key = SyncKey::generate();
    let wrapped = key.wrap(passphrase)?;
    let mut engine = lock(handle)?;
    // Save the key on this device first: a folder value that no device can
    // open would lock every device out.
    OsCredential::sync_key(engine.sync_id())?.set(key.to_secret().expose_secret())?;
    engine.resolve(
        &(sync_key_target(), WRAPPED_FIELD.into()),
        wrapped,
        wall_ms(),
    );
    *handle
        .key
        .lock()
        .map_err(|_| anyhow::anyhow!("Sync keys are unavailable"))? = Some(key);
    Ok(())
}

/// Open the sync key from the folder with the passphrase.
pub fn unlock_keys(handle: &Handle, passphrase: &str) -> Result<()> {
    let (wrapped, sync_id) = {
        let engine = lock(handle)?;
        let wrapped = engine
            .value(&sync_key_target(), WRAPPED_FIELD)
            .cloned()
            .context("No device has turned on key sync yet")?;
        (wrapped, engine.sync_id())
    };
    let key = SyncKey::unwrap(&wrapped, passphrase)?;
    OsCredential::sync_key(sync_id)?.set(key.to_secret().expose_secret())?;
    *handle
        .key
        .lock()
        .map_err(|_| anyhow::anyhow!("Sync keys are unavailable"))? = Some(key);
    Ok(())
}

/// Protect the same sync key with a new passphrase. API keys stay as they are.
pub fn change_passphrase(handle: &Handle, passphrase: &str) -> Result<()> {
    let wrapped = {
        let key = handle
            .key
            .lock()
            .map_err(|_| anyhow::anyhow!("Sync keys are unavailable"))?;
        key.as_ref()
            .context("Unlock key sync on this device first")?
            .wrap(passphrase)?
    };
    lock(handle)?.resolve(
        &(sync_key_target(), WRAPPED_FIELD.into()),
        wrapped,
        wall_ms(),
    );
    Ok(())
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
            // Joining again asks for the passphrase again.
            OsCredential::sync_key(guard.sync_id())?.remove()?;
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
