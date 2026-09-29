//! Three devices share one temporary folder, as they would share a cloud folder.

use anyhow::Result;
use gamesync_desktop::sync::{
    Change, ChangeSet, Cursor, DeviceInfo, DuplicateDevice, FieldState, IssueKind, SyncFolder,
    Target, FOLDER_NAME,
};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path};
use uuid::Uuid;

const MAC: Uuid = Uuid::from_u128(1);
const LINUX: Uuid = Uuid::from_u128(2);
const WINDOWS: Uuid = Uuid::from_u128(3);

struct Device {
    folder: SyncFolder,
    set: ChangeSet,
    cursor: Cursor,
    last_written: u64,
}

impl Device {
    fn join(selected: &Path, id: Uuid) -> Result<Self> {
        Ok(Self {
            folder: SyncFolder::create(selected, id)?,
            set: ChangeSet::new(id),
            cursor: Cursor::default(),
            last_written: 0,
        })
    }

    fn edit(&mut self, ms: u64, target: Target, field: &str, value: serde_json::Value) -> Change {
        self.set.edit(ms, target, field, value)
    }

    fn write(&mut self, changes: &[Change]) -> Result<()> {
        self.last_written = self.folder.write_batch(self.last_written, changes)?;
        Ok(())
    }

    fn receive(&mut self) -> Result<usize> {
        let scan = self.folder.scan(&mut self.cursor)?;
        assert!(scan.issues.is_empty(), "{:?}", scan.issues);
        let count = scan.changes.len();
        for change in scan.changes {
            self.set.insert(change);
        }
        Ok(count)
    }
}

fn batch_dir(selected: &Path, device: Uuid) -> std::path::PathBuf {
    selected
        .join(FOLDER_NAME)
        .join("changes")
        .join(device.to_string())
}

#[test]
fn three_devices_exchange_changes_through_one_folder() -> Result<()> {
    let cloud = tempfile::tempdir()?;
    let mut mac = Device::join(cloud.path(), MAC)?;
    // The sync folder itself can also be selected.
    let mut linux = Device::join(&cloud.path().join(FOLDER_NAME), LINUX)?;
    let mut windows = Device::join(cloud.path(), WINDOWS)?;
    assert_eq!(mac.folder.sync_id(), linux.folder.sync_id());
    assert_eq!(mac.folder.sync_id(), windows.folder.sync_id());

    let rating = mac.edit(1_000, Target::Steam(620), "personal.rating", json!(8));
    let theme = mac.edit(1_001, Target::Settings, "theme", json!("flexoki"));
    mac.write(&[rating, theme])?;
    let notes = linux.edit(2_000, Target::Steam(440), "personal.notes", json!("Co-op"));
    linux.write(&[notes])?;

    assert_eq!(windows.receive()?, 3);
    // A second scan reads nothing again.
    assert_eq!(windows.receive()?, 0);
    let later = windows.edit(3_000, Target::Steam(620), "personal.rating", json!(9));
    windows.write(&[later])?;

    for device in [&mut mac, &mut linux, &mut windows] {
        device.receive()?;
        match device.set.state(&Target::Steam(620), "personal.rating") {
            Some(FieldState::Value(change)) => assert_eq!(change.value, json!(9)),
            other => panic!("expected a value, found {other:?}"),
        }
        assert_eq!(device.set.changes().count(), 4);
    }
    Ok(())
}

#[test]
fn device_info_is_listed_for_other_devices() -> Result<()> {
    let cloud = tempfile::tempdir()?;
    let mac = Device::join(cloud.path(), MAC)?;
    let linux = Device::join(cloud.path(), LINUX)?;
    let info = DeviceInfo {
        name: "Omarchy desktop".into(),
        platform: "linux".into(),
        app_version: "0.1.0".into(),
        last_seen_ms: 5,
        extra: BTreeMap::new(),
    };
    linux.folder.write_device(&info)?;
    let found = mac.folder.devices()?;
    assert!(found.issues.is_empty());
    assert_eq!(found.devices, vec![(LINUX, info)]);
    Ok(())
}

#[test]
fn a_partial_file_is_read_again_at_the_next_scan() -> Result<()> {
    let cloud = tempfile::tempdir()?;
    let mut mac = Device::join(cloud.path(), MAC)?;
    let mut linux = Device::join(cloud.path(), LINUX)?;
    let change = mac.edit(1_000, Target::Steam(620), "personal.favorite", json!(true));
    mac.write(&[change])?;

    let path = batch_dir(cloud.path(), MAC).join("0000000001.json");
    let complete = fs::read(&path)?;
    fs::write(&path, &complete[..complete.len() / 2])?;
    let scan = linux.folder.scan(&mut linux.cursor)?;
    assert!(scan.changes.is_empty());
    assert!(matches!(scan.issues[0].kind, IssueKind::Unreadable(_)));

    // The cloud tool finishes the download.
    fs::write(&path, complete)?;
    assert_eq!(linux.receive()?, 1);
    Ok(())
}

#[test]
fn conflict_copies_are_reported_and_staged_files_are_ignored() -> Result<()> {
    let cloud = tempfile::tempdir()?;
    let mut mac = Device::join(cloud.path(), MAC)?;
    let mut linux = Device::join(cloud.path(), LINUX)?;
    let change = mac.edit(1_000, Target::Steam(620), "personal.hidden", json!(true));
    mac.write(&[change])?;

    let dir = batch_dir(cloud.path(), MAC);
    let original = fs::read(dir.join("0000000001.json"))?;
    for name in [
        "0000000001 (conflicted copy 2026-09-29).json",
        "0000000001.sync-conflict-20260929-120000-ABCDEFG.json",
    ] {
        fs::write(dir.join(name), &original)?;
    }
    fs::write(dir.join(".gamesync-abc.tmp"), b"{")?;
    fs::write(dir.join("desktop.ini"), b"")?;

    let scan = linux.folder.scan(&mut linux.cursor)?;
    assert_eq!(scan.changes.len(), 1);
    assert_eq!(scan.issues.len(), 2);
    assert!(scan
        .issues
        .iter()
        .all(|issue| issue.kind == IssueKind::Unexpected));
    Ok(())
}

#[test]
fn a_second_writer_with_the_same_device_id_is_found() -> Result<()> {
    let cloud = tempfile::tempdir()?;
    let mut mac = Device::join(cloud.path(), MAC)?;
    let first = mac.edit(1_000, Target::Steam(620), "personal.rating", json!(6));
    mac.write(std::slice::from_ref(&first))?;
    // An exact retry after an unconfirmed write succeeds.
    assert_eq!(mac.folder.write_batch(0, std::slice::from_ref(&first))?, 1);

    // A copy of the Mac's settings has the same ID and has written nothing.
    let mut copy = Device::join(cloud.path(), MAC)?;
    let other = copy.edit(2_000, Target::Steam(620), "personal.rating", json!(2));
    let error = copy.write(std::slice::from_ref(&other)).unwrap_err();
    assert!(error.downcast_ref::<DuplicateDevice>().is_some());

    // The Mac finds the copy's later batch the same way.
    let copy_folder = SyncFolder::open(cloud.path(), MAC)?;
    copy_folder.write_batch(1, &[other])?;
    let next = mac.edit(3_000, Target::Steam(620), "personal.rating", json!(7));
    let error = mac.write(&[next]).unwrap_err();
    assert!(error.downcast_ref::<DuplicateDevice>().is_some());
    Ok(())
}

#[test]
fn invalid_and_newer_batches_are_not_applied() -> Result<()> {
    let cloud = tempfile::tempdir()?;
    let mut mac = Device::join(cloud.path(), MAC)?;
    let mut linux = Device::join(cloud.path(), LINUX)?;
    let change = mac.edit(1_000, Target::Steam(620), "personal.rating", json!(6));

    // A device cannot write another device's change.
    let foreign = linux.edit(1_000, Target::Steam(620), "personal.rating", json!(1));
    assert!(mac.write(std::slice::from_ref(&foreign)).is_err());

    // A batch in the Mac's folder that holds a Linux change is rejected.
    let dir = batch_dir(cloud.path(), MAC);
    fs::create_dir_all(&dir)?;
    let forged = json!({"format": 1, "device": MAC, "seq": 1, "changes": [foreign]});
    fs::write(dir.join("0000000001.json"), serde_json::to_vec(&forged)?)?;
    let newer = json!({"format": 2, "device": MAC, "seq": 2, "changes": [change]});
    fs::write(dir.join("0000000002.json"), serde_json::to_vec(&newer)?)?;

    let scan = linux.folder.scan(&mut linux.cursor)?;
    assert!(scan.changes.is_empty());
    let kinds: Vec<_> = scan.issues.iter().map(|issue| &issue.kind).collect();
    assert!(matches!(kinds[0], IssueKind::Unreadable(_)));
    assert_eq!(kinds[1], &IssueKind::NewerFormat);
    Ok(())
}

#[test]
fn opening_needs_existing_supported_sync_data() -> Result<()> {
    let cloud = tempfile::tempdir()?;
    assert!(!SyncFolder::exists(cloud.path()));
    assert!(SyncFolder::open(cloud.path(), MAC).is_err());
    assert!(SyncFolder::create(&cloud.path().join("missing"), MAC).is_err());

    let root = cloud.path().join(FOLDER_NAME);
    fs::create_dir(&root)?;
    let manifest = json!({"format": 2, "sync_id": Uuid::from_u128(9)});
    fs::write(root.join("sync.json"), serde_json::to_vec(&manifest)?)?;
    let error = SyncFolder::open(cloud.path(), MAC).err().unwrap();
    assert!(error.to_string().contains("newer GameSync"));
    Ok(())
}
