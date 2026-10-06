use super::*;
use serde_json::json;
use std::cell::{Cell, RefCell};
const ACCOUNT: &str = "76561198000000000";
struct Source {
    fail_details: bool,
    extra_game: bool,
    details_calls: Cell<usize>,
    review_calls: Cell<usize>,
    cover_calls: Cell<usize>,
    banner_calls: Cell<usize>,
    tag_calls: Cell<usize>,
    setup_calls: Cell<usize>,
    /// The store omits game 43 from tag results, like a removed app.
    omit_tags_for_43: bool,
    last_played: i64,
    /// `None` makes the wishlist request fail.
    wishlist: RefCell<Option<Vec<u32>>>,
    /// Extra owned app IDs, to simulate a purchase.
    bought: RefCell<Vec<u32>>,
    cancel_after_details: Option<Cancellation>,
}
impl SteamSource for Source {
    fn owned(&self, _: &str, _: &str) -> Result<Vec<OwnedGame>> {
        let mut games = vec![OwnedGame {
            appid: 42,
            name: "Test game".into(),
            playtime_forever: 60,
            rtime_last_played: self.last_played,
            playtime_linux_forever: 40,
            playtime_deck_forever: 15,
            ..Default::default()
        }];
        games.extend(self.bought.borrow().iter().map(|&appid| OwnedGame {
            appid,
            name: format!("Bought {appid}"),
            ..Default::default()
        }));
        if self.extra_game {
            games.push(OwnedGame {
                appid: 43,
                name: "New game".into(),
                ..Default::default()
            });
        }
        Ok(games)
    }
    fn store_tags(&self, ids: &[u32]) -> Result<BTreeMap<u32, Vec<u32>>> {
        self.tag_calls.set(self.tag_calls.get() + 1);
        // Tag 99 has no name in the tag list and must be skipped.
        Ok(ids
            .iter()
            .filter(|id| !(self.omit_tags_for_43 && **id == 43))
            .map(|id| (*id, vec![20, 99, 10]))
            .collect())
    }
    fn tag_names(&self) -> Result<BTreeMap<u32, String>> {
        Ok(BTreeMap::from([(10, "Puzzle".into()), (20, "Cozy".into())]))
    }
    fn wishlist(&self, _: &str) -> Result<Vec<WishlistItem>> {
        let ids = self.wishlist.borrow().clone().context("Private wishlist")?;
        Ok(ids
            .into_iter()
            .enumerate()
            .map(|(priority, appid)| WishlistItem {
                appid,
                priority: priority as u32,
                date_added: 1_700_000_000,
            })
            .collect())
    }
    fn store_names(&self, ids: &[u32]) -> Result<BTreeMap<u32, String>> {
        Ok(ids.iter().map(|id| (*id, format!("Wish {id}"))).collect())
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
    fn banner(&self, _: u32) -> Result<Vec<u8>> {
        self.banner_calls.set(self.banner_calls.get() + 1);
        Ok(banner_jpeg())
    }
    fn deck_report(&self, _: u32) -> Result<serde_json::Value> {
        self.setup_calls.set(self.setup_calls.get() + 1);
        Ok(
            json!({"success": 1, "results": {"resolved_category": 2, "resolved_items": [
            {"display_type": 3, "loc_token": "#SteamDeckVerified_TestResult_InterfaceTextIsNotLegible"}]}}),
        )
    }
    fn store_categories(&self, _: u32) -> Result<serde_json::Value> {
        Ok(json!({"categories": [{"id": 28}]}))
    }
}
/// A small landscape JPEG with varied pixels, so it passes the placeholder check.
fn banner_jpeg() -> Vec<u8> {
    let image = image::RgbImage::from_fn(92, 43, |x, y| image::Rgb([x as u8 * 2, y as u8 * 5, 90]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Jpeg,
        )
        .unwrap();
    bytes
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
        banner_calls: Cell::new(0),
        tag_calls: Cell::new(0),
        setup_calls: Cell::new(0),
        omit_tags_for_43: false,
        last_played: 1_700_000_000,
        wishlist: RefCell::new(Some(Vec::new())),
        bought: RefCell::new(Vec::new()),
        cancel_after_details: None,
    }
}
fn current(library: &LibraryStore, app_id: u32) -> crate::records::GameRevision {
    let store = RecordStore::open(library.root()).unwrap();
    let id = uuid::Uuid::new_v5(
        &library.inspect().unwrap().current().unwrap().library_id,
        format!("steam:{app_id}").as_bytes(),
    );
    store.inspect(id).unwrap().current().unwrap().clone()
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
            source.cover_calls.get(),
            source.banner_calls.get()
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(
        (
            source.details_calls.get(),
            source.review_calls.get(),
            source.cover_calls.get(),
            source.banner_calls.get()
        ),
        (1, 1, 1, 1)
    );
    source.extra_game = true;
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(
        (
            source.details_calls.get(),
            source.review_calls.get(),
            source.cover_calls.get(),
            source.banner_calls.get()
        ),
        (2, 2, 2, 2)
    );
    // One batched tag request per sync that has games without tags.
    assert_eq!(source.tag_calls.get(), 2);
}

#[test]
fn sync_stores_activity_and_steam_tags_apart_from_personal_tags() {
    let (_temp, library) = library();
    let mut source = source(false);
    run(&library, &source);
    let game = current(&library, 42);
    let store = RecordStore::open(library.root()).unwrap();
    let mut personal = game.game.personal.clone();
    personal.tags = vec!["Weekend".into()];
    store
        .edit_personal(game.game_id, game.revision_id, personal)
        .unwrap();

    let saved = current(&library, 42);
    let steam = saved.game.steam.as_ref().unwrap();
    assert_eq!(steam.last_played, Some(1_700_000_000));
    assert_eq!(
        (steam.platform_minutes.linux, steam.platform_minutes.deck),
        (40, 15)
    );
    let metadata = steam.metadata.as_ref().unwrap();
    assert_eq!(metadata.tags, ["Cozy", "Puzzle"]);
    assert!(metadata.tags_complete);

    // An unchanged sync writes nothing; new activity updates only Steam data.
    run(&library, &source);
    assert_eq!(current(&library, 42).revision_id, saved.revision_id);
    source.last_played = 1_800_000_000;
    run(&library, &source);
    let updated = current(&library, 42);
    assert_eq!(
        updated.game.steam.as_ref().unwrap().last_played,
        Some(1_800_000_000)
    );
    assert_eq!(updated.game.personal.tags, ["Weekend"]);
    assert_eq!(
        updated
            .game
            .steam
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .tags,
        ["Cozy", "Puzzle"]
    );
}

#[test]
fn a_game_missing_from_tag_results_retries_on_the_next_sync() {
    let (_temp, library) = library();
    let mut source = source(false);
    source.extra_game = true;
    source.omit_tags_for_43 = true;
    assert_eq!(run(&library, &source).failures, 1);
    let tagged = |library: &LibraryStore| {
        current(library, 43)
            .game
            .steam
            .unwrap()
            .metadata
            .is_some_and(|m| m.tags_complete)
    };
    assert!(!tagged(&library));
    source.omit_tags_for_43 = false;
    assert_eq!(run(&library, &source).failures, 0);
    assert!(tagged(&library));
    assert_eq!(source.tag_calls.get(), 2);
}

#[test]
fn tag_parsers_order_by_weight_and_skip_failed_apps() {
    let tags = client::parse_store_tags(&json!({"response":{"store_items":[
        {"appid":42,"success":1,"tags":[{"tagid":10,"weight":5},{"tagid":20,"weight":9}]},
        {"appid":43,"success":1},
        {"appid":44,"success":2}
    ]}}))
    .unwrap();
    assert_eq!(tags[&42], [20, 10]);
    assert!(tags[&43].is_empty());
    assert!(!tags.contains_key(&44));
    assert!(client::parse_store_tags(&json!({"response":{}})).is_err());
    assert!(client::parse_tag_names(&json!({"response":{"tags":[]}})).is_err());
    assert_eq!(
        client::parse_tag_names(&json!({"response":{"tags":[{"tagid":10,"name":"Puzzle"}]}}))
            .unwrap()[&10],
        "Puzzle"
    );
}

#[test]
fn wishlist_games_import_leave_and_become_owned_without_losing_edits() {
    let (_temp, library) = library();
    let source = source(false);
    // Game 42 is owned, so its wishlist entry is ignored.
    *source.wishlist.borrow_mut() = Some(vec![99, 42]);
    assert_eq!(run(&library, &source).failures, 0);
    let wish = current(&library, 99);
    assert_eq!(wish.game.title, "Wish 99");
    assert_eq!(wish.game.personal.status, "wanted");
    assert!(wish.game.wishlisted());
    assert!(!current(&library, 42).game.wishlisted());
    // Wishlist games get the normal enrichment stages.
    assert!(
        wish.game
            .steam
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .tags_complete
    );

    let store = RecordStore::open(library.root()).unwrap();
    let mut personal = wish.game.personal.clone();
    personal.notes = "Wait for a sale".into();
    store
        .edit_personal(wish.game_id, wish.revision_id, personal)
        .unwrap();

    // A failed request marks nothing as removed.
    *source.wishlist.borrow_mut() = None;
    assert_eq!(run(&library, &source).failures, 1);
    let entry = |library: &LibraryStore| current(library, 99).game.steam.unwrap().wishlist;
    assert!(!entry(&library).unwrap().removed);

    // Leaving the wishlist keeps the record, marked as removed.
    *source.wishlist.borrow_mut() = Some(vec![100]);
    assert_eq!(run(&library, &source).failures, 0);
    assert!(entry(&library).unwrap().removed);
    assert!(current(&library, 99).game.wishlisted());

    // Buying it keeps one record and the personal note.
    source.bought.borrow_mut().push(99);
    run(&library, &source);
    let bought = current(&library, 99);
    assert_eq!(bought.game_id, wish.game_id);
    assert!(!bought.game.wishlisted());
    assert!(bought.game.steam.as_ref().unwrap().owned);
    assert_eq!(bought.game.personal.notes, "Wait for a sale");
}

#[test]
fn wishlist_parser_rejects_empty_private_and_duplicate_lists() {
    assert!(client::parse_wishlist(&json!({"response":{}})).is_err());
    assert!(client::parse_wishlist(&json!({"response":{"items":[]}})).is_err());
    assert!(client::parse_wishlist(
        &json!({"response":{"items":[{"appid":1,"priority":0},{"appid":1,"priority":1}]}})
    )
    .is_err());
    let items = client::parse_wishlist(
        &json!({"response":{"items":[{"appid":620,"priority":2,"date_added":1700000000}]}}),
    )
    .unwrap();
    assert_eq!(
        items,
        [WishlistItem {
            appid: 620,
            priority: 2,
            date_added: 1_700_000_000
        }]
    );
}

#[test]
fn sync_saves_steam_deck_evidence_once_and_assesses_it() {
    use crate::suitability::{BestOn, ControllerSupport, DeckRating};
    let (_temp, library) = library();
    let source = source(false);
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(source.setup_calls.get(), 1);
    let record = current(&library, 42);
    let evidence = record
        .game
        .steam
        .as_ref()
        .and_then(|steam| steam.metadata.as_ref()?.setup.clone())
        .unwrap();
    assert_eq!(evidence.deck, DeckRating::Playable);
    assert_eq!(evidence.controller, ControllerSupport::Full);
    // Personal data is untouched, and the result is calculated, not saved.
    assert!(record.game.suitability.is_none());
    assert_eq!(record.game.personal.setup_preference, None);
    let assessment = crate::suitability::assessment(&record.game).unwrap();
    assert_eq!(assessment.recommendation(), BestOn::Both);
    assert!(assessment
        .steam_deck
        .reason
        .contains("Interface text is not legible"));
}

#[test]
fn sync_saves_the_landscape_cover_once_and_a_personal_cover_hides_it() {
    let (_temp, library) = library();
    let source = source(false);
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(run(&library, &source).failures, 0);
    assert_eq!(source.banner_calls.get(), 1);
    let record = current(&library, 42);
    let banner = record.game.banner().unwrap().clone();
    assert!(banner.starts_with("media/steam-42-banner-"));
    assert!(library.root().join(&banner).is_file());
    // The portrait cover stays separate from the banner.
    assert_ne!(record.game.cover(), Some(&banner));
    let mut game = record.game.clone();
    game.personal.cover = Some("media/custom.jpg".into());
    assert_eq!(game.banner(), None);
}

#[test]
fn banner_validation_rejects_portrait_artwork() {
    assert!(client::validate_banner(&banner_jpeg()).is_ok());
    let portrait = include_bytes!("../../fixtures/covers/620.jpg");
    assert!(client::validate_cover(portrait).is_ok());
    assert!(client::validate_banner(portrait).is_err());
}
