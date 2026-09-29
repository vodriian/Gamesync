//! One sync session on one device: the folder, all known changes, and the
//! device state that must survive a restart.
//!
//! Each round compares three values for each field: the local value now, the
//! value at the last round (`known`), and the merged folder value.
//!
//! - Local equals known: the user did not edit it here. A new folder value is
//!   applied locally.
//! - Local differs from known: the user edited it here. The edit is written
//!   with the known changes as its base, so an unseen remote edit becomes a
//!   conflict and is not overwritten.
//!
//! The first round after joining has no known values. Then a value set on only
//! one side is taken, and different values become conflicts for review.

use super::project::is_synced;
use super::secrets::same_value;
use super::{
    apply::Apply, project::Projection, Change, ChangeSet, Cursor, DeviceInfo, Devices,
    DuplicateDevice, FieldKey, FieldState, Insert, SyncFolder, Target, MAX_BATCH_CHANGES,
};
use crate::storage::replace_current;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// A field that two devices changed without seeing each other's change.
#[derive(Clone, Debug)]
pub struct Conflict {
    pub key: FieldKey,
    /// The value on this device. `Null` means unset.
    pub local: Value,
    /// The competing changes, oldest first. One can be from this device.
    pub candidates: Vec<Change>,
}

#[derive(Debug, Default)]
pub struct Round {
    /// Folder values to write locally. Call `confirm` with the ones written.
    pub apply: Vec<Apply>,
    pub conflicts: Vec<Conflict>,
    /// Changes received from the folder in this round.
    pub received: usize,
    /// Local changes written to the folder in this round.
    pub written: usize,
    /// Local changes not written yet, for example while the folder is offline.
    pub waiting: usize,
    pub issues: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct Known {
    target: Target,
    field: String,
    value: Value,
    ids: Vec<Uuid>,
}

/// Device-local sync state. It stays outside the sync folder.
#[derive(Deserialize, Serialize)]
struct DeviceState {
    device: Uuid,
    sync_id: Uuid,
    last_written: u64,
    known: Vec<Known>,
    /// Local changes saved here before they are written to the folder.
    outbox: Vec<Change>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub struct SyncEngine {
    folder: SyncFolder,
    set: ChangeSet,
    cursor: Cursor,
    state_path: PathBuf,
    device: Uuid,
    last_written: u64,
    known: BTreeMap<FieldKey, (Value, Vec<Uuid>)>,
    outbox: Vec<Change>,
    info: DeviceInfo,
    extra: BTreeMap<String, Value>,
}

impl SyncEngine {
    /// Open sync for this device. `state_path` is a device-local file. With
    /// `create`, an empty selected folder gets new sync data.
    ///
    /// A device that was synced with a different sync folder is refused; the
    /// caller stops sync and joins again, so known values from the old folder
    /// are not used as a base for the new one.
    pub fn open(
        selected: &Path,
        state_path: &Path,
        info: DeviceInfo,
        create: bool,
    ) -> Result<Self> {
        let state: Option<DeviceState> =
            match fs::read(state_path) {
                Ok(bytes) => Some(serde_json::from_slice(&bytes).with_context(|| {
                    format!("Sync state is not valid: {}", state_path.display())
                })?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error).context("Could not read sync state"),
            };
        let device = state.as_ref().map_or_else(Uuid::new_v4, |s| s.device);
        let folder = if create {
            SyncFolder::create(selected, device)?
        } else {
            SyncFolder::open(selected, device)?
        };
        let mut engine = Self {
            set: ChangeSet::new(device),
            cursor: Cursor::default(),
            state_path: state_path.to_path_buf(),
            device,
            last_written: 0,
            known: BTreeMap::new(),
            outbox: Vec::new(),
            info,
            extra: BTreeMap::new(),
            folder,
        };
        if let Some(state) = state {
            anyhow::ensure!(
                state.sync_id == engine.folder.sync_id(),
                "This device was synced with another sync folder. Stop syncing, then join this folder."
            );
            engine.last_written = state.last_written;
            engine.known = state
                .known
                .into_iter()
                .map(|k| ((k.target, k.field), (k.value, k.ids)))
                .collect();
            for change in &state.outbox {
                engine.set.insert(change.clone());
            }
            engine.outbox = state.outbox;
            engine.extra = state.extra;
        }
        engine.save()?;
        // Other devices list this device from its first session, even
        // before it writes a change.
        engine.publish_info(super::wall_ms())?;
        Ok(engine)
    }

    /// Change the name or version that other devices show.
    pub fn set_info(&mut self, info: DeviceInfo, wall_ms: u64) -> Result<()> {
        self.info = info;
        self.publish_info(wall_ms)
    }

    fn publish_info(&self, wall_ms: u64) -> Result<()> {
        let mut info = self.info.clone();
        info.last_seen_ms = wall_ms;
        self.folder.write_device(&info)
    }

    pub fn device(&self) -> Uuid {
        self.device
    }

    pub fn sync_id(&self) -> Uuid {
        self.folder.sync_id()
    }

    pub fn folder(&self) -> &Path {
        self.folder.root()
    }

    /// The merged folder value of one field, if it is not in conflict.
    pub fn value(&self, target: &Target, field: &str) -> Option<&Value> {
        match self.set.state(target, field)? {
            FieldState::Value(change) => Some(&change.value),
            FieldState::Conflict(_) => None,
        }
    }

    pub fn has_conflict(&self, target: &Target, field: &str) -> bool {
        matches!(self.set.state(target, field), Some(FieldState::Conflict(_)))
    }

    pub fn devices(&self) -> Result<Devices> {
        self.folder.devices()
    }

    /// Read the folder, record local edits, and write them.
    ///
    /// `local` must be read from disk after the values of the previous round
    /// were applied. A stale projection looks like a local edit that sets the
    /// old values again.
    pub fn round(&mut self, local: &Projection, wall_ms: u64) -> Round {
        let mut round = Round::default();
        match self.folder.scan(&mut self.cursor) {
            Ok(scan) => {
                for issue in scan.issues {
                    round
                        .issues
                        .push(format!("{}: {:?}", issue.path.display(), issue.kind));
                }
                for change in scan.changes {
                    match self.set.insert(change) {
                        Insert::Added => round.received += 1,
                        Insert::Known => {}
                        Insert::Clash => round
                            .issues
                            .push("Two different sync changes use one ID".into()),
                    }
                }
            }
            Err(error) => round.issues.push(format!("{error:#}")),
        }

        let mut keys: BTreeSet<FieldKey> = local.fields.keys().cloned().collect();
        keys.extend(self.known.keys().cloned());
        keys.extend(self.set.fields().map(|(key, _)| key.clone()));
        keys.retain(is_synced);
        let before = self.known.len();
        self.known.retain(|key, _| is_synced(key));
        let mut edits = Vec::new();
        let mut changed = self.known.len() != before;
        for key in keys {
            let (known, known_ids) = self
                .known
                .get(&key)
                .cloned()
                .unwrap_or((Value::Null, Vec::new()));
            let state = self.set.state(&key.0, &key.1);
            if !local.targets.contains(&key.0) {
                // Not here now: offer the value, which waits until the target
                // is present. Never read absence as a cleared value.
                if let Some(FieldState::Value(change)) = state {
                    if !same_value(&change.value, &known) {
                        round.apply.push(Apply {
                            key,
                            value: change.value.clone(),
                            ids: vec![change.id],
                        });
                    }
                }
                continue;
            }
            let here = local.fields.get(&key).cloned().unwrap_or(Value::Null);
            match state {
                Some(FieldState::Conflict(heads)) if same_value(&here, &known) => {
                    round.conflicts.push(Conflict {
                        key,
                        local: here,
                        candidates: heads.into_iter().cloned().collect(),
                    });
                }
                // An edit here during a conflict is the user's choice.
                Some(FieldState::Conflict(heads)) => {
                    let base = heads.iter().map(|c| c.id).collect();
                    edits.push((key, here, base));
                }
                Some(FieldState::Value(change)) if same_value(&here, &known) => {
                    if !same_value(&change.value, &known) {
                        round.apply.push(Apply {
                            key,
                            value: change.value.clone(),
                            ids: vec![change.id],
                        });
                    } else if known_ids != [change.id] {
                        let id = change.id;
                        self.known.insert(key, (known, vec![id]));
                        changed = true;
                    }
                }
                Some(FieldState::Value(change)) if same_value(&change.value, &here) => {
                    let id = change.id;
                    self.known.insert(key, (here, vec![id]));
                    changed = true;
                }
                _ if same_value(&here, &known) => {}
                _ => edits.push((key, here, known_ids)),
            }
        }
        for (key, value, base) in edits {
            let change = self
                .set
                .edit_from(wall_ms, key.0.clone(), &key.1, value.clone(), base);
            // An edit based on an old value meets an unseen change here, for
            // example a different value found when joining.
            if let Some(FieldState::Conflict(heads)) = self.set.state(&key.0, &key.1) {
                round.conflicts.push(Conflict {
                    key: key.clone(),
                    local: value.clone(),
                    candidates: heads.into_iter().cloned().collect(),
                });
            }
            self.known.insert(key, (value, vec![change.id]));
            self.outbox.push(change);
            changed = true;
        }
        if changed {
            if let Err(error) = self.save() {
                round.issues.push(format!("{error:#}"));
            }
        }
        self.flush(wall_ms, &mut round);
        round
    }

    /// Record values of `apply` that were written locally.
    pub fn confirm(&mut self, apply: &[Apply], done: &[usize]) -> Result<()> {
        for index in done {
            let Some(item) = apply.get(*index) else {
                continue;
            };
            self.known
                .insert(item.key.clone(), (item.value.clone(), item.ids.clone()));
        }
        self.save()
    }

    /// Write a value the user chose: a conflict resolution, or a value that
    /// is not read from local data, such as the protected sync key. It
    /// replaces every current candidate. The next round applies it locally
    /// if it differs from the local value.
    pub fn resolve(&mut self, key: &FieldKey, value: Value, wall_ms: u64) -> Round {
        let change = self.set.edit(wall_ms, key.0.clone(), &key.1, value);
        self.outbox.push(change);
        let mut round = Round::default();
        if let Err(error) = self.save() {
            round.issues.push(format!("{error:#}"));
        }
        self.flush(wall_ms, &mut round);
        round
    }

    fn flush(&mut self, wall_ms: u64, round: &mut Round) {
        let mut replaced = false;
        while !self.outbox.is_empty() {
            let count = self.outbox.len().min(MAX_BATCH_CHANGES);
            match self
                .folder
                .write_batch(self.last_written, &self.outbox[..count])
            {
                Ok(seq) => {
                    self.last_written = seq;
                    self.outbox.drain(..count);
                    round.written += count;
                    if let Err(error) = self.save() {
                        round.issues.push(format!("{error:#}"));
                        break;
                    }
                }
                Err(error) if error.is::<DuplicateDevice>() && !replaced => {
                    round.issues.push(format!("{error:#}"));
                    replaced = true;
                    if let Err(error) = self.replace_device(wall_ms) {
                        round.issues.push(format!("{error:#}"));
                        break;
                    }
                }
                Err(error) => {
                    round
                        .issues
                        .push(format!("Changes wait to be written: {error:#}"));
                    break;
                }
            }
        }
        round.waiting = self.outbox.len();
        if round.written > 0 {
            if let Err(error) = self.publish_info(wall_ms) {
                round.issues.push(format!("{error:#}"));
            }
        }
    }

    /// Another installation writes with this device ID. Continue under a new
    /// ID: read the folder again and make the unwritten changes again.
    fn replace_device(&mut self, wall_ms: u64) -> Result<()> {
        let device = Uuid::new_v4();
        let folder = SyncFolder::open(self.folder.root(), device)?;
        let mut set = ChangeSet::new(device);
        let mut cursor = Cursor::default();
        for change in folder.scan(&mut cursor)?.changes {
            set.insert(change);
        }
        let mut renamed = HashMap::new();
        let mut outbox = Vec::new();
        for old in std::mem::take(&mut self.outbox) {
            let base = old
                .base
                .iter()
                .map(|id| *renamed.get(id).unwrap_or(id))
                .collect();
            let new = set.edit_from(wall_ms, old.target, &old.field, old.value, base);
            renamed.insert(old.id, new.id);
            outbox.push(new);
        }
        for (_, ids) in self.known.values_mut() {
            for id in ids {
                *id = *renamed.get(id).unwrap_or(id);
            }
        }
        self.device = device;
        self.folder = folder;
        self.set = set;
        self.cursor = cursor;
        self.last_written = 0;
        self.outbox = outbox;
        self.save()?;
        self.publish_info(wall_ms)
    }

    fn save(&self) -> Result<()> {
        let state = DeviceState {
            device: self.device,
            sync_id: self.folder.sync_id(),
            last_written: self.last_written,
            known: self
                .known
                .iter()
                .map(|((target, field), (value, ids))| Known {
                    target: target.clone(),
                    field: field.clone(),
                    value: value.clone(),
                    ids: ids.clone(),
                })
                .collect(),
            outbox: self.outbox.clone(),
            extra: self.extra.clone(),
        };
        let parent = self
            .state_path
            .parent()
            .context("Sync state path has no folder")?;
        fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec(&state)?;
        replace_current(&self.state_path, &bytes).context("Could not save sync state")
    }
}
