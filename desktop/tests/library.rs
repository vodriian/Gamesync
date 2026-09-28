#![cfg(unix)]

use gamesync_desktop::{
    library::{LibraryDefinitions, LibraryStore, StatusDefinition},
    record_store::RecordStore,
    records::GameData,
};
use serde_json::json;
use std::{collections::BTreeSet, fs};
use tempfile::tempdir;
use uuid::Uuid;

fn custom_status() -> StatusDefinition {
    StatusDefinition {
        key: "weekend".into(),
        label: "For the weekend".into(),
        recommendation_eligible: true,
        extra: Default::default(),
    }
}

#[test]
fn a_new_library_reopens_after_relocation_with_stable_identity() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("Games.library");
    let store = LibraryStore::create(&path, "Evening games").unwrap();
    let first = store.inspect().unwrap().current().unwrap().clone();
    for folder in ["games", "media", "history/library"] {
        assert!(path.join(folder).is_dir());
    }
    assert_eq!(first.definitions.statuses.len(), 6);
    assert_eq!(first.definitions.default_status, "backlog");
    assert!(
        first
            .definitions
            .status("playing")
            .unwrap()
            .recommendation_eligible
    );
    assert!(
        !first
            .definitions
            .status("paused")
            .unwrap()
            .recommendation_eligible
    );
    let moved = temp.path().join("Moved.library");
    fs::rename(path, &moved).unwrap();
    let reopened = LibraryStore::open(moved).unwrap().inspect().unwrap();
    assert_eq!(reopened.current().unwrap(), &first);
}

#[test]
fn creation_rejects_existing_folders_and_invalid_names_without_changes() {
    let temp = tempdir().unwrap();
    let marker = temp.path().join("keep.txt");
    fs::write(&marker, "Keep this file").unwrap();
    assert!(LibraryStore::create(temp.path(), "Games").is_err());
    assert_eq!(fs::read_to_string(marker).unwrap(), "Keep this file");
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    let path = temp.path().join("Invalid.library");
    assert!(LibraryStore::create(&path, "  ").is_err());
    assert!(!path.exists());
}

#[test]
fn labels_order_and_default_change_without_rewriting_game_records() {
    let temp = tempdir().unwrap();
    let store = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let game = RecordStore::open(store.root())
        .unwrap()
        .create(GameData::new("Hades"))
        .unwrap();
    let game_path = store
        .root()
        .join("games")
        .join(format!("{}.json", game.game_id));
    let before = fs::read(game_path.clone()).unwrap();
    let initial = store.inspect().unwrap().current().unwrap().clone();
    let mut definitions = initial.definitions.clone();
    definitions.name = "Cozy evenings".into();
    definitions.statuses[0].label = "Someday".into();
    definitions.statuses.push(custom_status());
    definitions.statuses.rotate_right(1);
    definitions.default_status = "weekend".into();
    let edited = store
        .edit(initial.revision_id, definitions.clone())
        .unwrap();
    assert_eq!(edited.library_id, initial.library_id);
    assert_eq!(edited.parents, vec![initial.revision_id]);
    assert_eq!(edited.definitions, definitions);
    assert_eq!(
        edited.definitions.status("backlog").unwrap().label,
        "Someday"
    );
    assert_eq!(fs::read(game_path).unwrap(), before);
    let current = fs::read(store.root().join("library.json")).unwrap();
    assert!(store.edit(initial.revision_id, definitions).is_err());
    assert_eq!(
        fs::read(store.root().join("library.json")).unwrap(),
        current
    );
}

#[test]
fn invalid_status_edits_and_removals_do_not_publish() {
    let temp = tempdir().unwrap();
    let store = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let first = store.inspect().unwrap().current().unwrap().clone();
    let before = fs::read(store.root().join("library.json")).unwrap();
    let mut cases = Vec::new();
    let mut duplicate = first.definitions.clone();
    duplicate.statuses.push(duplicate.statuses[0].clone());
    cases.push(duplicate);
    let mut missing_default = first.definitions.clone();
    missing_default.default_status = "unknown".into();
    cases.push(missing_default);
    let mut removed = first.definitions.clone();
    removed.statuses.pop();
    cases.push(removed);
    let mut renamed_key = first.definitions.clone();
    renamed_key.statuses[0].key = "someday".into();
    renamed_key.default_status = "someday".into();
    cases.push(renamed_key);
    let mut invalid_key = first.definitions.clone();
    invalid_key.statuses.push(StatusDefinition {
        key: "../bad".into(),
        ..custom_status()
    });
    cases.push(invalid_key);
    let mut blank = first.definitions.clone();
    blank.statuses[0].label.clear();
    cases.push(blank);
    for definitions in cases {
        assert!(store.edit(first.revision_id, definitions).is_err());
    }
    assert_eq!(fs::read(store.root().join("library.json")).unwrap(), before);
    assert_eq!(store.inspect().unwrap().revisions.len(), 1);
}

#[test]
fn library_conflicts_require_all_reviewed_heads_and_retain_status_keys() {
    let temp = tempdir().unwrap();
    let store = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let first = store.inspect().unwrap().current().unwrap().clone();
    let mut local_definitions = first.definitions.clone();
    local_definitions.name = "Local name".into();
    let local = store
        .edit(first.revision_id, local_definitions.clone())
        .unwrap();
    let mut remote = first.clone();
    remote.revision_id = Uuid::new_v4();
    remote.parents = vec![first.revision_id];
    remote.definitions.statuses.push(custom_status());
    let copy = store.root().join("library (remote conflicted copy).json");
    let bytes = serde_json::to_vec(&remote).unwrap();
    fs::write(&copy, &bytes).unwrap();
    let snapshot = store.inspect().unwrap();
    assert!(snapshot.current().is_none());
    assert_eq!(
        snapshot.heads,
        BTreeSet::from([local.revision_id, remote.revision_id])
    );
    assert!(store
        .edit(local.revision_id, local_definitions.clone())
        .is_err());
    assert!(store
        .resolve(
            &snapshot.heads,
            local.revision_id,
            local_definitions.clone()
        )
        .is_err());
    local_definitions.statuses.push(custom_status());
    let resolved = store
        .resolve(&snapshot.heads, local.revision_id, local_definitions)
        .unwrap();
    assert_eq!(
        resolved.parents.iter().copied().collect::<BTreeSet<_>>(),
        snapshot.heads
    );
    assert_eq!(fs::read(copy).unwrap(), bytes);
    assert!(store
        .root()
        .join("history/library")
        .join(format!("{}.json", remote.revision_id))
        .exists());
    assert_eq!(store.inspect().unwrap().current().unwrap(), &resolved);
}

#[test]
fn broken_newer_and_foreign_manifests_leave_valid_history_available() {
    let temp = tempdir().unwrap();
    let store = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let first = store.inspect().unwrap().current().unwrap().clone();
    let path = store.root().join("library.json");
    for bytes in [
        b"{".as_slice(),
        br#"{"schema_version":2,"new_format":true}"#,
    ] {
        fs::write(&path, bytes).unwrap();
        let snapshot = store.inspect().unwrap();
        assert!(snapshot.current().is_none());
        assert!(snapshot.revisions.contains_key(&first.revision_id));
        assert!(store
            .edit(first.revision_id, first.definitions.clone())
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    let mut foreign = first.clone();
    foreign.library_id = Uuid::new_v4();
    foreign.revision_id = Uuid::new_v4();
    fs::write(path, serde_json::to_vec(&foreign).unwrap()).unwrap();
    assert!(store
        .inspect()
        .unwrap()
        .issues
        .iter()
        .any(|issue| issue.contains("different libraries")));
}

#[test]
fn missing_current_manifest_recovers_from_history_and_preserves_unknown_fields() {
    let temp = tempdir().unwrap();
    let store = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let mut first = store.inspect().unwrap().current().unwrap().clone();
    first.extra.insert("future_envelope".into(), json!(true));
    first
        .definitions
        .extra
        .insert("custom_fields".into(), json!([]));
    first.definitions.statuses[0]
        .extra
        .insert("future_color".into(), json!("green"));
    fs::write(
        store
            .root()
            .join("history/library")
            .join(format!("{}.json", first.revision_id)),
        serde_json::to_vec(&first).unwrap(),
    )
    .unwrap();
    fs::remove_file(store.root().join("library.json")).unwrap();
    let reopened = LibraryStore::open(store.root()).unwrap();
    assert_eq!(reopened.inspect().unwrap().current().unwrap(), &first);
    let edited = reopened
        .edit(first.revision_id, LibraryDefinitions::new("Renamed"))
        .unwrap();
    assert_eq!(edited.extra, first.extra);
    assert_eq!(edited.definitions.extra, first.definitions.extra);
    assert_eq!(
        edited.definitions.statuses[0].extra,
        first.definitions.statuses[0].extra
    );
    assert!(store.root().join("library.json").exists());
}
