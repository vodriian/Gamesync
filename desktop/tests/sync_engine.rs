//! Two devices with real local libraries share one sync folder.

use gamesync_desktop::{
    library::{CollectionDefinition, LibraryStore, StatusDefinition},
    library_reader::{LibraryReader, LoadedLibrary},
    record_store::RecordStore,
    records::{GameData, PersonalData, SteamData},
    settings::{LibraryView, Settings},
    sync::{apply_library, apply_settings, project, DeviceInfo, Round, SyncEngine, Target},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use uuid::Uuid;

struct Machine {
    root: PathBuf,
    cache: PathBuf,
    state: PathBuf,
    settings: Settings,
    engine: Option<SyncEngine>,
}

impl Machine {
    fn new(base: &Path, name: &str, apps: &[u32]) -> Self {
        let dir = base.join(name);
        std::fs::create_dir_all(dir.join("cache")).unwrap();
        let root = dir.join("library");
        LibraryStore::create(&root, "My games").unwrap();
        let machine = Self {
            root,
            cache: dir.join("cache"),
            state: dir.join("state/sync.json"),
            settings: Settings::default(),
            engine: None,
        };
        for app in apps {
            machine.add_game(*app);
        }
        machine
    }

    fn add_game(&self, app_id: u32) {
        let mut game = GameData::new(format!("Game {app_id}"));
        game.steam = Some(SteamData {
            app_id,
            description: None,
            playtime_minutes: 0,
            owned: true,
            last_played: None,
            platform_minutes: Default::default(),
            wishlist: None,
            metadata: None,
            extra: Default::default(),
        });
        RecordStore::open(&self.root).unwrap().create(game).unwrap();
    }

    fn load(&self) -> LoadedLibrary {
        LibraryReader::open(&self.root, &self.cache)
            .unwrap()
            .refresh()
            .unwrap()
    }

    fn personal(&self, app_id: u32) -> PersonalData {
        self.load()
            .games
            .into_iter()
            .find(|g| g.game.steam.as_ref().unwrap().app_id == app_id)
            .unwrap()
            .game
            .personal
    }

    fn edit(&self, app_id: u32, change: impl FnOnce(&mut PersonalData)) {
        let loaded = self.load();
        let game = loaded
            .games
            .iter()
            .find(|g| g.game.steam.as_ref().unwrap().app_id == app_id)
            .unwrap();
        let mut personal = game.game.personal.clone();
        change(&mut personal);
        LibraryStore::open(&self.root)
            .unwrap()
            .edit_game_personal(&loaded.manifest, game.game_id, game.revision_id, personal)
            .unwrap();
    }

    fn edit_definitions(
        &self,
        change: impl FnOnce(&mut gamesync_desktop::library::LibraryDefinitions),
    ) {
        let store = LibraryStore::open(&self.root).unwrap();
        let current = store.inspect().unwrap().current().unwrap().clone();
        let mut definitions = current.definitions.clone();
        change(&mut definitions);
        store.edit(current.revision_id, definitions).unwrap();
    }

    fn join(&mut self, folder: &Path) {
        let info = DeviceInfo {
            name: "test".into(),
            platform: "linux".into(),
            app_version: "0.1.0".into(),
            last_seen_ms: 0,
            extra: BTreeMap::new(),
        };
        self.engine = Some(SyncEngine::open(folder, &self.state, info, true).unwrap());
    }

    /// One round as the app runs it: read local state, sync, write values.
    fn sync(&mut self, wall_ms: u64) -> Round {
        let loaded = self.load();
        let local = project(&loaded.manifest.definitions, &loaded.games, &self.settings);
        let engine = self.engine.as_mut().unwrap();
        let round = engine.round(&local, wall_ms);
        let mut applied = apply_library(&self.root, &loaded.games, &round.apply);
        applied
            .done
            .extend(apply_settings(&mut self.settings, &round.apply).done);
        assert!(applied.issues.is_empty(), "{:?}", applied.issues);
        engine.confirm(&round.apply, &applied.done).unwrap();
        round
    }

    fn engine(&mut self) -> &mut SyncEngine {
        self.engine.as_mut().unwrap()
    }

    fn resolve(&mut self, app_id: u32, field: &str, value: Value, wall_ms: u64) {
        let key = (Target::Steam(app_id), field.to_owned());
        let round = self.engine.as_mut().unwrap().resolve(&key, value, wall_ms);
        assert!(round.issues.is_empty(), "{:?}", round.issues);
    }
}

#[test]
fn personal_data_statuses_collections_and_settings_reach_the_other_device() {
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[620, 440]);
    let mut linux = Machine::new(base.path(), "linux", &[620, 440]);

    let coop = Uuid::new_v4();
    mac.edit_definitions(|d| {
        d.statuses.insert(
            0,
            StatusDefinition {
                key: "replay".into(),
                label: "Replay".into(),
                recommendation_eligible: true,
                extra: Default::default(),
            },
        );
        d.collections.push(CollectionDefinition {
            id: coop,
            name: "Co-op".into(),
            archived: false,
            extra: Default::default(),
        });
    });
    mac.edit(620, |p| {
        p.rating = Some(8);
        p.set_status("replay".into());
        p.notes = "Finish the DLC".into();
    });
    mac.edit(440, |p| {
        p.favorite = true;
        p.collections.push(coop);
        p.tags = vec!["Weekend".into()];
    });
    mac.settings.reduce_motion = true;
    mac.settings.section_views.insert(
        r#"smart:{"kind":"my_tag","value":"Weekend"}"#.into(),
        LibraryView::Board,
    );

    mac.join(&cloud);
    let round = mac.sync(1_000);
    assert!(round.written > 0 && round.issues.is_empty(), "{round:?}");

    linux.join(&cloud);
    let round = linux.sync(2_000);
    assert!(
        round.conflicts.is_empty() && round.issues.is_empty(),
        "{round:?}"
    );

    let definitions = linux.load().manifest.definitions;
    assert_eq!(definitions.statuses[0].key, "replay");
    assert_eq!(definitions.statuses[0].label, "Replay");
    assert!(definitions
        .collections
        .iter()
        .any(|c| c.id == coop && c.name == "Co-op"));
    let game = linux.personal(620);
    assert_eq!((game.rating, game.status.as_str()), (Some(8), "replay"));
    assert_eq!(game.notes, "Finish the DLC");
    let game = linux.personal(440);
    assert!(game.favorite);
    assert_eq!(game.collections, vec![coop]);
    assert_eq!(game.tags, vec!["Weekend".to_string()]);
    assert!(linux.settings.reduce_motion);
    assert_eq!(
        linux.settings.section_views.values().next(),
        Some(&LibraryView::Board)
    );

    // Nothing changes when both sides agree.
    let round = linux.sync(3_000);
    assert_eq!((round.written, round.apply.len()), (0, 0), "{round:?}");
    let round = mac.sync(3_000);
    assert_eq!((round.written, round.apply.len()), (0, 0), "{round:?}");
}

#[test]
fn a_later_edit_applies_and_a_concurrent_edit_waits_for_review() {
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[620]);
    let mut linux = Machine::new(base.path(), "linux", &[620]);
    mac.join(&cloud);
    linux.join(&cloud);
    mac.sync(1_000);
    linux.sync(1_000);

    // Sequential: the Mac edits, Linux reads it, then Linux edits.
    mac.edit(620, |p| p.rating = Some(6));
    mac.sync(2_000);
    linux.sync(2_100);
    assert_eq!(linux.personal(620).rating, Some(6));
    linux.edit(620, |p| p.rating = Some(9));
    linux.sync(2_200);
    let round = mac.sync(2_300);
    assert!(round.conflicts.is_empty());
    assert_eq!(mac.personal(620).rating, Some(9));

    // Concurrent: both edit the notes before either reads the other.
    mac.edit(620, |p| p.notes = "Mac".into());
    linux.edit(620, |p| p.notes = "Linux".into());
    mac.sync(3_000);
    linux.sync(3_100);
    let round = mac.sync(3_200);
    assert_eq!(round.conflicts.len(), 1);
    assert_eq!(round.conflicts[0].local, json!("Mac"));
    // Nothing is lost while the review waits.
    assert_eq!(mac.personal(620).notes, "Mac");
    assert_eq!(linux.personal(620).notes, "Linux");

    // The Mac keeps both; Linux applies the result and the review closes.
    mac.resolve(620, "personal.notes", json!("Mac\n\nLinux"), 4_000);
    let round = linux.sync(4_100);
    assert!(round.conflicts.is_empty());
    assert_eq!(linux.personal(620).notes, "Mac\n\nLinux");
    let round = mac.sync(4_200);
    assert!(round.conflicts.is_empty());
    assert_eq!(mac.personal(620).notes, "Mac\n\nLinux");
}

#[test]
fn joining_takes_one_sided_values_and_asks_about_different_values() {
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[620, 440]);
    let mut linux = Machine::new(base.path(), "linux", &[620, 440]);
    mac.edit(620, |p| p.rating = Some(8));
    linux.edit(620, |p| p.rating = Some(6));
    linux.edit(440, |p| p.tags = vec!["Short".into()]);
    mac.join(&cloud);
    mac.sync(1_000);
    linux.join(&cloud);
    let round = linux.sync(2_000);

    assert_eq!(round.conflicts.len(), 1);
    assert_eq!(
        round.conflicts[0].key,
        (Target::Steam(620), "personal.rating".into())
    );
    let round = mac.sync(3_000);
    assert_eq!(round.conflicts.len(), 1);
    assert_eq!(mac.personal(440).tags, vec!["Short".to_string()]);

    // "Use the folder" on Linux: it takes the Mac value.
    linux.resolve(620, "personal.rating", json!(8), 4_000);
    linux.sync(4_100);
    assert_eq!(linux.personal(620).rating, Some(8));
    assert!(mac.sync(4_200).conflicts.is_empty());
}

#[test]
fn values_for_a_missing_game_wait_until_steam_adds_it() {
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[620, 730]);
    let mut linux = Machine::new(base.path(), "linux", &[620]);
    mac.edit(730, |p| p.favorite = true);
    mac.join(&cloud);
    mac.sync(1_000);
    linux.join(&cloud);
    let round = linux.sync(2_000);
    assert!(round.conflicts.is_empty());
    assert_eq!(round.written, 0, "a missing game is not a cleared game");

    linux.add_game(730);
    linux.sync(3_000);
    assert!(linux.personal(730).favorite);
    assert!(mac.sync(3_100).conflicts.is_empty());
    assert!(mac.personal(730).favorite);
}

#[test]
fn a_restart_keeps_what_the_device_knew() {
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[620]);
    let mut linux = Machine::new(base.path(), "linux", &[620]);
    mac.join(&cloud);
    linux.join(&cloud);
    mac.sync(1_000);
    linux.sync(1_000);
    let device = linux.engine.as_ref().unwrap().device();

    // Linux closes. The Mac edits while it is closed.
    linux.engine = None;
    mac.edit(620, |p| p.rating = Some(4));
    mac.sync(2_000);
    linux.join(&cloud);
    assert_eq!(linux.engine.as_ref().unwrap().device(), device);
    let round = linux.sync(3_000);
    assert!(round.conflicts.is_empty(), "{round:?}");
    assert_eq!(linux.personal(620).rating, Some(4));
}

#[test]
fn edits_wait_while_the_folder_is_offline() {
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[620]);
    let mut linux = Machine::new(base.path(), "linux", &[620]);
    mac.join(&cloud);
    linux.join(&cloud);
    mac.sync(1_000);

    let parked = base.path().join("parked");
    std::fs::rename(&cloud, &parked).unwrap();
    mac.edit(620, |p| p.hidden = true);
    let round = mac.sync(2_000);
    assert_eq!(round.waiting, 1);
    assert!(!round.issues.is_empty());
    assert!(
        !cloud.exists(),
        "an offline folder is not created on the local disk"
    );

    std::fs::rename(&parked, &cloud).unwrap();
    let round = mac.sync(3_000);
    assert_eq!((round.written, round.waiting), (1, 0));
    linux.sync(3_100);
    assert!(linux.personal(620).hidden);
}

#[test]
fn a_copied_device_state_continues_under_a_new_device_id() {
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[620]);
    mac.join(&cloud);
    mac.edit(620, |p| p.rating = Some(2));
    mac.sync(1_000);

    // A second computer starts from a copy of the Mac's app data.
    copy_dir(&base.path().join("mac"), &base.path().join("copy"));
    let mut copy = Machine {
        root: base.path().join("copy/library"),
        cache: base.path().join("copy/cache"),
        state: base.path().join("copy/state/sync.json"),
        settings: Settings::default(),
        engine: None,
    };
    copy.join(&cloud);
    copy.edit(620, |p| p.favorite = true);
    copy.sync(2_000);
    let old = mac.engine.as_ref().unwrap().device();

    mac.edit(620, |p| p.rating = Some(3));
    let round = mac.sync(3_000);
    assert_eq!(round.waiting, 0, "{round:?}");
    assert!(round.issues.iter().any(|i| i.contains("Another device")));
    assert_ne!(mac.engine.as_ref().unwrap().device(), old);
    assert!(mac.personal(620).favorite);

    let mut linux = Machine::new(base.path(), "linux", &[620]);
    linux.join(&cloud);
    linux.sync(4_000);
    let game = linux.personal(620);
    assert_eq!((game.rating, game.favorite), (Some(3), true));
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn sealed_keys_sync_without_rewrites_when_encryption_changes() {
    use gamesync_desktop::sync::secrets::{ExposeSecret as _, SyncKey};
    let base = tempfile::tempdir().unwrap();
    let cloud = base.path().join("cloud");
    std::fs::create_dir(&cloud).unwrap();
    let mut mac = Machine::new(base.path(), "mac", &[]);
    let mut linux = Machine::new(base.path(), "linux", &[]);
    mac.join(&cloud);
    linux.join(&cloud);
    let key = SyncKey::generate();
    let target = Target::Secret("steam_api_key".into());
    let field = (target.clone(), "value".to_string());

    // Each round encrypts again; only the fingerprint decides equality.
    let round = |machine: &mut Machine, secret: Option<&str>, ms: u64| {
        let loaded = machine.load();
        let mut local = project(
            &loaded.manifest.definitions,
            &loaded.games,
            &machine.settings,
        );
        local.targets.insert(target.clone());
        if let Some(secret) = secret {
            local
                .fields
                .insert(field.clone(), key.seal("steam_api_key", secret).unwrap());
        }
        machine.engine().round(&local, ms)
    };
    assert_eq!(round(&mut mac, Some("K1"), 1_000).written, 1);
    assert_eq!(round(&mut mac, Some("K1"), 2_000).written, 0);

    let received = round(&mut linux, None, 3_000);
    assert_eq!(received.apply.len(), 1);
    assert_eq!(
        key.open(&received.apply[0].value).unwrap().expose_secret(),
        "K1"
    );
    linux.engine().confirm(&received.apply, &[0]).unwrap();
    // Linux now holds K1, encrypted again by its own round: no write back.
    assert_eq!(round(&mut linux, Some("K1"), 4_000).written, 0);

    // A new key on Linux reaches the Mac.
    assert_eq!(round(&mut linux, Some("K2"), 5_000).written, 1);
    let received = round(&mut mac, Some("K1"), 6_000);
    assert!(received.conflicts.is_empty());
    assert_eq!(
        key.open(&received.apply[0].value).unwrap().expose_secret(),
        "K2"
    );
}
