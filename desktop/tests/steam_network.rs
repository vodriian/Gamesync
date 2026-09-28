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
