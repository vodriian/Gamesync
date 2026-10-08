use gamesync_desktop::{
    library::LibraryStore,
    recommendations::*,
    record_store::RecordStore,
    records::{GameData, GameRevision, SteamData},
};
use uuid::Uuid;

fn record(id: u128) -> GameRevision {
    let mut game = GameData::new(format!("Game {id}"));
    game.steam = Some(SteamData {
        app_id: id as u32,
        description: None,
        playtime_minutes: 0,
        owned: true,
        last_played: None,
        platform_minutes: Default::default(),
        wishlist: None,
        metadata: None,
        extra: Default::default(),
    });
    game.personal.play_now.profile = Profile {
        mechanical: Some(Effort::Low),
        cognitive: Some(Effort::Low),
        narrative: Some(Effort::Low),
        onboarding: Some(Effort::Low),
        minimum_minutes: Some(10),
        ideal_minutes: Some(25),
        setup_minutes: Some(2),
        stopping: Some(Stopping::Flexible),
        activities: Some(vec![Activity::Explore]),
    };
    GameRevision {
        schema_version: 1,
        game_id: Uuid::from_u128(id),
        revision_id: Uuid::new_v4(),
        parents: vec![],
        deleted: false,
        game,
        extra: Default::default(),
    }
}
fn candidate(record: &GameRevision) -> Candidate<'_> {
    Candidate {
        record,
        status_eligible: true,
        installed: Some(true),
        blocked: false,
    }
}
fn reject(record: &GameRevision, context: &Context) -> Option<Rejection> {
    eligibility(&candidate(record), context, &resolve(&record.game)).err()
}

#[test]
fn hidden_unowned_deleted_status_exclusion_and_device_are_hard_limits() {
    let mut g = record(1);
    let c = Context {
        minutes: None,
        energy: Effort::High,
        ..Default::default()
    };
    g.game.personal.hidden = true;
    assert_eq!(reject(&g, &c), Some(Rejection::Hidden));
    g.game.personal.hidden = false;
    g.game.steam.as_mut().unwrap().owned = false;
    assert_eq!(reject(&g, &c), Some(Rejection::Unavailable));
    g.game.steam.as_mut().unwrap().owned = true;
    g.deleted = true;
    assert_eq!(reject(&g, &c), Some(Rejection::Unavailable));
    g.deleted = false;
    g.game.personal.play_now.excluded = true;
    assert_eq!(reject(&g, &c), Some(Rejection::Excluded));
    g.game.personal.play_now.excluded = false;
    let mut data = candidate(&g);
    data.status_eligible = false;
    assert_eq!(available(&data), Err(Rejection::Status));
    data.status_eligible = true;
    data.blocked = true;
    assert_eq!(available(&data), Err(Rejection::Device));
}
#[test]
fn finite_time_includes_startup_and_infinity_does_not_disable_energy() {
    let mut g = record(1);
    let mut c = Context {
        minutes: Some(10),
        ..Default::default()
    };
    assert_eq!(reject(&g, &c), Some(Rejection::Time));
    c.minutes = Some(12);
    assert_eq!(reject(&g, &c), None);
    c.minutes = None;
    g.game.personal.play_now.profile.minimum_minutes = None;
    g.game.personal.play_now.profile.setup_minutes = None;
    assert_eq!(reject(&g, &c), None);
    g.game.personal.play_now.profile.cognitive = Some(Effort::High);
    assert_eq!(reject(&g, &c), Some(Rejection::Energy));
}
#[test]
fn brain_dead_keeps_mechanics_and_requires_evidence() {
    let mut g = record(1);
    let mut c = Context {
        brain_dead: true,
        ..Default::default()
    };
    assert_eq!(reject(&g, &c), None);
    g.game.personal.play_now.profile.mechanical = Some(Effort::High);
    assert_eq!(reject(&g, &c), Some(Rejection::Energy));
    c.energy = Effort::High;
    assert_eq!(reject(&g, &c), None);
    g.game.personal.play_now.profile.onboarding = None;
    assert_eq!(reject(&g, &c), Some(Rejection::Unknown));
    g.game.personal.play_now.profile.cognitive = Some(Effort::High);
    assert_eq!(reject(&g, &c), Some(Rejection::BrainDead));
    g.game.personal.play_now.profile.cognitive = Some(Effort::Low);
    g.game.personal.play_now.profile.onboarding = Some(Effort::High);
    assert_eq!(reject(&g, &c), Some(Rejection::BrainDead));
}
#[test]
fn exact_activity_scope_and_installation_filters_compose() {
    let mut g = record(1);
    let mut c = Context {
        activity: Some(Activity::Horse),
        ..Default::default()
    };
    assert_eq!(reject(&g, &c), Some(Rejection::Activity));
    c.activity = Some(Activity::Explore);
    c.favorites_only = true;
    assert_eq!(reject(&g, &c), Some(Rejection::Scope));
    g.game.personal.favorite = true;
    c.scope = Scope::Unplayed;
    assert_eq!(reject(&g, &c), None);
    g.game.steam.as_mut().unwrap().playtime_minutes = 1;
    assert_eq!(reject(&g, &c), Some(Rejection::Scope));
    c.scope = Scope::Any;
    c.installed_only = true;
    let mut data = candidate(&g);
    data.installed = None;
    assert_eq!(
        eligibility(&data, &c, &resolve(&g.game)),
        Err(Rejection::Installation)
    );
}
#[test]
fn estimates_are_labeled_and_missing_data_is_not_a_match() {
    let mut g = record(1);
    g.game.personal.play_now.profile = Profile::default();
    assert_eq!(reject(&g, &Context::default()), Some(Rejection::Unknown));
    g.game.personal.tags = vec!["Card Battler".into()];
    let p = resolve(&g.game);
    assert!(p.estimated());
    assert_eq!(p.values.cognitive, Some(Effort::High));
    assert_eq!(p.values.activities, Some(vec![Activity::Cards]));
    assert_eq!(reject(&g, &Context::default()), Some(Rejection::Energy));
    g.game.personal.tags = vec!["RPG".into()];
    assert_eq!(resolve(&g.game).values.activities, None);
}
#[test]
fn manual_values_win_and_stale_analysis_is_not_used() {
    let mut g = record(1);
    let mut ai = g.game.personal.play_now.profile.clone();
    ai.cognitive = Some(Effort::High);
    ai.activities = Some(vec![Activity::Horse]);
    g.game.recommendation_analysis = Some(Analysis {
        game_id: g.game_id,
        version: PROFILE_VERSION,
        provider: "fixture".into(),
        model: "test".into(),
        fingerprint: fingerprint(&g.game),
        analyzed_at: 1,
        confidence: 50,
        reason: None,
        profile: ai,
    });
    assert_eq!(resolve(&g.game).values.cognitive, Some(Effort::Low));
    g.game.personal.play_now.profile.cognitive = None;
    assert_eq!(resolve(&g.game).values.cognitive, Some(Effort::High));
    g.game.personal.play_now.profile.activities = Some(vec![]);
    assert_eq!(resolve(&g.game).values.activities, Some(vec![]));
    g.game.title = "Changed metadata".into();
    assert_eq!(resolve(&g.game).values.cognitive, None);
    let ai = g.game.recommendation_analysis.as_mut().unwrap();
    ai.game_id = Uuid::new_v4();
    assert!(g.validate().is_err());
}
#[test]
fn deterministic_unique_hands_revalidate_and_reshuffle_only_once() {
    let mut records: Vec<_> = (1..=8).map(record).collect();
    let c = Context::default();
    let candidates: Vec<_> = records.iter().rev().map(candidate).collect();
    let picks = rank(&candidates, &c, 100).ranked;
    assert_eq!(picks[0].id, records[0].game_id);
    let mut s = Session::default();
    s.deal(c.clone(), &picks, false);
    let first = s.hand.clone();
    assert_eq!(first.len(), 3);
    s.dismissed.insert(first[0]);
    s.deal(c.clone(), &picks, false);
    assert_eq!(s.hand, first);
    s.deal(c.clone(), &picks, true);
    assert!(s.hand.iter().all(|id| !first.contains(id)));
    assert_eq!(s.reshuffles, 0);
    let second = s.hand.clone();
    s.deal(c.clone(), &picks, true);
    assert_eq!(s.hand, second);
    s.deal(c.clone(), &picks, false);
    assert_eq!(s.hand, second);
    records
        .iter_mut()
        .find(|r| r.game_id == second[0])
        .unwrap()
        .game
        .personal
        .hidden = true;
    let candidates: Vec<_> = records.iter().map(candidate).collect();
    let new_picks = rank(&candidates, &c, 100).ranked;
    s.deal(c, &new_picks, false);
    assert!(!s.hand.contains(&second[0]));
}
#[test]
fn fewer_eligible_games_are_not_padded_and_duplicate_ids_are_removed() {
    let g = record(1);
    let context = Context::default();
    let selection = rank(&[candidate(&g), candidate(&g)], &context, 0);
    assert_eq!(selection.ranked.len(), 1);
    let mut s = Session::default();
    s.deal(context, &selection.ranked, false);
    assert_eq!(s.hand.len(), 1);
}
#[test]
fn profile_ranges_and_activity_duplicates_are_validated() {
    let mut p = Profile {
        minimum_minutes: Some(45),
        ideal_minutes: Some(30),
        ..Default::default()
    };
    assert!(p.validate().is_err());
    p.ideal_minutes = Some(60);
    assert!(p.validate().is_ok());
    p.activities = Some(vec![Activity::Cards, Activity::Cards]);
    assert!(p.validate().is_err());
}
#[test]
fn feedback_persists_recovers_and_does_not_change_steam_or_personal_rating() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let library = LibraryStore::create(temp.path().join("library"), "Test")?;
    let store = RecordStore::open(library.root())?;
    let base = store.create(record(1).game)?;
    let manifest = library.inspect()?.current().unwrap().clone();
    let mut personal = base.game.personal.clone();
    personal.play_now.saved = true;
    personal.play_now.excluded = true;
    personal.play_now.record_choice(Choice {
        at: 100,
        session: Uuid::new_v4(),
        context: Context::default(),
        launch: LaunchOutcome::NotRequested,
        finished_at: None,
    });
    let saved =
        library.edit_game_personal(&manifest, base.game_id, base.revision_id, personal.clone())?;
    assert_eq!(saved.game.steam, base.game.steam);
    assert_eq!(saved.game.personal.status, base.game.personal.status);
    assert_eq!(saved.game.personal.rating, base.game.personal.rating);
    assert!(!saved.game.personal.hidden);
    assert!(library
        .edit_game_personal(&manifest, base.game_id, base.revision_id, personal)
        .is_err());
    assert_eq!(
        RecordStore::open(library.root())?
            .inspect(base.game_id)?
            .current()
            .unwrap(),
        &saved
    );
    store.update_steam(base.game_id, 1, |s| s.playtime_minutes = 100)?;
    assert!(
        store
            .inspect(base.game_id)?
            .current()
            .unwrap()
            .game
            .personal
            .play_now
            .saved
    );
    assert_eq!(
        store
            .inspect(base.game_id)?
            .current()
            .unwrap()
            .game
            .personal
            .play_now
            .profile,
        base.game.personal.play_now.profile
    );
    Ok(())
}
#[test]
fn personal_feedback_projects_and_applies_through_existing_sync() -> anyhow::Result<()> {
    use gamesync_desktop::sync::{apply_library, project, Apply, Target};
    let temp = tempfile::tempdir()?;
    let library = LibraryStore::create(temp.path().join("library"), "Test")?;
    let store = RecordStore::open(library.root())?;
    let local = store.create(record(1).game)?;
    let manifest = library.inspect()?.current().unwrap().clone();
    let mut remote = local.clone();
    remote.game.personal.play_now.saved = true;
    let projection = project(&manifest.definitions, &[remote], &Default::default());
    let key = (Target::Steam(1), "personal.play_now".into());
    let applied = apply_library(
        library.root(),
        std::slice::from_ref(&local),
        &[Apply {
            key: key.clone(),
            value: projection.fields[&key].clone(),
            ids: vec![],
        }],
    );
    assert!(applied.issues.is_empty());
    assert_eq!(applied.done, vec![0]);
    assert!(
        store
            .inspect(local.game_id)?
            .current()
            .unwrap()
            .game
            .personal
            .play_now
            .saved
    );
    Ok(())
}

#[test]
fn playing_timer_recovers_elapsed_time_without_counting_ui_ticks() -> anyhow::Result<()> {
    let active = ActivePlay {
        id: Uuid::new_v4(),
        library_id: Uuid::new_v4(),
        game_id: Uuid::new_v4(),
        title: "A game".into(),
        started_at: 1_000,
        context: Context::default(),
    };
    let restored: ActivePlay = serde_json::from_slice(&serde_json::to_vec(&active)?)?;
    assert_eq!(restored.timer(1_065), "01:05");
    assert_eq!(restored.timer(4_661), "1:01:01");
    assert_eq!(restored.elapsed(900), 0);
    // Ending after a backwards clock change still yields a valid choice.
    assert_eq!(
        restored
            .choice(LaunchOutcome::NotRequested, Some(900))
            .finished_at,
        Some(1_000)
    );
    Ok(())
}

#[test]
fn launch_retry_and_done_update_one_choice_and_preserve_imported_playtime() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let library = LibraryStore::create(temp.path().join("library"), "Test")?;
    let store = RecordStore::open(library.root())?;
    let base = store.create(record(1).game)?;
    let manifest = library.inspect()?.current().unwrap().clone();
    let active = ActivePlay {
        id: Uuid::new_v4(),
        library_id: manifest.library_id,
        game_id: base.game_id,
        title: "A game".into(),
        started_at: 100,
        context: Context::default(),
    };
    let mut personal = base.game.personal.clone();
    for (outcome, finished) in [
        (LaunchOutcome::Failed, None),
        (LaunchOutcome::Accepted, None),
        (LaunchOutcome::Accepted, Some(200)),
    ] {
        personal
            .play_now
            .record_choice(active.choice(outcome, finished));
    }
    assert_eq!(personal.play_now.recent.len(), 1);
    assert_eq!(personal.play_now.recent[0].at, 100);
    assert_eq!(personal.play_now.recent[0].finished_at, Some(200));
    let saved = library.edit_game_personal(&manifest, base.game_id, base.revision_id, personal)?;
    assert_eq!(saved.game.steam, base.game.steam);
    assert_eq!(saved.game.personal.status, base.game.personal.status);
    let reopened = store.inspect(base.game_id)?.current().unwrap().clone();
    assert_eq!(
        reopened.game.personal.play_now.recent[0].finished_at,
        Some(200)
    );
    let mut invalid = reopened.game.personal.play_now.clone();
    invalid.recent[0].finished_at = Some(99);
    assert!(invalid.validate().is_err());
    // Pre-timer records remain readable.
    let mut old = serde_json::to_value(active.choice(LaunchOutcome::NotRequested, None))?;
    old.as_object_mut().unwrap().remove("finished_at");
    assert_eq!(serde_json::from_value::<Choice>(old)?.finished_at, None);
    Ok(())
}
