use super::*;
use serde_json::json;
use std::cell::Cell;
const ACCOUNT: &str = "76561198000000000";
struct Source {
    fail_details: bool,
    extra_game: bool,
    details_calls: Cell<usize>,
    review_calls: Cell<usize>,
    cover_calls: Cell<usize>,
    cancel_after_details: Option<Cancellation>,
}
impl SteamSource for Source {
    fn owned(&self, _: &str, _: &str) -> Result<Vec<OwnedGame>> {
        let mut games = vec![OwnedGame {
            appid: 42,
            name: "Test game".into(),
            playtime_forever: 60,
        }];
        if self.extra_game {
            games.push(OwnedGame {
                appid: 43,
                name: "New game".into(),
                playtime_forever: 0,
            });
        }
        Ok(games)
    }
    fn details(&self, _: u32) -> Result<serde_json::Value> {
        self.details_calls.set(self.details_calls.get() + 1);
        if let Some(cancel) = &self.cancel_after_details {
            cancel.store(true, Ordering::Relaxed);
        }
        if self.fail_details {
            anyhow::bail!("Offline");
        }
        Ok(
            json!({"short_description":"A calm test game.", "genres":[{"description":"Puzzle"}], "release_date":{"date":"12 Sep, 2020"}}),
        )
    }
    fn reviews(&self, _: u32) -> Result<serde_json::Value> {
        self.review_calls.set(self.review_calls.get() + 1);
        Ok(json!({"total_reviews":10,"total_positive":9,"review_score_desc":"Positive"}))
    }
    fn cover(&self, _: u32) -> Result<Vec<u8>> {
        self.cover_calls.set(self.cover_calls.get() + 1);
        Ok(include_bytes!("../../fixtures/covers/620.jpg").to_vec())
    }
}
fn library() -> (tempfile::TempDir, LibraryStore) {
    let temp = tempfile::tempdir().unwrap();
    let store = LibraryStore::create(temp.path().join("library"), "Tests").unwrap();
    let rev = store.inspect().unwrap().current().unwrap().revision_id;
    store.bind_steam(rev, ACCOUNT).unwrap();
    (temp, store)
}
fn source(fail_details: bool) -> Source {
    Source {
        fail_details,
        extra_game: false,
        details_calls: Cell::new(0),
        review_calls: Cell::new(0),
        cover_calls: Cell::new(0),
        cancel_after_details: None,
    }
}
fn run(store: &LibraryStore, source: &Source) -> SyncProgress {
    run_sync(
        store.root(),
        ACCOUNT,
        "dummy",
        Arc::new(AtomicBool::new(false)),
        source,
        Duration::ZERO,
        &mut |_| {},
    )
    .unwrap()
}
#[test]
fn partial_sync_retries_only_failed_stages_and_preserves_personal_data() {
    let (_temp, library) = library();
    let failed = source(true);
    assert_eq!(run(&library, &failed).failures, 1);
    let game = library
        .import_steam(ACCOUNT, &failed.owned("", "").unwrap()[0])
        .unwrap();
    let mut personal = game.game.personal.clone();
    personal.notes = "My note".into();
    personal.rating = Some(10);
    personal.description = Some("My description".into());
    personal.cover = Some("media/custom.jpg".into());
    let store = RecordStore::open(library.root()).unwrap();
    store
        .edit_personal(game.game_id, game.revision_id, personal.clone())
        .unwrap();
    let good = source(false);
    assert_eq!(run(&library, &good).failures, 0);
    assert_eq!(good.details_calls.get(), 1);
    let snapshot = store.inspect(game.game_id).unwrap();
    let saved = snapshot.current().unwrap();
    assert_eq!(saved.game.personal, personal);
    let metadata = saved
        .game
        .steam
        .as_ref()
        .unwrap()
        .metadata
        .as_ref()
        .unwrap();
    assert!(metadata.details_complete && metadata.cover_complete && metadata.reviews_complete);
    assert_eq!(metadata.review_percent, Some(90));
    assert_eq!(metadata.release_year.as_deref(), Some("2020"));
    assert_eq!(saved.game.cover().unwrap(), "media/custom.jpg");
    let revision = saved.revision_id;
    run(&library, &good);
    assert_eq!(good.details_calls.get(), 1);
    assert_eq!(
        store
            .inspect(game.game_id)
            .unwrap()
            .current()
            .unwrap()
            .revision_id,
        revision
    );
}
#[test]
fn cancellation_keeps_completed_stages_and_a_later_sync_resumes() {
    let (_temp, library) = library();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut first = source(false);
    first.cancel_after_details = Some(cancel.clone());
    let result = run_sync(
        library.root(),
        ACCOUNT,
        "dummy",
        cancel,
        &first,
        Duration::ZERO,
        &mut |_| {},
    )
    .unwrap();
    assert!(result.cancelled);
    let game = library
        .import_steam(ACCOUNT, &first.owned("", "").unwrap()[0])
        .unwrap();
    let metadata = game.game.steam.as_ref().unwrap().metadata.as_ref().unwrap();
    assert!(metadata.details_complete);
    assert!(!metadata.reviews_complete);
    let next = source(false);
    assert_eq!(run(&library, &next).failures, 0);
    assert_eq!(next.details_calls.get(), 0);
}
#[test]
fn cancelled_before_import_and_simultaneous_sync_make_no_changes() {
    let (_temp, library) = library();
    let cancel = Arc::new(AtomicBool::new(true));
    let result = run_sync(
        library.root(),
        ACCOUNT,
        "dummy",
        cancel,
        &source(false),
        Duration::ZERO,
        &mut |_| {},
    )
    .unwrap();
    assert!(result.cancelled);
    assert_eq!(result.imported, 0);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(library.root().join(".steam-sync.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(run_sync(
        library.root(),
        ACCOUNT,
        "dummy",
        Arc::new(AtomicBool::new(false)),
        &source(false),
        Duration::ZERO,
        &mut |_| {}
    )
    .is_err());
}
#[test]
fn owned_parser_distinguishes_empty_private_partial_and_duplicates() {
    assert!(client::parse_owned(json!({"response":{"game_count":0}}))
        .unwrap()
        .is_empty());
    assert!(client::parse_owned(json!({"response":{}})).is_err());
    assert!(client::parse_owned(
        json!({"response":{"game_count":2,"games":[{"appid":42,"name":"Test"}]}})
    )
    .is_err());
    assert!(client::parse_owned(json!({"response":{"game_count":2,"games":[{"appid":42,"name":"Test"},{"appid":42,"name":"Test"}]}})).is_err());
}

#[test]
fn invalid_cover_does_not_count_as_a_completed_download() {
    assert!(client::validate_cover(&[0xff, 0xd8, 0xff]).is_err());
    assert!(client::validate_cover(include_bytes!("../../fixtures/covers/620.jpg")).is_ok());
}

#[test]
fn cover_refresh_uses_a_new_path_and_preserves_personal_data_on_failure() {
    let (_temp, library) = library();
    let game = library
        .import_steam(ACCOUNT, &source(false).owned("", "").unwrap()[0])
        .unwrap();
    let store = RecordStore::open(library.root()).unwrap();
    let mut personal = game.game.personal.clone();
    personal.notes = "Keep this note".into();
    personal.cover = Some("media/custom.jpg".into());
    store
        .edit_personal(game.game_id, game.revision_id, personal.clone())
        .unwrap();
    let bytes = include_bytes!("../../fixtures/covers/620.jpg");
    let first = covers::save(library.root(), game.game_id, 42, bytes).unwrap();
    let second = covers::save(library.root(), game.game_id, 42, bytes).unwrap();
    let path = |r: &crate::records::GameRevision| {
        r.game
            .steam
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .cover
            .clone()
            .unwrap()
    };
    assert_ne!(path(&first), path(&second));
    assert!(library.root().join(path(&first)).is_file());
    assert!(library.root().join(path(&second)).is_file());
    assert_eq!(second.game.personal, personal);
    assert!(covers::save(library.root(), game.game_id, 42, b"broken").is_err());
    assert_eq!(
        store
            .inspect(game.game_id)
            .unwrap()
            .current()
            .unwrap()
            .revision_id,
        second.revision_id
    );
}

#[test]
fn repeat_sync_does_not_download_cached_metadata_or_covers() {
    let (_temp, library) = library();
    let mut source = source(false);
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(
        (
            source.details_calls.get(),
            source.review_calls.get(),
            source.cover_calls.get()
        ),
        (1, 1, 1)
    );
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(
        (
            source.details_calls.get(),
            source.review_calls.get(),
            source.cover_calls.get()
        ),
        (1, 1, 1)
    );
    source.extra_game = true;
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(
        (
            source.details_calls.get(),
            source.review_calls.get(),
            source.cover_calls.get()
        ),
        (2, 2, 2)
    );
}
