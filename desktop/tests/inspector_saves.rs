#![cfg(unix)]
//! Exercise the same library-aware write entry point as the native inspector.
use gamesync_desktop::{
    library::LibraryStore,
    library_reader::LibraryReader,
    record_store::RecordStore,
    records::{GameData, SteamData},
};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn personal_edits_survive_reload_and_keep_provider_and_extension_fields() {
    let temp = tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let manifest = library.inspect().unwrap().current().unwrap().clone();
    let records = RecordStore::open(library.root()).unwrap();
    let mut game = GameData::new("Hades");
    game.steam = Some(SteamData {
        metadata: None,
        app_id: 1145360,
        description: Some("Steam text".into()),
        playtime_minutes: 42,
        owned: true,
        extra: Default::default(),
    });
    game.personal
        .extra
        .insert("custom".into(), json!({"keep":true}));
    game.personal.cover = Some("media/hades.jpg".into());
    let base = records.create(game).unwrap();
    let mut personal = base.game.personal.clone();
    personal.status = "playing".into();
    personal.rating = Some(9);
    personal.favorite = true;
    personal.tags = vec!["Evening".into(), "Calm, sometimes".into()];
    personal.notes = "Try a short run.\nThen stop.".into();
    personal.description = Some("My description".into());
    let saved = library
        .edit_game_personal(&manifest, base.game_id, base.revision_id, personal.clone())
        .unwrap();
    assert_eq!(saved.parents, vec![base.revision_id]);
    assert_eq!(saved.game.personal, personal);
    assert_eq!(saved.game.steam, base.game.steam);
    assert_eq!(saved.game.title, base.game.title);
    let cache = temp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let loaded = LibraryReader::open(library.root(), &cache)
        .unwrap()
        .refresh()
        .unwrap();
    assert_eq!(loaded.games, vec![saved.clone()]);
    assert!(
        loaded.write_issue.is_none(),
        "Missing covers must not block edits"
    );
    personal.description = Some(String::new());
    let blank = library
        .edit_game_personal(&manifest, base.game_id, saved.revision_id, personal.clone())
        .unwrap();
    assert_eq!(blank.game.description(), Some(""));
    personal.description = None;
    let provider = library
        .edit_game_personal(&manifest, base.game_id, blank.revision_id, personal)
        .unwrap();
    assert_eq!(provider.game.description(), Some("Steam text"));
}

#[test]
fn invalid_status_and_stale_definitions_do_not_publish_a_game_revision() {
    let temp = tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let manifest = library.inspect().unwrap().current().unwrap().clone();
    let records = RecordStore::open(library.root()).unwrap();
    let base = records.create(GameData::new("Hades")).unwrap();
    let mut personal = base.game.personal.clone();
    personal.status = "unknown".into();
    assert!(library
        .edit_game_personal(&manifest, base.game_id, base.revision_id, personal)
        .is_err());
    let mut definitions = manifest.definitions.clone();
    definitions.name = "Renamed".into();
    library.edit(manifest.revision_id, definitions).unwrap();
    assert!(library
        .edit_game_personal(
            &manifest,
            base.game_id,
            base.revision_id,
            base.game.personal.clone()
        )
        .is_err());
    let snapshot = records.inspect(base.game_id).unwrap();
    assert_eq!(snapshot.current(), Some(&base));
    assert_eq!(snapshot.revisions.len(), 1);
}

#[test]
fn stale_game_and_remote_conflict_keep_draft_and_existing_files() {
    let temp = tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("Games.library"), "Games").unwrap();
    let manifest = library.inspect().unwrap().current().unwrap().clone();
    let records = RecordStore::open(library.root()).unwrap();
    let base = records.create(GameData::new("Hades")).unwrap();
    let mut draft = base.game.personal.clone();
    draft.notes = "Unsaved draft".into();
    let mut remote = base.game.personal.clone();
    remote.rating = Some(8);
    let current = records
        .edit_personal(base.game_id, base.revision_id, remote)
        .unwrap();
    assert!(library
        .edit_game_personal(&manifest, base.game_id, base.revision_id, draft.clone())
        .is_err());
    let mut branch = base.clone();
    branch.revision_id = uuid::Uuid::new_v4();
    branch.parents = vec![base.revision_id];
    branch.game.personal.favorite = true;
    let copy = library.root().join("games/remote copy.json");
    fs::write(&copy, serde_json::to_vec(&branch).unwrap()).unwrap();
    assert!(library
        .edit_game_personal(&manifest, base.game_id, current.revision_id, draft.clone())
        .is_err());
    assert_eq!(draft.notes, "Unsaved draft");
    let snapshot = records.inspect(base.game_id).unwrap();
    assert!(snapshot.has_conflict());
    assert_eq!(snapshot.revisions.len(), 3);
    assert_eq!(
        serde_json::from_slice::<gamesync_desktop::records::GameRevision>(&fs::read(copy).unwrap())
            .unwrap(),
        branch
    );
    let cache = temp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    assert!(LibraryReader::open(library.root(), &cache)
        .unwrap()
        .refresh()
        .unwrap()
        .write_issue
        .is_some());
}
