//! The shared sync folder. It is a transport, not a library.
//!
//! Each device writes only its own files, so a cloud tool never sees two
//! writers of one file. Change batches are immutable once written. Files can
//! arrive late, twice, or partly downloaded; a file that cannot be read is
//! skipped and read again at the next scan.

use super::Change;
use crate::storage::{publish_revision, replace_current};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt, fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// The subfolder that GameSync creates in the folder the user selects.
pub const FOLDER_NAME: &str = "GameSync Sync";
const FORMAT: u32 = 1;
/// A join writes all personal data of a device, so it uses several batches.
pub const MAX_BATCH_CHANGES: usize = 1000;
const MAX_BATCH_BYTES: u64 = 16 << 20;
const MAX_DEVICE_BYTES: u64 = 64 << 10;
const MANIFEST: &str = "sync.json";

#[derive(Deserialize, Serialize)]
struct Manifest {
    format: u32,
    sync_id: Uuid,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

/// What other devices show about this device. Written only by that device.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    /// `macos`, `linux`, or `windows`.
    pub platform: String,
    pub app_version: String,
    /// Wall time of the last write, in milliseconds since the Unix epoch.
    pub last_seen_ms: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct Batch {
    format: u32,
    device: Uuid,
    seq: u64,
    changes: Vec<Change>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

/// Another installation writes batches with this device ID, usually because
/// device settings were copied. Stop writing and use a new device ID.
#[derive(Debug)]
pub struct DuplicateDevice;

impl fmt::Display for DuplicateDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Another device uses this device ID. Sync will use a new ID for this device.")
    }
}

impl std::error::Error for DuplicateDevice {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueKind {
    /// Not readable yet or not valid. It is read again at the next scan.
    Unreadable(String),
    /// Written by a newer app version. Update GameSync to read it.
    NewerFormat,
    /// Not a GameSync file, often a cloud conflict copy. It is not read.
    Unexpected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub path: PathBuf,
    pub kind: IssueKind,
}

/// The batches that this device has read, by device. Kept in memory; a new
/// session reads all batches again.
#[derive(Default)]
pub struct Cursor {
    read: HashMap<Uuid, BTreeSet<u64>>,
}

/// Device files by device ID, and the files that could not be read.
pub struct Devices {
    pub devices: Vec<(Uuid, DeviceInfo)>,
    pub issues: Vec<Issue>,
}

#[derive(Default)]
pub struct Scan {
    /// New changes, in device and batch order.
    pub changes: Vec<Change>,
    pub issues: Vec<Issue>,
}

pub struct SyncFolder {
    root: PathBuf,
    sync_id: Uuid,
    device: Uuid,
}

impl SyncFolder {
    /// The sync folder for a selected folder. The user can select the parent
    /// or the sync folder itself.
    pub fn locate(selected: &Path) -> PathBuf {
        if selected.file_name().is_some_and(|name| name == FOLDER_NAME) {
            selected.to_path_buf()
        } else {
            selected.join(FOLDER_NAME)
        }
    }

    pub fn exists(selected: &Path) -> bool {
        Self::locate(selected).join(MANIFEST).is_file()
    }

    /// Open an existing sync folder.
    pub fn open(selected: &Path, device: Uuid) -> Result<Self> {
        let root = Self::locate(selected);
        let path = root.join(MANIFEST);
        let manifest: Manifest = serde_json::from_slice(
            &read_limited(&path, MAX_DEVICE_BYTES)
                .with_context(|| format!("No GameSync sync data in {}", root.display()))?,
        )
        .with_context(|| format!("Sync folder file is not valid: {}", path.display()))?;
        ensure!(
            manifest.format <= FORMAT,
            "This sync folder needs a newer GameSync version"
        );
        Ok(Self {
            root,
            sync_id: manifest.sync_id,
            device,
        })
    }

    /// Open the sync folder, or create it in an existing selected folder.
    pub fn create(selected: &Path, device: Uuid) -> Result<Self> {
        if Self::exists(selected) {
            return Self::open(selected, device);
        }
        ensure!(
            selected.is_dir(),
            "Folder not found: {}",
            selected.display()
        );
        let root = Self::locate(selected);
        fs::create_dir_all(&root)
            .with_context(|| format!("Could not create {}", root.display()))?;
        let manifest = Manifest {
            format: FORMAT,
            sync_id: Uuid::new_v4(),
            extra: BTreeMap::new(),
        };
        // No-clobber: if another device created the folder at the same time,
        // keep its manifest and join it.
        let mut bytes = serde_json::to_vec_pretty(&manifest)?;
        bytes.push(b'\n');
        if publish_revision(&root.join(MANIFEST), &bytes).is_err() && !Self::exists(selected) {
            bail!("Could not create sync data in {}", root.display());
        }
        Self::open(selected, device)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Identifies this sync folder. A device remembers it, so a replaced
    /// folder is found and not mixed with the old one.
    pub fn sync_id(&self) -> Uuid {
        self.sync_id
    }

    pub fn write_device(&self, info: &DeviceInfo) -> Result<()> {
        let dir = self.root.join("devices");
        fs::create_dir_all(&dir)?;
        let mut bytes = serde_json::to_vec_pretty(info)?;
        bytes.push(b'\n');
        replace_current(&dir.join(format!("{}.json", self.device)), &bytes)
    }

    pub fn devices(&self) -> Result<Devices> {
        let mut devices = Vec::new();
        let mut issues = Vec::new();
        for (path, name) in entries(&self.root.join("devices"), &mut issues)? {
            let Some(id) = name.strip_suffix(".json").and_then(parse_id) else {
                issues.push(Issue {
                    path,
                    kind: IssueKind::Unexpected,
                });
                continue;
            };
            match read_limited(&path, MAX_DEVICE_BYTES)
                .and_then(|bytes| Ok(serde_json::from_slice::<DeviceInfo>(&bytes)?))
            {
                Ok(info) => devices.push((id, info)),
                Err(error) => issues.push(Issue {
                    path,
                    kind: IssueKind::Unreadable(format!("{error:#}")),
                }),
            }
        }
        devices.sort_by_key(|(id, _)| *id);
        Ok(Devices { devices, issues })
    }

    /// Write one batch of this device's changes and return its number.
    ///
    /// `last_written` is the number of the last batch that this device wrote,
    /// or 0. The caller stores the result before the next write. A retry with
    /// the same changes after a failure is safe. Any other batch with a number
    /// above `last_written` is from another writer: this returns
    /// `DuplicateDevice`.
    pub fn write_batch(&self, last_written: u64, changes: &[Change]) -> Result<u64> {
        ensure!(
            !changes.is_empty(),
            "A sync batch needs at least one change"
        );
        ensure!(
            changes.len() <= MAX_BATCH_CHANGES,
            "A sync batch can have at most {MAX_BATCH_CHANGES} changes"
        );
        for change in changes {
            change.validate()?;
            ensure!(
                change.at.device == self.device,
                "A device can only write its own changes"
            );
        }
        let seq = last_written + 1;
        let dir = self.root.join("changes").join(self.device.to_string());
        fs::create_dir_all(&dir).with_context(|| format!("Could not create {}", dir.display()))?;
        let batch = Batch {
            format: FORMAT,
            device: self.device,
            seq,
            changes: changes.to_vec(),
            extra: BTreeMap::new(),
        };
        let bytes = serde_json::to_vec(&batch)?;
        ensure!(
            bytes.len() as u64 <= MAX_BATCH_BYTES,
            "A sync batch is too large"
        );

        let path = dir.join(batch_name(seq));
        let mut ignored = Vec::new();
        let other_writer = entries(&dir, &mut ignored)?
            .into_iter()
            .filter_map(|(_, name)| parse_batch_name(&name))
            .any(|found| found > seq)
            || fs::read(&path).is_ok_and(|previous| previous != bytes);
        if other_writer {
            return Err(DuplicateDevice.into());
        }
        publish_revision(&path, &bytes)?;
        Ok(seq)
    }

    /// Read batches that `cursor` has not read. Fails only when the folder
    /// itself cannot be read; problems with single files are issues.
    pub fn scan(&self, cursor: &mut Cursor) -> Result<Scan> {
        let mut scan = Scan::default();
        let mut devices = Vec::new();
        for (path, name) in entries(&self.root.join("changes"), &mut scan.issues)? {
            match parse_id(&name).filter(|_| path.is_dir()) {
                Some(device) => devices.push((device, path)),
                None => scan.issues.push(Issue {
                    path,
                    kind: IssueKind::Unexpected,
                }),
            }
        }
        devices.sort();
        for (device, dir) in devices {
            let mut batches = Vec::new();
            for (path, name) in entries(&dir, &mut scan.issues)? {
                match parse_batch_name(&name) {
                    Some(seq) => batches.push((seq, path)),
                    None => scan.issues.push(Issue {
                        path,
                        kind: IssueKind::Unexpected,
                    }),
                }
            }
            batches.sort();
            let read = cursor.read.entry(device).or_default();
            for (seq, path) in batches {
                if read.contains(&seq) {
                    continue;
                }
                match read_batch(&path, device, seq) {
                    Ok(changes) => {
                        read.insert(seq);
                        scan.changes.extend(changes);
                    }
                    Err(kind) => scan.issues.push(Issue { path, kind }),
                }
            }
        }
        Ok(scan)
    }
}

fn read_batch(path: &Path, device: Uuid, seq: u64) -> Result<Vec<Change>, IssueKind> {
    let unreadable = |error: anyhow::Error| IssueKind::Unreadable(format!("{error:#}"));
    let bytes = read_limited(path, MAX_BATCH_BYTES).map_err(unreadable)?;
    // Check the format before the full shape, so a newer batch is reported
    // as newer and not as damaged.
    #[derive(Deserialize)]
    struct Header {
        format: u32,
    }
    let header: Header = serde_json::from_slice(&bytes).map_err(|e| unreadable(e.into()))?;
    if header.format > FORMAT {
        return Err(IssueKind::NewerFormat);
    }
    let batch: Batch = serde_json::from_slice(&bytes).map_err(|e| unreadable(e.into()))?;
    if batch.device != device || batch.seq != seq {
        return Err(IssueKind::Unreadable(
            "The batch does not match its file name".into(),
        ));
    }
    for change in &batch.changes {
        change.validate().map_err(unreadable)?;
        if change.at.device != device {
            return Err(IssueKind::Unreadable(
                "The batch has a change from another device".into(),
            ));
        }
    }
    Ok(batch.changes)
}

/// Directory entries with UTF-8 names. A missing directory has no entries.
/// Hidden files, such as staged writes and cloud tool files, are skipped.
fn entries(dir: &Path, issues: &mut Vec<Issue>) -> Result<Vec<(PathBuf, String)>> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", dir.display()))
        }
    };
    let mut found = Vec::new();
    for entry in listing {
        let entry = entry.with_context(|| format!("Could not read {}", dir.display()))?;
        let path = entry.path();
        match entry.file_name().into_string() {
            Ok(name) if name.starts_with('.') || name.eq_ignore_ascii_case("desktop.ini") => {}
            Ok(name) => found.push((path, name)),
            Err(_) => issues.push(Issue {
                path,
                kind: IssueKind::Unexpected,
            }),
        }
    }
    Ok(found)
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let size = fs::metadata(path)
        .with_context(|| format!("Could not read {}", path.display()))?
        .len();
    ensure!(size <= limit, "File is too large: {}", path.display());
    fs::read(path).with_context(|| format!("Could not read {}", path.display()))
}

/// Only the canonical lowercase form, so each device has one folder name.
fn parse_id(name: &str) -> Option<Uuid> {
    Uuid::parse_str(name)
        .ok()
        .filter(|id| id.to_string() == name)
}

fn batch_name(seq: u64) -> String {
    format!("{seq:010}.json")
}

fn parse_batch_name(name: &str) -> Option<u64> {
    let digits = name.strip_suffix(".json")?;
    let seq = (digits.len() == 10 && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())??;
    (seq > 0).then_some(seq)
}
