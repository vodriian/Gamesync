#![cfg(unix)]

use std::{fs, path::PathBuf, sync::Arc, thread};

use gamesync_desktop::storage::save_record;
use serde::{Deserialize, Serialize};
use tempfile::{tempdir, TempDir};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Record {
    title: String,
    revision: u32,
}

fn record(revision: u32) -> Record {
    Record {
        title: "A quiet evening".into(),
        revision,
    }
}

struct LibraryFiles {
    _root: TempDir,
    current: PathBuf,
    history: PathBuf,
}

impl LibraryFiles {
    fn new() -> Self {
        let root = tempdir().unwrap();
        let games = root.path().join("games");
        let history = root.path().join("history/game-1");
        fs::create_dir(&games).unwrap();
        fs::create_dir_all(&history).unwrap();
        Self {
            current: games.join("game-1.json"),
            history,
            _root: root,
        }
    }

    fn revision(&self, id: u32) -> PathBuf {
        self.history.join(format!("{id}.json"))
    }

    fn read_current(&self) -> Record {
        serde_json::from_slice(&fs::read(&self.current).unwrap()).unwrap()
    }
}

#[test]
fn edits_keep_previous_revisions_and_publish_identical_json() {
    let files = LibraryFiles::new();
    save_record(&files.current, &files.revision(1), &record(1)).unwrap();
    let original = fs::read(files.revision(1)).unwrap();
    save_record(&files.current, &files.revision(2), &record(2)).unwrap();

    assert_eq!(files.read_current(), record(2));
    assert_eq!(fs::read(files.revision(1)).unwrap(), original);
    assert_eq!(
        fs::read(&files.current).unwrap(),
        fs::read(files.revision(2)).unwrap()
    );
    assert_eq!(fs::read_dir(&files.history).unwrap().count(), 2);
}

#[test]
fn changed_content_cannot_reuse_a_revision_path() {
    let files = LibraryFiles::new();
    save_record(&files.current, &files.revision(1), &record(1)).unwrap();
    let error = save_record(&files.current, &files.revision(1), &record(2)).unwrap_err();
    assert!(error.to_string().contains("different content"));
    assert_eq!(files.read_current(), record(1));
    assert_eq!(
        fs::read(&files.current).unwrap(),
        fs::read(files.revision(1)).unwrap()
    );
}

#[test]
fn retry_recovers_a_revision_after_current_publication_failed() {
    let files = LibraryFiles::new();
    // A directory at the destination forces failure after revision publication.
    fs::create_dir(&files.current).unwrap();
    let error = save_record(&files.current, &files.revision(1), &record(1)).unwrap_err();
    assert!(error.to_string().contains("Revision retained"));
    let retained = fs::read(files.revision(1)).unwrap();

    fs::remove_dir(&files.current).unwrap();
    save_record(&files.current, &files.revision(1), &record(1)).unwrap();
    assert_eq!(files.read_current(), record(1));
    assert_eq!(fs::read(files.revision(1)).unwrap(), retained);
}

#[test]
fn missing_history_leaves_the_current_record_unchanged() {
    let files = LibraryFiles::new();
    save_record(&files.current, &files.revision(1), &record(1)).unwrap();
    assert!(save_record(
        &files.current,
        &files.history.join("missing/2.json"),
        &record(2)
    )
    .is_err());
    assert_eq!(files.read_current(), record(1));
}

#[test]
fn serialization_failure_does_not_change_files() {
    struct Invalid;
    impl Serialize for Invalid {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("invalid record"))
        }
    }
    let files = LibraryFiles::new();
    save_record(&files.current, &files.revision(1), &record(1)).unwrap();
    assert!(save_record(&files.current, &files.revision(2), &Invalid).is_err());
    assert_eq!(files.read_current(), record(1));
    assert!(!files.revision(2).exists());
}

#[test]
fn interrupted_staging_does_not_replace_a_valid_record() {
    let files = LibraryFiles::new();
    save_record(&files.current, &files.revision(1), &record(1)).unwrap();
    let leftover = files.current.with_file_name(".gamesync-interrupted.tmp");
    fs::write(&leftover, b"{\"title\":").unwrap();
    assert_eq!(files.read_current(), record(1));
    save_record(&files.current, &files.revision(2), &record(2)).unwrap();
    assert_eq!(files.read_current(), record(2));
    // A writer does not remove unknown files: another process may own them.
    assert!(leftover.exists());
}

#[test]
fn concurrent_writers_cannot_clobber_an_immutable_revision() {
    let files = Arc::new(LibraryFiles::new());
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = (1..=2)
        .map(|id| {
            let files = files.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                save_record(&files.current, &files.revision(1), &record(id))
            })
        })
        .collect();
    let successes = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .filter(Result::is_ok)
        .count();
    assert_eq!(successes, 1);
    assert_eq!(
        fs::read(&files.current).unwrap(),
        fs::read(files.revision(1)).unwrap()
    );
    assert_eq!(fs::read_dir(&files.history).unwrap().count(), 1);
}

#[test]
fn readers_see_complete_records_during_replacement() {
    let files = Arc::new(LibraryFiles::new());
    save_record(&files.current, &files.revision(0), &record(0)).unwrap();
    let reader_files = files.clone();
    let reader = thread::spawn(move || {
        for _ in 0..1_000 {
            let value = reader_files.read_current();
            assert_eq!(value.title, "A quiet evening");
            assert!(value.revision <= 16);
        }
    });
    for id in 1..=16 {
        save_record(&files.current, &files.revision(id), &record(id)).unwrap();
    }
    reader.join().unwrap();
    assert_eq!(files.read_current(), record(16));
}
