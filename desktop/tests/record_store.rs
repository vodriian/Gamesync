#![cfg(unix)]

use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::{Arc, Barrier},
    thread,
};

use gamesync_desktop::{
    record_store::RecordStore,
    records::{GameData, GameRevision, SteamData},
};
use serde_json::json;
use tempfile::{tempdir, TempDir};
use uuid::Uuid;

struct Library {
    root: TempDir,
    store: RecordStore,
    first: GameRevision,
}

impl Library {
    fn new() -> Self {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("games")).unwrap();
        fs::create_dir(root.path().join("history")).unwrap();
        let store = RecordStore::open(root.path()).unwrap();
        let mut game = GameData::new("Hades");
        game.steam = Some(SteamData {
            metadata: None,
            app_id: 1145360,
            description: Some("Store description".into()),
            playtime_minutes: 90,
            owned: true,
            extra: Default::default(),
        });
        let first = store.create(game).unwrap();
        Self { root, store, first }
    }

    fn current(&self) -> PathBuf {
        self.root
            .path()
            .join("games")
            .join(format!("{}.json", self.first.game_id))
    }
    fn history(&self, id: Uuid) -> PathBuf {
        self.root
            .path()
            .join("history")
            .join(self.first.game_id.to_string())
            .join(format!("{id}.json"))
    }

    fn branch(&self, notes: &str) -> GameRevision {
        let mut record = self.first.clone();
        record.revision_id = Uuid::new_v4();
        record.parents = vec![self.first.revision_id];
        record.game.personal.notes = notes.into();
        record
    }

    fn conflict_copy(&self, record: &GameRevision) -> PathBuf {
        let path = self.current().with_file_name(format!(
            "{} (computer conflicted copy).json",
            self.first.game_id
        ));
        fs::write(&path, serde_json::to_vec(record).unwrap()).unwrap();
        path
    }
}

#[test]
fn edits_preserve_provider_data_and_reject_stale_parents() {
    let library = Library::new();
    let first = &library.first;
    let mut personal = first.game.personal.clone();
    personal.rating = Some(9);
    personal.description = Some(String::new());
    let edited = library
        .store
        .edit_personal(first.game_id, first.revision_id, personal.clone())
        .unwrap();
    assert_eq!(edited.parents, vec![first.revision_id]);
    assert_eq!(edited.game.steam, first.game.steam);
    assert_eq!(edited.game.description(), Some(""));
    let before = fs::read(library.current()).unwrap();
    assert!(library
        .store
        .edit_personal(first.game_id, first.revision_id, personal)
        .is_err());
    assert_eq!(fs::read(library.current()).unwrap(), before);
}

#[test]
fn conflict_resolution_retains_both_branches_and_acknowledges_all_heads() {
    let library = Library::new();
    let first = &library.first;
    let local = library
        .store
        .edit_personal(
            first.game_id,
            first.revision_id,
            first.game.personal.clone(),
        )
        .unwrap();
    let remote = library.branch("Played on my other computer");
    let copy = library.conflict_copy(&remote);
    let copy_bytes = fs::read(&copy).unwrap();
    let snapshot = library.store.inspect(first.game_id).unwrap();
    assert!(snapshot.has_conflict());
    assert!(snapshot.current().is_none());
    assert_eq!(
        snapshot.heads,
        BTreeSet::from([local.revision_id, remote.revision_id])
    );
    assert!(library
        .store
        .edit_personal(
            first.game_id,
            local.revision_id,
            first.game.personal.clone()
        )
        .is_err());

    let resolved = library
        .store
        .resolve(first.game_id, &snapshot.heads, remote.revision_id)
        .unwrap();
    assert_eq!(
        resolved.parents.into_iter().collect::<BTreeSet<_>>(),
        snapshot.heads
    );
    assert_eq!(resolved.game.personal.notes, remote.game.personal.notes);
    assert_eq!(fs::read(&copy).unwrap(), copy_bytes);
    assert!(library.history(local.revision_id).exists());
    assert!(library.history(remote.revision_id).exists());
    // Once retained in history, removing a redundant sync copy loses no branch.
    fs::remove_file(copy).unwrap();
    let reopened = RecordStore::open(library.root.path())
        .unwrap()
        .inspect(first.game_id)
        .unwrap();
    assert_eq!(
        reopened.current().unwrap().revision_id,
        resolved.revision_id
    );
    assert_eq!(reopened.revisions.len(), 4);
}

#[test]
fn a_new_branch_invalidates_a_pending_resolution() {
    let library = Library::new();
    let first = &library.first;
    let local = library
        .store
        .edit_personal(
            first.game_id,
            first.revision_id,
            first.game.personal.clone(),
        )
        .unwrap();
    library.conflict_copy(&library.branch("Remote one"));
    let reviewed = library.store.inspect(first.game_id).unwrap().heads;
    let later = library.branch("Remote two");
    fs::write(
        library.history(later.revision_id),
        serde_json::to_vec(&later).unwrap(),
    )
    .unwrap();
    let before = fs::read(library.current()).unwrap();
    assert!(library
        .store
        .resolve(first.game_id, &reviewed, local.revision_id)
        .is_err());
    assert_eq!(fs::read(library.current()).unwrap(), before);
}

#[test]
fn missing_parent_blocks_writes_until_it_arrives() {
    let library = Library::new();
    let parent = library.branch("Delayed parent");
    let mut child = library.branch("Arrived first");
    child.parents = vec![parent.revision_id];
    library.conflict_copy(&child);
    let incomplete = library.store.inspect(library.first.game_id).unwrap();
    assert!(incomplete
        .issues
        .iter()
        .any(|issue| issue.contains("Missing revision parent")));
    assert!(incomplete.current().is_none());
    assert!(library
        .store
        .edit_personal(
            library.first.game_id,
            child.revision_id,
            child.game.personal.clone()
        )
        .is_err());
    fs::write(
        library.history(parent.revision_id),
        serde_json::to_vec(&parent).unwrap(),
    )
    .unwrap();
    let complete = library.store.inspect(library.first.game_id).unwrap();
    assert!(complete.issues.is_empty());
    assert_eq!(complete.current().unwrap().revision_id, child.revision_id);
}

#[test]
fn partial_and_newer_files_block_edits_without_losing_valid_history() {
    let library = Library::new();
    for bytes in [
        b"{\"schema_version\":".as_slice(),
        br#"{"schema_version":2,"new_shape":true}"#,
    ] {
        fs::write(library.current(), bytes).unwrap();
        let snapshot = library.store.inspect(library.first.game_id).unwrap();
        assert!(!snapshot.issues.is_empty());
        assert!(snapshot.revisions.contains_key(&library.first.revision_id));
        assert!(library
            .store
            .edit_personal(
                library.first.game_id,
                library.first.revision_id,
                library.first.game.personal.clone()
            )
            .is_err());
        assert_eq!(fs::read(library.current()).unwrap(), bytes);
    }
}

#[test]
fn supported_unknown_fields_survive_personal_edits() {
    let library = Library::new();
    let mut record = library.first.clone();
    record
        .extra
        .insert("future_envelope".into(), json!({"value":1}));
    record
        .game
        .extra
        .insert("analysis".into(), json!({"energy":"low"}));
    record
        .game
        .personal
        .extra
        .insert("custom_field".into(), json!(["cozy"]));
    record
        .game
        .steam
        .as_mut()
        .unwrap()
        .extra
        .insert("future_store".into(), json!(true));
    let bytes = serde_json::to_vec(&record).unwrap();
    fs::write(library.current(), &bytes).unwrap();
    fs::write(library.history(record.revision_id), &bytes).unwrap();
    let edited = library
        .store
        .edit_personal(
            record.game_id,
            record.revision_id,
            library.first.game.personal.clone(),
        )
        .unwrap();
    assert_eq!(edited.extra, record.extra);
    assert_eq!(edited.game.extra, record.game.extra);
    assert_eq!(edited.game.personal.extra, record.game.personal.extra);
    assert_eq!(edited.game.steam, record.game.steam);
}

#[test]
fn a_missing_current_file_is_not_a_deletion_and_archive_is_reversible() {
    let library = Library::new();
    fs::remove_file(library.current()).unwrap();
    fs::write(library.current().with_extension("tmp"), b"{").unwrap();
    let snapshot = library.store.inspect(library.first.game_id).unwrap();
    assert!(!snapshot.current().unwrap().deleted);
    let archived = library
        .store
        .set_archived(library.first.game_id, library.first.revision_id, true)
        .unwrap();
    assert!(archived.deleted);
    assert!(library
        .store
        .edit_personal(
            archived.game_id,
            archived.revision_id,
            archived.game.personal.clone()
        )
        .is_err());
    let restored = library
        .store
        .set_archived(archived.game_id, archived.revision_id, false)
        .unwrap();
    assert!(!restored.deleted);
    assert_eq!(restored.game, library.first.game);
}

#[test]
fn duplicate_revision_content_and_cycles_block_writes() {
    let library = Library::new();
    let mut duplicate = library.first.clone();
    duplicate.game.title = "Different content, same ID".into();
    let copy = library.conflict_copy(&duplicate);
    assert!(library
        .store
        .inspect(duplicate.game_id)
        .unwrap()
        .issues
        .iter()
        .any(|issue| issue.contains("different content")));
    fs::remove_file(copy).unwrap();
    let mut a = library.branch("A");
    let mut b = library.branch("B");
    a.parents = vec![b.revision_id];
    b.parents = vec![a.revision_id];
    for record in [&a, &b] {
        fs::write(
            library.history(record.revision_id),
            serde_json::to_vec(record).unwrap(),
        )
        .unwrap();
    }
    let snapshot = library.store.inspect(a.game_id).unwrap();
    assert!(snapshot.issues.iter().any(|issue| issue.contains("cycle")));
    assert!(snapshot.current().is_none());
}

#[test]
fn invalid_values_and_unsafe_cover_paths_never_publish() {
    let library = Library::new();
    let before = fs::read(library.current()).unwrap();
    for cover in [
        "../cover.jpg",
        "/media/cover.jpg",
        "media/../cover.jpg",
        "media\\cover.jpg",
        "C:/cover.jpg",
    ] {
        let mut personal = library.first.game.personal.clone();
        personal.cover = Some(cover.into());
        assert!(library
            .store
            .edit_personal(library.first.game_id, library.first.revision_id, personal)
            .is_err());
    }
    let mut personal = library.first.game.personal.clone();
    personal.rating = Some(11);
    assert!(library
        .store
        .edit_personal(library.first.game_id, library.first.revision_id, personal)
        .is_err());
    let mut personal = library.first.game.personal.clone();
    personal.extra.insert("rating".into(), json!(5));
    assert!(library
        .store
        .edit_personal(library.first.game_id, library.first.revision_id, personal)
        .is_err());
    assert_eq!(fs::read(library.current()).unwrap(), before);
}

#[test]
fn concurrent_local_edits_do_not_both_accept_the_same_parent() {
    let library = Library::new();
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|index| {
            let barrier = barrier.clone();
            let root = library.root.path().to_path_buf();
            let first = library.first.clone();
            thread::spawn(move || {
                let store = RecordStore::open(root).unwrap();
                let mut personal = first.game.personal;
                personal.notes = format!("Edit {index}");
                barrier.wait();
                store.edit_personal(first.game_id, first.revision_id, personal)
            })
        })
        .collect();
    assert_eq!(
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(Result::is_ok)
            .count(),
        1
    );
    let snapshot = library.store.inspect(library.first.game_id).unwrap();
    assert!(snapshot.current().is_some());
    assert_eq!(snapshot.revisions.len(), 2);
}

#[test]
fn oversized_records_and_misplaced_identity_are_blocked() {
    let library = Library::new();
    let before = fs::read(library.current()).unwrap();
    let mut personal = library.first.game.personal.clone();
    personal.notes = "x".repeat(1024 * 1024);
    assert!(library
        .store
        .edit_personal(library.first.game_id, library.first.revision_id, personal)
        .is_err());
    assert_eq!(fs::read(library.current()).unwrap(), before);
    let mut misplaced = library.first.clone();
    misplaced.game_id = Uuid::new_v4();
    let copy = library.conflict_copy(&misplaced);
    assert!(library
        .store
        .inspect(library.first.game_id)
        .unwrap()
        .issues
        .iter()
        .any(|issue| issue.contains("identity")));
    fs::write(copy, "x".repeat(1024 * 1024 + 1)).unwrap();
    assert!(library
        .store
        .inspect(library.first.game_id)
        .unwrap()
        .issues
        .iter()
        .any(|issue| issue.contains("1 MiB")));
}
