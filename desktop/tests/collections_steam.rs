use gamesync_desktop::{
    library::{CollectionDefinition, LibraryStore},
    record_store::RecordStore,
    records::GameData,
    steam::OwnedGame,
};
use uuid::Uuid;
const ACCOUNT: &str = "76561198000000000";
fn collection() -> CollectionDefinition {
    CollectionDefinition {
        id: Uuid::new_v4(),
        name: "Cozy".into(),
        archived: false,
        extra: Default::default(),
    }
}
#[test]
fn collections_survive_rename_archive_and_relocation_without_game_rewrites() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("games");
    let library = LibraryStore::create(&root, "Games").unwrap();
    let base = library.inspect().unwrap().current().unwrap().clone();
    let mut definitions = base.definitions.clone();
    let c = collection();
    definitions.collections.push(c.clone());
    let first = library.edit(base.revision_id, definitions.clone()).unwrap();
    assert!(library.edit(base.revision_id, definitions).is_err());
    let game = RecordStore::open(&root)
        .unwrap()
        .create(GameData::new("Test"))
        .unwrap();
    let mut personal = game.game.personal.clone();
    personal.collections.push(c.id);
    let saved = library
        .edit_game_personal(&first, game.game_id, game.revision_id, personal)
        .unwrap();
    let mut definitions = first.definitions.clone();
    definitions.collections[0].name = "Evening".into();
    let second = library
        .edit(first.revision_id, definitions.clone())
        .unwrap();
    definitions.collections[0].archived = true;
    let last = library.edit(second.revision_id, definitions).unwrap();
    assert_eq!(
        RecordStore::open(&root)
            .unwrap()
            .inspect(game.game_id)
            .unwrap()
            .current()
            .unwrap(),
        &saved
    );
    let mut invalid = last.definitions.clone();
    invalid.collections.clear();
    assert!(library.edit(last.revision_id, invalid).is_err());
    let moved = temp.path().join("moved");
    std::fs::rename(root, &moved).unwrap();
    assert_eq!(
        LibraryStore::open(&moved)
            .unwrap()
            .inspect()
            .unwrap()
            .current()
            .unwrap(),
        &last
    );
    assert_eq!(
        RecordStore::open(&moved)
            .unwrap()
            .inspect(game.game_id)
            .unwrap()
            .current()
            .unwrap()
            .game
            .personal
            .collections,
        vec![c.id]
    );
}
#[test]
fn old_records_default_to_empty_collections_and_keep_extensions() {
    let mut game = serde_json::to_value(GameData::new("Old game")).unwrap();
    game["personal"]
        .as_object_mut()
        .unwrap()
        .remove("collections");
    game["personal"]["extension"] = serde_json::json!({"value":5});
    let old: GameData = serde_json::from_value(game).unwrap();
    assert!(old.personal.collections.is_empty());
    assert_eq!(old.personal.extra["extension"]["value"], 5);
    let mut definitions =
        serde_json::to_value(gamesync_desktop::library::LibraryDefinitions::new("Old")).unwrap();
    definitions.as_object_mut().unwrap().remove("collections");
    definitions.as_object_mut().unwrap().remove("steam_account");
    let old: gamesync_desktop::library::LibraryDefinitions =
        serde_json::from_value(definitions).unwrap();
    assert!(old.collections.is_empty());
    assert!(old.steam_account.is_none());
}
#[test]
fn steam_import_is_idempotent_and_preserves_archived_games_and_manual_changes() {
    let temp = tempfile::tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("games"), "Games").unwrap();
    let base = library.inspect().unwrap().current().unwrap().clone();
    let manifest = library.bind_steam(base.revision_id, ACCOUNT).unwrap();
    assert!(library
        .bind_steam(manifest.revision_id, "76561198000000001")
        .is_err());
    let mut input = OwnedGame {
        appid: 42,
        name: "Test".into(),
        playtime_forever: 5,
    };
    let game = library.import_steam(ACCOUNT, &input).unwrap();
    let again = library.import_steam(ACCOUNT, &input).unwrap();
    assert_eq!(game, again);
    let mut personal = game.game.personal.clone();
    personal.rating = Some(8);
    personal.notes = "Keep".into();
    personal.status = "completed".into();
    let saved = library
        .edit_game_personal(&manifest, game.game_id, game.revision_id, personal.clone())
        .unwrap();
    let store = RecordStore::open(library.root()).unwrap();
    store
        .set_archived(game.game_id, saved.revision_id, true)
        .unwrap();
    input.playtime_forever = 10;
    let synced = library.import_steam(ACCOUNT, &input).unwrap();
    assert!(synced.deleted);
    assert_eq!(synced.game.personal, personal);
    assert_eq!(synced.game.steam.unwrap().playtime_minutes, 10);
    assert!(library
        .edit_game_personal(&manifest, game.game_id, saved.revision_id, personal)
        .is_err());
}
#[test]
fn duplicate_steam_id_blocks_import_instead_of_guessing() {
    let temp = tempfile::tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("games"), "Games").unwrap();
    let base = library.inspect().unwrap().current().unwrap().revision_id;
    library.bind_steam(base, ACCOUNT).unwrap();
    let input = OwnedGame {
        appid: 42,
        name: "Test".into(),
        playtime_forever: 5,
    };
    let game = library.import_steam(ACCOUNT, &input).unwrap();
    RecordStore::open(library.root())
        .unwrap()
        .create(game.game)
        .unwrap();
    assert!(library.import_steam(ACCOUNT, &input).is_err());
}

#[test]
fn collection_conflicts_need_all_heads_and_keep_membership_history() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("games");
    let library = LibraryStore::create(&root, "Games").unwrap();
    let base = library.inspect().unwrap().current().unwrap().clone();
    let mut definitions = base.definitions.clone();
    definitions.collections.push(collection());
    let parent = library.edit(base.revision_id, definitions).unwrap();
    let mut local = parent.definitions.clone();
    local.collections[0].name = "Evening".into();
    let first = library.edit(parent.revision_id, local).unwrap();
    let mut remote = parent.clone();
    remote.revision_id = Uuid::new_v4();
    remote.parents = vec![parent.revision_id];
    remote.definitions.collections[0].archived = true;
    std::fs::write(
        root.join("library (conflicted copy).json"),
        serde_json::to_vec_pretty(&remote).unwrap(),
    )
    .unwrap();
    let snapshot = library.inspect().unwrap();
    assert!(snapshot.has_conflict());
    assert!(library
        .edit(first.revision_id, first.definitions.clone())
        .is_err());
    let stale = std::collections::BTreeSet::from([parent.revision_id, first.revision_id]);
    assert!(library
        .resolve(&stale, first.revision_id, first.definitions.clone())
        .is_err());
    let saved = library
        .resolve(
            &snapshot.heads,
            first.revision_id,
            first.definitions.clone(),
        )
        .unwrap();
    assert_eq!(saved.parents.len(), 2);
    assert_eq!(saved.definitions.collections[0].name, "Evening");
    assert!(!saved.definitions.collections[0].archived);
    let final_state = library.inspect().unwrap();
    assert!(final_state.revisions.contains_key(&remote.revision_id));
    assert!(final_state.current().is_some());
}
