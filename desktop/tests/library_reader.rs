#![cfg(any(unix, windows))]
use gamesync_desktop::{
    library::LibraryStore,
    library_reader::LibraryReader,
    record_store::RecordStore,
    records::{GameData, GameRevision, SteamData, SCHEMA_VERSION},
};
use std::{fs, path::PathBuf, time::Instant};
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
fn cache_cannot_be_inside_the_library() {
    let f = Fixture::new();
    assert!(LibraryReader::open(&f.root, &f.root.join("media")).is_err());
}

// Windows symlink creation needs developer mode or elevated privileges.
#[cfg(unix)]
#[test]
fn media_cannot_escape_its_expected_folder() {
    let f = Fixture::new();
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

/// Startup and idle scan timings for a library with history. Prints figures and
/// checks results only; it has no time limit. Sizes come from
/// `GAMESYNC_PROBE_GAMES` (1,000) and `GAMESYNC_PROBE_REVISIONS` (8).
///
/// `cargo test --release --test library_reader scan_timing -- --ignored --nocapture`
#[test]
#[ignore = "timing probe; prints figures"]
fn scan_timing_probe() {
    let size = |name: &str, default: usize| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let games = size("GAMESYNC_PROBE_GAMES", 1000);
    let revisions = size("GAMESYNC_PROBE_REVISIONS", 8).max(1);
    let temp = tempdir().unwrap();
    let root = temp.path().join("Probe.library");
    let cache = temp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    LibraryStore::create(&root, "Probe").unwrap();
    // Write history directly. The store flushes each file, which would make a
    // library of this size take minutes to prepare.
    for index in 0..games {
        let game_id = Uuid::new_v4();
        let history = root.join("history").join(game_id.to_string());
        fs::create_dir(&history).unwrap();
        let mut game = GameData::new(format!("Game {index:05}"));
        // About the size of a synced Steam record.
        game.steam = Some(SteamData {
            app_id: index as u32 + 1,
            description: Some("Probe description. ".repeat(160)),
            playtime_minutes: index as u32,
            owned: true,
            last_played: None,
            platform_minutes: Default::default(),
            wishlist: None,
            metadata: None,
            extra: Default::default(),
        });
        let mut record = GameRevision {
            schema_version: SCHEMA_VERSION,
            game_id,
            revision_id: Uuid::new_v4(),
            parents: Vec::new(),
            deleted: false,
            game,
            extra: Default::default(),
        };
        for revision in 0..revisions {
            if revision > 0 {
                record.parents = vec![record.revision_id];
                record.revision_id = Uuid::new_v4();
                record.game.personal.rating = Some((revision % 10 + 1) as u8);
            }
            let bytes = serde_json::to_vec_pretty(&record).unwrap();
            fs::write(history.join(format!("{}.json", record.revision_id)), &bytes).unwrap();
            if revision + 1 == revisions {
                fs::write(root.join("games").join(format!("{game_id}.json")), &bytes).unwrap();
            }
        }
    }
    let report = |label: &str, start: Instant| {
        eprintln!(
            "[probe] {label} ms={:.1}",
            start.elapsed().as_secs_f64() * 1000.
        );
    };
    eprintln!("[probe] games={games} revisions={revisions}");
    let start = Instant::now();
    let mut reader = LibraryReader::open(&root, &cache).unwrap();
    report("open_empty_index", start);
    let start = Instant::now();
    let loaded = reader.refresh().unwrap();
    report("first_scan", start);
    assert_eq!(loaded.games.len(), games);
    assert!(loaded.issues.is_empty(), "{:?}", loaded.issues.first());
    let start = Instant::now();
    assert_eq!(reader.refresh().unwrap().games.len(), games);
    report("repeat_scan", start);
    drop(reader);
    let start = Instant::now();
    let mut reader = LibraryReader::open(&root, &cache).unwrap();
    report("open_warm_index", start);
    let start = Instant::now();
    assert_eq!(reader.refresh().unwrap().games.len(), games);
    report("scan_after_reopen", start);
}
