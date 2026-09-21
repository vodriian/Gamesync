#![cfg(unix)]
use gamesync_desktop::{
    library::LibraryStore, library_reader::LibraryReader, record_store::RecordStore,
    records::GameData,
};
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn conflicts_are_discoverable_without_a_cache_and_resolution_retains_both_versions() {
    let temp = tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let manifest = library.inspect().unwrap().current().unwrap().clone();
    let store = RecordStore::open(library.root()).unwrap();
    let base = store.create(GameData::new("Hades")).unwrap();
    let mut local = base.game.personal.clone();
    local.notes = "Local note".into();
    let local = store
        .edit_personal(base.game_id, base.revision_id, local)
        .unwrap();
    let mut remote = base.clone();
    remote.revision_id = Uuid::new_v4();
    remote.parents = vec![base.revision_id];
    remote.game.personal.notes = "Other computer".into();
    remote.deleted = true;
    let copy = library.root().join("games/remote copy.json");
    fs::write(&copy, serde_json::to_vec(&remote).unwrap()).unwrap();
    let cache = temp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let mut reader = LibraryReader::open(library.root(), &cache).unwrap();
    let loaded = reader.refresh().unwrap();
    assert!(loaded.games.is_empty());
    assert_eq!(
        loaded.conflicts.get(&base.game_id).map(String::as_str),
        Some("Hades")
    );
    let heads = store.inspect(base.game_id).unwrap().heads;
    let saved = library
        .resolve_game(&manifest, base.game_id, &heads, local.revision_id)
        .unwrap();
    assert_eq!(saved.game.personal.notes, "Local note");
    assert_eq!(
        saved
            .parents
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        heads
    );
    let snapshot = store.inspect(base.game_id).unwrap();
    assert_eq!(snapshot.revisions.get(&remote.revision_id), Some(&remote));
    assert!(copy.exists());
    let loaded = reader.refresh().unwrap();
    assert!(loaded.conflicts.is_empty());
    assert_eq!(loaded.games.len(), 1);
}

#[test]
fn stale_review_and_unknown_status_do_not_resolve_conflicts() {
    let temp = tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let manifest = library.inspect().unwrap().current().unwrap().clone();
    let store = RecordStore::open(library.root()).unwrap();
    let base = store.create(GameData::new("Hades")).unwrap();
    let local = store
        .edit_personal(base.game_id, base.revision_id, base.game.personal.clone())
        .unwrap();
    let mut remote = base.clone();
    remote.revision_id = Uuid::new_v4();
    remote.parents = vec![base.revision_id];
    remote.game.personal.status = "unknown".into();
    fs::write(
        library.root().join("games/remote.json"),
        serde_json::to_vec(&remote).unwrap(),
    )
    .unwrap();
    let heads = store.inspect(base.game_id).unwrap().heads;
    assert!(library
        .resolve_game(&manifest, base.game_id, &heads, remote.revision_id)
        .is_err());
    let mut late = base.clone();
    late.revision_id = Uuid::new_v4();
    late.parents = vec![base.revision_id];
    fs::write(
        library.root().join("games/late.json"),
        serde_json::to_vec(&late).unwrap(),
    )
    .unwrap();
    assert!(library
        .resolve_game(&manifest, base.game_id, &heads, local.revision_id)
        .is_err());
    let heads = store.inspect(base.game_id).unwrap().heads;
    let mut definitions = manifest.definitions.clone();
    definitions.name = "Changed library".into();
    library.edit(manifest.revision_id, definitions).unwrap();
    assert!(library
        .resolve_game(&manifest, base.game_id, &heads, local.revision_id)
        .is_err());
    assert_eq!(store.inspect(base.game_id).unwrap().heads, heads);
}
