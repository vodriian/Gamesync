#![cfg(unix)]
use gamesync_desktop::{
    library::LibraryStore,
    library_reader::LibraryReader,
    record_store::RecordStore,
    records::{GameData, GameRevision},
};
use std::{fs, path::PathBuf};
use tempfile::{tempdir, TempDir};
use uuid::Uuid;

struct Fixture {
    temp: TempDir,
    root: PathBuf,
    cache: PathBuf,
    game: GameRevision,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = temp.path().join("Games.library");
        let cache = temp.path().join("cache");
        fs::create_dir(&cache).unwrap();
        LibraryStore::create(&root, "Evenings").unwrap();
        let game = RecordStore::open(&root)
            .unwrap()
            .create(GameData::new("Hades"))
            .unwrap();
        Self {
            temp,
            root,
            cache,
            game,
        }
    }
    fn reader(&self) -> LibraryReader {
        LibraryReader::open(&self.root, &self.cache).unwrap()
    }
    fn current(&self) -> PathBuf {
        self.root
            .join("games")
            .join(format!("{}.json", self.game.game_id))
    }
}

#[test]
fn refresh_loads_updates_and_keeps_source_files_unchanged() {
    let f = Fixture::new();
    let mut reader = f.reader();
    let before = fs::read(f.current()).unwrap();
    let loaded = reader.refresh().unwrap();
    assert_eq!(loaded.games.len(), 1);
    assert!(loaded.issues.is_empty());
    assert_eq!(fs::read(f.current()).unwrap(), before);
    let mut personal = f.game.game.personal.clone();
    personal.rating = Some(8);
    RecordStore::open(&f.root)
        .unwrap()
        .edit_personal(f.game.game_id, f.game.revision_id, personal)
        .unwrap();
    assert_eq!(
        reader.refresh().unwrap().games[0].game.personal.rating,
        Some(8)
    );
}

#[test]
fn partial_files_and_missing_sources_keep_valid_data_after_restart() {
    let f = Fixture::new();
    f.reader().refresh().unwrap();
    fs::write(f.current(), "{").unwrap();
    let mut reader = f.reader();
    let loaded = reader.refresh().unwrap();
    assert_eq!(loaded.games[0], f.game);
    assert!(!loaded.issues.is_empty());
    fs::remove_file(f.current()).unwrap();
    fs::remove_dir_all(f.root.join("history").join(f.game.game_id.to_string())).unwrap();
    let loaded = reader.refresh().unwrap();
    assert_eq!(loaded.games[0], f.game);
    assert!(!loaded.issues.is_empty());
    // A temporarily unavailable root also preserves the device-local index.
    fs::rename(&f.root, f.temp.path().join("Away.library")).unwrap();
    assert_eq!(reader.refresh().unwrap().games[0], f.game);
    assert_eq!(f.reader().refresh().unwrap().games[0], f.game);
}

#[test]
fn invalid_definitions_and_unknown_status_do_not_replace_valid_data() {
    let f = Fixture::new();
    let mut reader = f.reader();
    reader.refresh().unwrap();
    let mut personal = f.game.game.personal.clone();
    personal.status = "undefined".into();
    RecordStore::open(&f.root)
        .unwrap()
        .edit_personal(f.game.game_id, f.game.revision_id, personal)
        .unwrap();
    let loaded = reader.refresh().unwrap();
    assert_eq!(loaded.games[0], f.game);
    assert!(loaded
        .issues
        .iter()
        .any(|issue| issue.contains("unknown status")));
    fs::write(f.root.join("library.json"), "{").unwrap();
    let loaded = reader.refresh().unwrap();
    assert_eq!(loaded.manifest.definitions.name, "Evenings");
    assert_eq!(loaded.games[0], f.game);
}

#[test]
fn renamed_conflict_copies_are_detected_without_choosing_a_branch() {
    let f = Fixture::new();
    let mut reader = f.reader();
    reader.refresh().unwrap();
    let store = RecordStore::open(&f.root).unwrap();
    let local = store
        .edit_personal(
            f.game.game_id,
            f.game.revision_id,
            f.game.game.personal.clone(),
        )
        .unwrap();
    let mut remote = f.game.clone();
    remote.revision_id = Uuid::new_v4();
    remote.parents = vec![f.game.revision_id];
    fs::write(
        f.root.join("games/renamed-copy.json"),
        serde_json::to_vec(&remote).unwrap(),
    )
    .unwrap();
    let loaded = reader.refresh().unwrap();
    assert_eq!(loaded.games[0], f.game);
    assert!(loaded
        .issues
        .iter()
        .any(|issue| issue.contains("conflicting edits")));
    assert!(store
        .edit_personal(
            f.game.game_id,
            local.revision_id,
            f.game.game.personal.clone()
        )
        .is_err());
}

#[test]
fn tombstones_hide_games_and_deleting_the_index_rebuilds_it() {
    let f = Fixture::new();
    f.reader().refresh().unwrap();
    let store = RecordStore::open(&f.root).unwrap();
    store
        .set_archived(f.game.game_id, f.game.revision_id, true)
        .unwrap();
    assert!(f.reader().refresh().unwrap().games.is_empty());
    for entry in fs::read_dir(&f.cache).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    assert!(f.reader().refresh().unwrap().games.is_empty());
}

#[test]
fn media_and_cache_cannot_escape_their_expected_folders() {
    let f = Fixture::new();
    assert!(LibraryReader::open(&f.root, &f.root.join("media")).is_err());
    let outside = f.temp.path().join("outside.jpg");
    fs::write(&outside, "not an image").unwrap();
    std::os::unix::fs::symlink(outside, f.root.join("media/cover.jpg")).unwrap();
    let mut personal = f.game.game.personal.clone();
    personal.cover = Some("media/cover.jpg".into());
    RecordStore::open(&f.root)
        .unwrap()
        .edit_personal(f.game.game_id, f.game.revision_id, personal)
        .unwrap();
    let loaded = f.reader().refresh().unwrap();
    assert!(loaded.covers.is_empty());
    assert!(loaded
        .issues
        .iter()
        .any(|issue| issue.contains("outside media")));
}

#[test]
fn a_relocated_library_rebuilds_with_relative_covers() {
    let f = Fixture::new();
    fs::write(f.root.join("media/cover.jpg"), "fixture").unwrap();
    let mut personal = f.game.game.personal.clone();
    personal.cover = Some("media/cover.jpg".into());
    RecordStore::open(&f.root)
        .unwrap()
        .edit_personal(f.game.game_id, f.game.revision_id, personal)
        .unwrap();
    f.reader().refresh().unwrap();
    let moved = f.temp.path().join("Moved.library");
    fs::rename(&f.root, &moved).unwrap();
    let loaded = LibraryReader::open(&moved, &f.cache)
        .unwrap()
        .refresh()
        .unwrap();
    assert_eq!(loaded.games.len(), 1);
    assert!(loaded.covers[&f.game.game_id].starts_with(moved.canonicalize().unwrap()));
}
