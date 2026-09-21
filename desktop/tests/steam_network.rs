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
