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
    personal.hidden = true;
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

#[test]
fn adding_collection_membership_preserves_game_data_and_rejects_stale_writes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("games");
    let library = LibraryStore::create(&root, "Games").unwrap();
    let base = library.inspect().unwrap().current().unwrap().clone();
    let first = collection();
    let mut second = collection();
    second.name = "Weekend".into();
    let mut definitions = base.definitions.clone();
    definitions.collections = vec![first.clone(), second.clone()];
    let manifest = library.edit(base.revision_id, definitions).unwrap();
    let mut data = GameData::new("Test game");
    data.personal.collections.push(first.id);
    data.personal.notes = "Keep my notes".into();
    data.personal.favorite = true;
    data.personal.rating = Some(9);
    let store = RecordStore::open(&root).unwrap();
    let game = store.create(data).unwrap();
    let mut personal = game.game.personal.clone();
    personal.collections.push(second.id);
    let saved = library
        .edit_game_personal(&manifest, game.game_id, game.revision_id, personal.clone())
        .unwrap();
    let mut expected = game.game.clone();
    expected.personal = personal.clone();
    assert_eq!(saved.game, expected);
    assert_eq!(saved.game.personal.collections, vec![first.id, second.id]);
    assert!(library
        .edit_game_personal(&manifest, game.game_id, game.revision_id, personal.clone())
        .is_err());
    let mut definitions = manifest.definitions.clone();
    definitions.collections[1].name = "Renamed elsewhere".into();
    library.edit(manifest.revision_id, definitions).unwrap();
    assert!(library
        .edit_game_personal(&manifest, saved.game_id, saved.revision_id, personal)
        .is_err());
    assert_eq!(
        store.inspect(game.game_id).unwrap().current().unwrap(),
        &saved
    );
}

#[test]
fn hidden_state_defaults_for_old_records_and_survives_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let library = LibraryStore::create(temp.path().join("games"), "Games").unwrap();
    let manifest = library.inspect().unwrap().current().unwrap().clone();
    let store = RecordStore::open(library.root()).unwrap();
    let game = store.create(GameData::new("Hidden example")).unwrap();
    let mut old = serde_json::to_value(&game).unwrap();
    old["game"]["personal"]
        .as_object_mut()
        .unwrap()
        .remove("hidden");
    let old: gamesync_desktop::records::GameRevision = serde_json::from_value(old).unwrap();
    assert!(!old.game.personal.hidden);
    let mut personal = game.game.personal.clone();
    personal.hidden = true;
    let saved = library
        .edit_game_personal(&manifest, game.game_id, game.revision_id, personal)
        .unwrap();
    assert!(!saved.deleted);
    let reopened = RecordStore::open(library.root())
        .unwrap()
        .inspect(game.game_id)
        .unwrap()
        .current()
        .unwrap()
        .clone();
    assert_eq!(reopened, saved);
    let mut personal = saved.game.personal.clone();
    personal.hidden = false;
    let restored = library
        .edit_game_personal(&manifest, saved.game_id, saved.revision_id, personal)
        .unwrap();
    assert_eq!(restored.game, game.game);
}

#[test]
fn bulk_edits_preserve_unrelated_fields_and_report_stale_games() {
    use gamesync_desktop::bulk::{apply, Change};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("games");
    let library = LibraryStore::create(&root, "Games").unwrap();
    let manifest = library.inspect().unwrap().current().unwrap().clone();
    let store = RecordStore::open(&root).unwrap();
    let mut data = GameData::new("Selected");
    data.personal.notes = "Keep this note".into();
    data.personal.tags = vec!["cozy".into()];
    let first = store.create(data).unwrap();
    let stale = store.create(GameData::new("Changed elsewhere")).unwrap();
    let untouched = store.create(GameData::new("Not selected")).unwrap();
    let mut personal = stale.game.personal.clone();
    personal.notes = "New note from another window".into();
    let newer = library
        .edit_game_personal(&manifest, stale.game_id, stale.revision_id, personal)
        .unwrap();
    let outcome = apply(
        &library,
        &manifest,
        vec![first.clone(), stale.clone()],
        &Change::Favorite(true),
    );
    assert_eq!(outcome.saved.len(), 1);
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].0, stale.game_id);
    let mut expected = first.game.clone();
    expected.personal.favorite = true;
    assert_eq!(outcome.saved[0].game, expected);
    assert_eq!(
        store.inspect(stale.game_id).unwrap().current().unwrap(),
        &newer
    );
    assert_eq!(
        store.inspect(untouched.game_id).unwrap().current().unwrap(),
        &untouched
    );
    let hidden = apply(&library, &manifest, outcome.saved, &Change::Hidden(true));
    assert!(hidden.failed.is_empty());
    assert!(hidden.saved[0].game.personal.hidden);
    assert!(hidden.saved[0].game.personal.favorite);
    let restored = apply(&library, &manifest, hidden.saved, &Change::Hidden(false));
    assert_eq!(restored.saved[0].game, expected);
}

#[test]
fn bulk_collection_add_remove_and_status_use_current_definitions() {
    use gamesync_desktop::bulk::{apply, Change};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("games");
    let library = LibraryStore::create(&root, "Games").unwrap();
    let base = library.inspect().unwrap().current().unwrap().clone();
    let c = collection();
    let mut definitions = base.definitions.clone();
    definitions.collections.push(c.clone());
    let manifest = library.edit(base.revision_id, definitions).unwrap();
    let store = RecordStore::open(&root).unwrap();
    let first = store.create(GameData::new("First")).unwrap();
    let second = store.create(GameData::new("Second")).unwrap();
    let added = apply(
        &library,
        &manifest,
        vec![first, second],
        &Change::Collection(c.id, true),
    );
    let added_again = apply(
        &library,
        &manifest,
        added.saved,
        &Change::Collection(c.id, true),
    );
    assert!(added_again.failed.is_empty());
    assert!(added_again
        .saved
        .iter()
        .all(|r| r.game.personal.collections == vec![c.id]));
    let status = manifest.definitions.statuses.last().unwrap().key.clone();
    let changed = apply(
        &library,
        &manifest,
        added_again.saved,
        &Change::Status(status.clone()),
    );
    assert!(changed
        .saved
        .iter()
        .all(|r| r.game.personal.status == status));
    let removed = apply(
        &library,
        &manifest,
        changed.saved,
        &Change::Collection(c.id, false),
    );
    assert!(removed
        .saved
        .iter()
        .all(|r| r.game.personal.collections.is_empty()));
    let mut definitions = manifest.definitions.clone();
    definitions.collections[0].archived = true;
    library.edit(manifest.revision_id, definitions).unwrap();
    let rejected = apply(
        &library,
        &manifest,
        removed.saved,
        &Change::Collection(c.id, true),
    );
    assert_eq!(rejected.failed.len(), 2);
    assert!(rejected.saved.is_empty());
}
