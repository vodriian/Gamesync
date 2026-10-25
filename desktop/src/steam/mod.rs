//! Sequential, cancellable Steam import. Stage completion lives with provider metadata.
pub mod client;
pub mod covers;
mod diagnostics;
use crate::{
    library::{LibraryStore, SteamChange, SteamImport},
    record_store::RecordStore,
    records::{GameRevision, SteamMetadata},
};
use anyhow::{Context, Result};
pub use client::{OwnedGame, SteamClient, WishlistItem};
pub use diagnostics::directory as log_directory;
use diagnostics::{Event, SyncLog};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Default, Clone)]
pub struct SyncProgress {
    pub message: String,
    pub imported: usize,
    pub new_games: usize,
    pub updated_games: usize,
    pub unchanged_games: usize,
    /// Number of missing metadata steps planned for this run, including tags.
    pub missing_details: usize,
    pub details_completed: usize,
    pub details_attempted: usize,
    pub failures: usize,
    pub cancelled: bool,
    pub issues: Vec<String>,
}
impl SyncProgress {
    fn count_change(&mut self, change: SteamChange) {
        match change {
            SteamChange::New => self.new_games += 1,
            SteamChange::Updated => self.updated_games += 1,
            SteamChange::Unchanged => self.unchanged_games += 1,
        }
    }
    fn counts(&self) -> String {
        format!(
            "{} new games · {} updated games · {} unchanged games",
            self.new_games, self.updated_games, self.unchanged_games
        )
    }
    fn detail_progress(&self) -> String {
        format!(
            "Fetching missing details · {} of {} steps checked · {} still missing",
            self.details_attempted,
            self.missing_details,
            self.missing_details - self.details_completed
        )
    }
}

fn missing_stages(metadata: &SteamMetadata) -> [bool; 5] {
    [
        !metadata.details_complete,
        !metadata.reviews_complete,
        !metadata.cover_complete,
        !metadata
            .setup
            .as_ref()
            .is_some_and(|setup| setup.workshop.is_some()),
        metadata.banner.is_none(),
    ]
}
pub type Cancellation = Arc<AtomicBool>;
fn cancelled(cancel: &Cancellation) -> bool {
    cancel.load(Ordering::Relaxed)
}
fn pause(cancel: &Cancellation, delay: Duration) {
    // Short waits keep cancellation responsive during request throttling.
    for _ in 0..(delay.as_millis() / 100) {
        if cancelled(cancel) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub fn sync(
    root: &Path,
    account: &str,
    key: &str,
    cancel: Cancellation,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncProgress> {
    let log = log_directory()
        .and_then(|folder| SyncLog::open(&folder, account, key))
        .map(Arc::new);
    let diagnostics = log.as_ref().ok().cloned();
    let result = (|| {
        let client = SteamClient::with_diagnostics(diagnostics.clone())?;
        let result = run_sync(
            root,
            account,
            key,
            cancel,
            &client,
            Duration::from_millis(1200),
            &mut progress,
            diagnostics.as_deref(),
        )?;
        // Prices use the profile country. A hidden country keeps the old value.
        match client.country(key, account) {
            Ok(Some(code)) => {
                if let Err(error) = crate::settings::update(|s| s.detected_country = Some(code)) {
                    if let Some(log) = &diagnostics {
                        log.record(Event::Issue {
                            detail: &format!("Save store country: {error:#}"),
                        });
                    }
                    log::warn!("Could not save the store country: {error:#}");
                }
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(log) = &diagnostics {
                    log.record(Event::Issue {
                        detail: &format!("Read store country: {error:#}"),
                    });
                }
                log::warn!("Could not read the profile country: {error:#}");
            }
        }
        Ok(result)
    })();
    finish_sync(result, log, &mut progress)
}

fn finish_sync(
    result: Result<SyncProgress>,
    log: Result<Arc<SyncLog>>,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<SyncProgress> {
    let feedback = match &log {
        Ok(log) => {
            match &result {
                Ok(state) => {
                    log.record(Event::Counts {
                        new_games: state.new_games,
                        updated_games: state.updated_games,
                        unchanged_games: state.unchanged_games,
                        missing_details: state.missing_details,
                        details_completed: state.details_completed,
                    });
                    log.finish(
                        if state.cancelled {
                            "cancelled"
                        } else if state.failures > 0 {
                            "needs_retry"
                        } else {
                            "success"
                        },
                        state.imported,
                        state.failures,
                    );
                }
                Err(error) => {
                    log.record(Event::Issue {
                        detail: &format!("Sync stopped: {error:#}"),
                    });
                    log.finish("failed", 0, 1);
                }
            }
            log.feedback()
        }
        Err(error) => format!("Could not create the Steam sync log: {error}"),
    };
    match result {
        Ok(mut state) => {
            state.message.push_str(&format!("\n{feedback}"));
            progress(state.clone());
            Ok(state)
        }
        Err(error) => Err(anyhow::anyhow!("{error}\n{feedback}")),
    }
}

/// The transport boundary lets recovery tests use fixed responses without real keys.
trait SteamSource {
    fn owned(&self, key: &str, account: &str) -> Result<Vec<OwnedGame>>;
    fn details(&self, id: u32) -> Result<serde_json::Value>;
    fn reviews(&self, id: u32) -> Result<serde_json::Value>;
    fn cover(&self, id: u32) -> Result<Vec<u8>>;
    fn banner(&self, id: u32) -> Result<Vec<u8>>;
    fn store_tags(&self, ids: &[u32]) -> Result<BTreeMap<u32, Vec<u32>>>;
    fn tag_names(&self) -> Result<BTreeMap<u32, String>>;
    fn wishlist(&self, account: &str) -> Result<Vec<WishlistItem>>;
    fn store_names(&self, ids: &[u32]) -> Result<BTreeMap<u32, String>>;
    fn deck_report(&self, id: u32) -> Result<serde_json::Value>;
    fn store_categories(&self, id: u32) -> Result<serde_json::Value>;
}
impl SteamSource for SteamClient {
    fn deck_report(&self, id: u32) -> Result<serde_json::Value> {
        self.deck_report(id)
    }
    fn store_categories(&self, id: u32) -> Result<serde_json::Value> {
        self.store_categories(id)
    }
    fn wishlist(&self, account: &str) -> Result<Vec<WishlistItem>> {
        self.wishlist(account)
    }
    fn store_names(&self, ids: &[u32]) -> Result<BTreeMap<u32, String>> {
        self.store_names(ids)
    }
    fn store_tags(&self, ids: &[u32]) -> Result<BTreeMap<u32, Vec<u32>>> {
        self.store_tags(ids)
    }
    fn tag_names(&self) -> Result<BTreeMap<u32, String>> {
        self.tag_names()
    }
    fn owned(&self, key: &str, account: &str) -> Result<Vec<OwnedGame>> {
        self.owned(key, account)
    }
    fn details(&self, id: u32) -> Result<serde_json::Value> {
        self.details(id)
    }
    fn reviews(&self, id: u32) -> Result<serde_json::Value> {
        self.reviews(id)
    }
    fn cover(&self, id: u32) -> Result<Vec<u8>> {
        self.cover(id)
    }
    fn banner(&self, id: u32) -> Result<Vec<u8>> {
        self.banner(id)
    }
}
#[allow(clippy::too_many_arguments)]
fn run_sync(
    root: &Path,
    account: &str,
    key: &str,
    cancel: Cancellation,
    client: &impl SteamSource,
    delay: Duration,
    progress: &mut impl FnMut(SyncProgress),
    log: Option<&SyncLog>,
) -> Result<SyncProgress> {
    if let Some(log) = log {
        log.record(Event::Stage {
            stage: "open_library",
            app_id: None,
        });
    }
    let library = LibraryStore::open(root)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(".steam-sync.lock"))?;
    lock.try_lock()
        .context("Steam sync is already running for this library")?;
    let mut state = SyncProgress {
        message: "Reading Steam library…".into(),
        ..Default::default()
    };
    progress(state.clone());
    if let Some(log) = log {
        log.record(Event::Stage {
            stage: "owned_games",
            app_id: None,
        });
    }
    let games = client.owned(key, account)?;
    state.message = "Scanning local library…".into();
    progress(state.clone());
    let mut imports = library.steam_import(account)?;
    if let Some(log) = log {
        log.record(Event::Stage {
            stage: "library_index_ready",
            app_id: None,
        });
    }
    let mut records = Vec::new();
    for game in &games {
        if cancelled(&cancel) {
            break;
        }
        match imports.import_steam(game) {
            Ok(imported) => {
                state.imported += 1;
                state.count_change(imported.change);
                records.push(imported.record);
            }
            Err(error) => {
                note_issue(
                    log,
                    &mut state,
                    1,
                    format!("{} (Steam {}) · import: {error:#}", game.name, game.appid),
                );
            }
        }
        state.message = format!(
            "Checking owned games · {} of {} · {}",
            state.imported,
            games.len(),
            state.counts()
        );
        progress(state.clone());
    }
    let store = RecordStore::open(root)?;
    let owned: std::collections::BTreeSet<u32> = games.iter().map(|g| g.appid).collect();
    records.extend(sync_wishlist(
        &mut imports,
        &store,
        account,
        &owned,
        &cancel,
        client,
        delay,
        &mut state,
        progress,
        log,
    ));
    drop(imports);
    state.missing_details = records
        .iter()
        .filter_map(|record| record.game.steam.as_ref())
        .map(|steam| {
            let metadata = steam.metadata.clone().unwrap_or_default();
            missing_stages(&metadata)
                .into_iter()
                .filter(|missing| *missing)
                .count()
                + usize::from(!metadata.tags_complete)
        })
        .sum();
    if state.missing_details > 0 && !cancelled(&cancel) {
        state.message = format!("{} · {}", state.counts(), state.detail_progress());
        progress(state.clone());
    }
    sync_tags(
        &store, &records, &cancel, client, delay, &mut state, progress, log,
    );
    for record in &records {
        if cancelled(&cancel) {
            break;
        }
        let Some(steam) = &record.game.steam else {
            continue;
        };
        let id = steam.app_id;
        let metadata = steam.metadata.clone().unwrap_or_default();
        for (stage, missing) in missing_stages(&metadata).into_iter().enumerate() {
            if cancelled(&cancel) {
                break;
            }
            if !missing {
                continue;
            }
            pause(&cancel, delay);
            if cancelled(&cancel) {
                break;
            }
            let stage_name = [
                "description",
                "reviews",
                "cover",
                "Steam Deck",
                "landscape cover",
            ][stage];
            if let Some(log) = log {
                log.record(Event::Stage {
                    stage: stage_name,
                    app_id: Some(id),
                });
            }
            let result = match stage {
                0 => client
                    .details(id)
                    .and_then(|data| apply_details(&store, record.game_id, id, &data)),
                1 => client
                    .reviews(id)
                    .and_then(|data| apply_reviews(&store, record.game_id, id, &data)),
                2 => client
                    .cover(id)
                    .and_then(|bytes| covers::save(root, record.game_id, id, &bytes).map(|_| ())),
                // Two store requests; the pause keeps the usual request spacing.
                3 => client.deck_report(id).and_then(|report| {
                    pause(&cancel, delay);
                    let categories = client.store_categories(id)?;
                    apply_setup(&store, record.game_id, id, &report, &categories)
                }),
                _ => client.banner(id).and_then(|bytes| {
                    covers::save_banner(root, record.game_id, id, &bytes).map(|_| ())
                }),
            };
            state.details_attempted += 1;
            if let Err(error) = result {
                note_issue(
                    log,
                    &mut state,
                    1,
                    format!(
                        "{} (Steam {id}) · {stage_name}: {error:#}",
                        record.game.title
                    ),
                );
            } else {
                state.details_completed += 1;
            }
            state.message = format!("{} · {}", state.counts(), state.detail_progress());
            progress(state.clone());
        }
    }
    state.cancelled = cancelled(&cancel);
    state.message =
        if state.cancelled {
            format!("Sync cancelled · {}. Saved progress stays.", state.counts())
        } else if state.failures > 0 {
            format!(
            "Sync finished · {} · {} missing detail steps · {} items need retry or conflict review",
            state.counts(), state.missing_details - state.details_completed, state.failures
        )
        } else {
            format!(
                "Sync finished · {} · {} missing detail steps",
                state.counts(),
                state.missing_details - state.details_completed
            )
        };
    if !state.issues.is_empty() {
        state
            .message
            .push_str(&format!("\n{}", state.issues.join("\n")));
    }
    progress(state.clone());
    Ok(state)
}

fn apply_details(
    store: &RecordStore,
    game_id: uuid::Uuid,
    app_id: u32,
    data: &serde_json::Value,
) -> Result<()> {
    let description = data["short_description"]
        .as_str()
        .context("Description is missing")?
        .to_owned();
    let genres = data["genres"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| g["description"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let year = data["release_date"]["date"]
        .as_str()
        .and_then(|date| {
            date.split(|c: char| !c.is_ascii_digit())
                .find(|p| p.len() == 4)
        })
        .map(str::to_owned);
    store.update_steam(game_id, app_id, |steam| {
        steam.description = Some(description);
        let metadata = steam.metadata.get_or_insert_with(SteamMetadata::default);
        metadata.genres = genres;
        metadata.release_year = year;
        metadata.details_complete = true;
    })?;
    Ok(())
}

fn apply_reviews(
    store: &RecordStore,
    game_id: uuid::Uuid,
    app_id: u32,
    data: &serde_json::Value,
) -> Result<()> {
    let total = data["total_reviews"]
        .as_u64()
        .context("Review count is missing")?;
    let positive = data["total_positive"]
        .as_u64()
        .context("Positive count is missing")?;
    anyhow::ensure!(positive <= total, "Invalid review counts");
    store.update_steam(game_id, app_id, |steam| {
        let metadata = steam.metadata.get_or_insert_with(SteamMetadata::default);
        metadata.review_label = data["review_score_desc"].as_str().map(str::to_owned);
        metadata.review_percent =
            (total > 0).then(|| ((positive as f64 / total as f64) * 100.).round() as u8);
        metadata.reviews_complete = true;
    })?;
    Ok(())
}

fn apply_setup(
    store: &RecordStore,
    game_id: uuid::Uuid,
    app_id: u32,
    report: &serde_json::Value,
    categories: &serde_json::Value,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    let evidence = client::parse_setup(report, categories, now)?;
    store.update_steam(game_id, app_id, |steam| {
        steam
            .metadata
            .get_or_insert_with(SteamMetadata::default)
            .setup = Some(evidence);
    })?;
    Ok(())
}

/// A tag added after the name list was built is skipped.
fn apply_tags(
    store: &RecordStore,
    game_id: uuid::Uuid,
    app_id: u32,
    tag_ids: &[u32],
    names: &BTreeMap<u32, String>,
) -> Result<GameRevision> {
    let tags = tag_ids
        .iter()
        .filter_map(|tag| names.get(tag).cloned())
        .collect::<Vec<_>>();
    store.update_steam(game_id, app_id, |steam| {
        let metadata = steam.metadata.get_or_insert_with(SteamMetadata::default);
        metadata.tags = tags;
        metadata.tags_complete = true;
    })
}

/// Fetch store details, reviews, Steam tags, and Steam Deck evidence for one
/// game again, even when its stages are complete. No key is needed. Returns one message for each
/// part that failed; parts that succeed are kept.
pub fn resync_game(root: &Path, game_id: uuid::Uuid) -> Result<Vec<String>> {
    let store = RecordStore::open(root)?;
    let snapshot = store.inspect(game_id)?;
    let app_id = snapshot
        .current()
        .context("Resolve this game's conflict before resyncing")?
        .game
        .steam
        .as_ref()
        .context("This game is not linked to Steam")?
        .app_id;
    let client = SteamClient::new()?;
    let mut failures = Vec::new();
    if let Err(error) = client
        .details(app_id)
        .and_then(|data| apply_details(&store, game_id, app_id, &data))
    {
        failures.push(format!("description: {error}"));
    }
    if let Err(error) = client
        .reviews(app_id)
        .and_then(|data| apply_reviews(&store, game_id, app_id, &data))
    {
        failures.push(format!("reviews: {error}"));
    }
    let tags = client.tag_names().and_then(|names| {
        let tag_ids = client
            .store_tags(&[app_id])?
            .remove(&app_id)
            .context("Store tags are unavailable")?;
        apply_tags(&store, game_id, app_id, &tag_ids, &names)
    });
    if let Err(error) = tags {
        failures.push(format!("tags: {error}"));
    }
    let setup = client.deck_report(app_id).and_then(|report| {
        let categories = client.store_categories(app_id)?;
        apply_setup(&store, game_id, app_id, &report, &categories)
    });
    if let Err(error) = setup {
        failures.push(format!("Steam Deck: {error}"));
    }
    if let Err(error) = client
        .banner(app_id)
        .and_then(|bytes| covers::save_banner(root, game_id, app_id, &bytes))
    {
        failures.push(format!("landscape cover: {error}"));
    }
    Ok(failures)
}

/// Count `stages` failed stages under one reported issue.
fn note_issue(log: Option<&SyncLog>, state: &mut SyncProgress, stages: usize, issue: String) {
    // The UI keeps only 20 issues; the bounded disk log also records later failures.
    if let Some(log) = log {
        log.record(Event::Issue { detail: &issue });
    }
    state.failures += stages;
    if state.issues.len() < 20 {
        state.issues.push(issue);
    }
}

/// Import the wishlist, then mark games that left it. Removal marks are written
/// only after a complete, successful pass, so a failed request never removes
/// anything. Returns wishlist records for the enrichment stages.
#[allow(clippy::too_many_arguments)]
fn sync_wishlist(
    imports: &mut SteamImport<'_>,
    store: &RecordStore,
    account: &str,
    owned: &std::collections::BTreeSet<u32>,
    cancel: &Cancellation,
    client: &impl SteamSource,
    delay: Duration,
    state: &mut SyncProgress,
    progress: &mut impl FnMut(SyncProgress),
    log: Option<&SyncLog>,
) -> Vec<GameRevision> {
    if cancelled(cancel) {
        return Vec::new();
    }
    state.message = "Reading Steam wishlist…".into();
    progress(state.clone());
    if let Some(log) = log {
        log.record(Event::Stage {
            stage: "wishlist",
            app_id: None,
        });
    }
    let items = match client.wishlist(account) {
        // Owned games are never wishlist records, even while Steam lists both.
        Ok(items) => items
            .into_iter()
            .filter(|item| !owned.contains(&item.appid))
            .collect::<Vec<_>>(),
        Err(error) => {
            note_issue(log, state, 1, format!("Wishlist: {error:#}"));
            return Vec::new();
        }
    };
    let existing = imports.records();
    let known: std::collections::BTreeSet<u32> = existing
        .iter()
        .filter_map(|r| r.game.steam.as_ref().map(|s| s.app_id))
        .collect();
    // Only new games need a store name.
    let new_ids: Vec<u32> = items
        .iter()
        .map(|item| item.appid)
        .filter(|id| !known.contains(id))
        .collect();
    let failures = state.failures;
    let mut names = BTreeMap::new();
    for batch in new_ids.chunks(client::TAG_BATCH) {
        pause(cancel, delay);
        if cancelled(cancel) {
            return Vec::new();
        }
        match client.store_names(batch) {
            Ok(found) => names.extend(found),
            Err(error) => note_issue(
                log,
                state,
                batch.len(),
                format!("Wishlist names for {} games: {error}", batch.len()),
            ),
        }
    }
    let mut imported = Vec::new();
    for item in &items {
        if cancelled(cancel) {
            return imported;
        }
        let name = match names.get(&item.appid) {
            Some(name) => name.as_str(),
            // The update path never uses the title.
            None if known.contains(&item.appid) => "",
            None => {
                note_issue(
                    log,
                    state,
                    1,
                    format!("Wishlist game {}: store name is unavailable", item.appid),
                );
                continue;
            }
        };
        match imports.import_wishlist(item, name) {
            Ok(result) => {
                state.count_change(result.change);
                imported.push(result.record);
            }
            Err(error) => note_issue(
                log,
                state,
                1,
                format!("Wishlist game {}: {error:#}", item.appid),
            ),
        }
    }
    if state.failures == failures {
        let listed: std::collections::BTreeSet<u32> = items.iter().map(|i| i.appid).collect();
        for record in &existing {
            let Some(steam) = &record.game.steam else {
                continue;
            };
            let leaving = !steam.owned
                && steam.wishlist.is_some_and(|w| !w.removed)
                && !listed.contains(&steam.app_id);
            if leaving {
                let result =
                    store.update_steam_with_change(record.game_id, steam.app_id, |steam| {
                        if let Some(wishlist) = &mut steam.wishlist {
                            wishlist.removed = true;
                        }
                    });
                match result {
                    Ok((_, true)) => state.updated_games += 1,
                    Ok((_, false)) => {}
                    Err(error) => {
                        note_issue(
                            log,
                            state,
                            1,
                            format!(
                                "{} (Steam {}) · wishlist: {error:#}",
                                record.game.title, steam.app_id
                            ),
                        );
                    }
                }
            }
        }
    }
    state.message = format!("Wishlist checked · {}", state.counts());
    progress(state.clone());
    imported
}

/// Store tags are batched: one request covers up to `TAG_BATCH` games. The tag
/// name list is fetched only when some game still needs tags.
#[allow(clippy::too_many_arguments)]
fn sync_tags(
    store: &RecordStore,
    records: &[GameRevision],
    cancel: &Cancellation,
    client: &impl SteamSource,
    delay: Duration,
    state: &mut SyncProgress,
    progress: &mut impl FnMut(SyncProgress),
    log: Option<&SyncLog>,
) {
    let pending = records
        .iter()
        .filter_map(|record| {
            let steam = record.game.steam.as_ref()?;
            let complete = steam.metadata.as_ref().is_some_and(|m| m.tags_complete);
            (!complete).then_some((record, steam.app_id))
        })
        .collect::<Vec<_>>();
    if pending.is_empty() || cancelled(cancel) {
        return;
    }
    state.message = format!("{} · {}", state.counts(), state.detail_progress());
    progress(state.clone());
    if let Some(log) = log {
        log.record(Event::Stage {
            stage: "tags",
            app_id: None,
        });
    }
    let names = match client.tag_names() {
        Ok(names) => names,
        Err(error) => {
            state.details_attempted += pending.len();
            note_issue(log, state, pending.len(), format!("Steam tags: {error:#}"));
            return;
        }
    };
    for batch in pending.chunks(client::TAG_BATCH) {
        pause(cancel, delay);
        if cancelled(cancel) {
            return;
        }
        let ids = batch.iter().map(|(_, id)| *id).collect::<Vec<_>>();
        match client.store_tags(&ids) {
            Ok(tags) => {
                for (record, id) in batch {
                    let result =
                        tags.get(id)
                            .context("Store tags are unavailable")
                            .and_then(|tag_ids| {
                                apply_tags(store, record.game_id, *id, tag_ids, &names)
                            });
                    if let Err(error) = result {
                        note_issue(
                            log,
                            state,
                            1,
                            format!("{} (Steam {id}) · tags: {error:#}", record.game.title),
                        );
                    } else {
                        state.details_completed += 1;
                    }
                }
            }
            Err(error) => note_issue(
                log,
                state,
                batch.len(),
                format!("Steam tags for {} games: {error}", batch.len()),
            ),
        }
        state.details_attempted += batch.len();
        state.message = format!("{} · {}", state.counts(), state.detail_progress());
        progress(state.clone());
    }
}

#[cfg(test)]
mod tests;
