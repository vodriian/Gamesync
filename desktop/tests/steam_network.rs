//! Optional smoke check. No account or API key is used.
use gamesync_desktop::steam::SteamClient;
#[test]
#[ignore = "Reads public Steam endpoints; keep network separate from deterministic tests"]
fn public_store_metadata_and_portrait_are_available() {
    let client = SteamClient::new().unwrap();
    let details = client.details(620).unwrap();
    assert!(details["short_description"].as_str().is_some());
    let reviews = client.reviews(620).unwrap();
    assert!(reviews["total_reviews"].as_u64().is_some());
    assert!(!client.cover(620).unwrap().is_empty());
}

#[test]
#[ignore = "Reads public Steam artwork; no account or API key is used"]
fn battlefield_cover_is_real_artwork() {
    // The legacy URL can return a valid, flat gray JPEG for this game.
    let bytes = SteamClient::new().unwrap().cover(2807960).unwrap();
    gamesync_desktop::steam::client::validate_cover(&bytes).unwrap();
    if let Some(path) = std::env::var_os("GAMESYNC_COVER_PREVIEW") {
        std::fs::write(path, bytes).unwrap();
    }
}

#[test]
#[ignore = "Reads public Steam tags; no account or API key is used"]
fn store_tags_resolve_to_names_and_skip_unknown_apps() {
    let client = SteamClient::new().unwrap();
    let names = client.tag_names().unwrap();
    // App 1 does not exist and must stay retryable, not complete with no tags.
    let tags = client.store_tags(&[620, 1]).unwrap();
    assert!(!tags.contains_key(&1));
    let portal = &tags[&620];
    assert!(!portal.is_empty() && portal.len() <= 20);
    assert!(portal.iter().any(|id| names.contains_key(id)));
}

#[test]
#[ignore = "Reads public Steam prices; no account or API key is used"]
fn store_quotes_include_paid_free_and_missing_apps() {
    use gamesync_desktop::prices::{Quote, DEFAULT_COUNTRY};
    let quotes = SteamClient::new()
        .unwrap()
        .quotes(&[1091500, 570, 1], DEFAULT_COUNTRY)
        .unwrap();
    assert!(matches!(&quotes[&1091500], Quote::Price(p) if p.formatted_final.contains('₴')));
    assert_eq!(quotes[&570], Quote::Free);
    assert!(!quotes.contains_key(&1));
}

#[test]
#[ignore = "Reads public Steam store data; no account or API key is used"]
fn resync_fills_details_reviews_and_tags_for_one_game() {
    use gamesync_desktop::{library::LibraryStore, record_store::RecordStore, steam::OwnedGame};
    let temp = tempfile::tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("library"), "Test").unwrap();
    let base = library.inspect().unwrap().current().unwrap().revision_id;
    let account = "76561198000000000";
    library.bind_steam(base, account).unwrap();
    let game = OwnedGame {
        appid: 620,
        name: "Portal 2".into(),
        ..Default::default()
    };
    let record = library.import_steam(account, &game).unwrap();
    let failures = gamesync_desktop::steam::resync_game(library.root(), record.game_id).unwrap();
    assert!(failures.is_empty(), "{failures:?}");
    let snapshot = RecordStore::open(library.root())
        .unwrap()
        .inspect(record.game_id)
        .unwrap();
    let steam = snapshot.current().unwrap().game.steam.clone().unwrap();
    let metadata = steam.metadata.unwrap();
    assert!(steam.description.is_some());
    assert!(metadata.details_complete && metadata.reviews_complete && metadata.tags_complete);
    assert!(!metadata.tags.is_empty());
    // Portal 2 is Steam Deck Verified with full controller support.
    let setup = metadata.setup.unwrap();
    assert_eq!(
        setup.deck,
        gamesync_desktop::suitability::DeckRating::Verified
    );
    assert_eq!(
        setup.controller,
        gamesync_desktop::suitability::ControllerSupport::Full
    );
}

#[test]
#[ignore = "Reads public Steam Deck reports; no account or API key is used"]
fn deck_report_names_an_anti_cheat_blocker() {
    use gamesync_desktop::suitability::{DeckNoteKind, DeckRating};
    let client = SteamClient::new().unwrap();
    // Destiny 2 is Unsupported because of its anti-cheat.
    let evidence = gamesync_desktop::steam::client::parse_setup(
        &client.deck_report(1085660).unwrap(),
        &client.store_categories(1085660).unwrap(),
        0,
    )
    .unwrap();
    assert_eq!(evidence.deck, DeckRating::Unsupported);
    assert!(evidence
        .deck_notes
        .iter()
        .any(|note| note.kind == DeckNoteKind::Blocker && note.text.contains("anti cheat")));
}
