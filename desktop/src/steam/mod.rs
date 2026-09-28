//! Sequential, cancellable Steam import. Stage completion lives with provider metadata.
pub mod client;
pub mod covers;
use crate::{
    library::LibraryStore,
    record_store::RecordStore,
    records::{GameRevision, SteamMetadata},
};
use anyhow::{Context, Result};
pub use client::{OwnedGame, SteamClient, WishlistItem};
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
    pub failures: usize,
    pub cancelled: bool,
    pub issues: Vec<String>,
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
    let client = SteamClient::new()?;
    let result = run_sync(
        root,
        account,
        key,
        cancel,
        &client,
        Duration::from_millis(1200),
        &mut progress,
    )?;
    // Prices use the profile country. A hidden country keeps the old value.
    match client.country(key, account) {
        Ok(Some(code)) => {
            if let Err(error) = crate::settings::update(|s| s.detected_country = Some(code)) {
                log::warn!("Could not save the store country: {error:#}");
            }
        }
        Ok(None) => {}
        Err(error) => log::warn!("Could not read the profile country: {error:#}"),
    }
    Ok(result)
}

/// The transport boundary lets recovery tests use fixed responses without real keys.
trait SteamSource {
    fn owned(&self, key: &str, account: &str) -> Result<Vec<OwnedGame>>;
    fn details(&self, id: u32) -> Result<serde_json::Value>;
    fn reviews(&self, id: u32) -> Result<serde_json::Value>;
    fn cover(&self, id: u32) -> Result<Vec<u8>>;
    fn store_tags(&self, ids: &[u32]) -> Result<BTreeMap<u32, Vec<u32>>>;
    fn tag_names(&self) -> Result<BTreeMap<u32, String>>;
    fn wishlist(&self, account: &str) -> Result<Vec<WishlistItem>>;
    fn store_names(&self, ids: &[u32]) -> Result<BTreeMap<u32, String>>;
}
impl SteamSource for SteamClient {
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
}
fn run_sync(
    root: &Path,
    account: &str,
    key: &str,
    cancel: Cancellation,
    client: &impl SteamSource,
    delay: Duration,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<SyncProgress> {
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
    let games = client.owned(key, account)?;
    let mut records = Vec::new();
    for game in &games {
        if cancelled(&cancel) {
            break;
        }
        match library.import_steam(account, game) {
            Ok(record) => {
                state.imported += 1;
                records.push(record);
            }
            Err(error) => {
                state.failures += 1;
                if state.issues.len() < 20 {
                    state.issues.push(format!("{}: {error}", game.name));
                }
            }
        }
        state.message = format!(
            "Imported {} of {} games · {} need attention",
            state.imported,
            games.len(),
            state.failures
        );
        progress(state.clone());
    }
    let store = RecordStore::open(root)?;
    let owned: std::collections::BTreeSet<u32> = games.iter().map(|g| g.appid).collect();
    records.extend(sync_wishlist(
        &library, &store, account, &owned, &cancel, client, delay, &mut state, progress,
    ));
    sync_tags(
        &store, &records, &cancel, client, delay, &mut state, progress,
    );
    for (index, record) in records.iter().enumerate() {
        if cancelled(&cancel) {
            break;
        }
        let Some(steam) = &record.game.steam else {
            continue;
        };
        let id = steam.app_id;
        let metadata = steam.metadata.clone().unwrap_or_default();
        for stage in 0..3 {
            if cancelled(&cancel) {
                break;
            }
            if [
                metadata.details_complete,
                metadata.reviews_complete,
                metadata.cover_complete,
            ][stage]
            {
                continue;
            }
            pause(&cancel, delay);
            if cancelled(&cancel) {
                break;
            }
            let result = match stage {
                0 => client
                    .details(id)
                    .and_then(|data| apply_details(&store, record.game_id, id, &data)),
                1 => client
                    .reviews(id)
                    .and_then(|data| apply_reviews(&store, record.game_id, id, &data)),
                _ => client
                    .cover(id)
                    .and_then(|bytes| covers::save(root, record.game_id, id, &bytes).map(|_| ())),
            };
            if let Err(error) = result {
                state.failures += 1;
                if state.issues.len() < 20 {
                    state.issues.push(format!(
                        "{} · {}: {error}",
                        record.game.title,
                        ["description", "reviews", "cover"][stage]
                    ));
                }
            }
            state.message = format!(
                "Loading details · {} of {} games · {} stages need retry",
                index + 1,
                records.len(),
                state.failures
            );
            progress(state.clone());
        }
    }
    state.cancelled = cancelled(&cancel);
    state.message = if state.cancelled {
        "Sync cancelled. Saved progress stays.".into()
    } else if state.failures > 0 {
        format!(
            "Sync finished · {} games · {} items need retry or conflict review",
            state.imported, state.failures
        )
    } else {
        format!("Synced {} games", state.imported)
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

/// Fetch store details, reviews, and Steam tags for one game again, even when
/// its stages are complete. No key is needed. Returns one message for each
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
    Ok(failures)
}

/// Count `stages` failed stages under one reported issue.
fn note_issue(state: &mut SyncProgress, stages: usize, issue: String) {
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
    library: &LibraryStore,
    store: &RecordStore,
    account: &str,
    owned: &std::collections::BTreeSet<u32>,
    cancel: &Cancellation,
    client: &impl SteamSource,
    delay: Duration,
    state: &mut SyncProgress,
    progress: &mut impl FnMut(SyncProgress),
) -> Vec<GameRevision> {
    if cancelled(cancel) {
        return Vec::new();
    }
    state.message = "Reading Steam wishlist…".into();
    progress(state.clone());
    let items = match client.wishlist(account) {
        // Owned games are never wishlist records, even while Steam lists both.
        Ok(items) => items
            .into_iter()
            .filter(|item| !owned.contains(&item.appid))
            .collect::<Vec<_>>(),
        Err(error) => {
            note_issue(state, 1, format!("Wishlist: {error}"));
            return Vec::new();
        }
    };
    let existing = match library.steam_records() {
        Ok(records) => records,
        Err(error) => {
            note_issue(state, 1, format!("Wishlist: {error}"));
            return Vec::new();
        }
    };
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
                    state,
                    1,
                    format!("Wishlist game {}: store name is unavailable", item.appid),
                );
                continue;
            }
        };
        match library.import_wishlist(account, item, name) {
            Ok(record) => imported.push(record),
            Err(error) => note_issue(state, 1, format!("Wishlist game {}: {error}", item.appid)),
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
                let result = store.update_steam(record.game_id, steam.app_id, |steam| {
                    if let Some(wishlist) = &mut steam.wishlist {
                        wishlist.removed = true;
                    }
                });
                if let Err(error) = result {
                    note_issue(
                        state,
                        1,
                        format!("{} · wishlist: {error}", record.game.title),
                    );
                }
            }
        }
    }
    state.message = format!("Wishlist · {} games", imported.len());
    progress(state.clone());
    imported
}

/// Store tags are batched: one request covers up to `TAG_BATCH` games. The tag
/// name list is fetched only when some game still needs tags.
fn sync_tags(
    store: &RecordStore,
    records: &[GameRevision],
    cancel: &Cancellation,
    client: &impl SteamSource,
    delay: Duration,
    state: &mut SyncProgress,
    progress: &mut impl FnMut(SyncProgress),
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
    state.message = "Loading Steam tags…".into();
    progress(state.clone());
    let names = match client.tag_names() {
        Ok(names) => names,
        Err(error) => {
            note_issue(state, pending.len(), format!("Steam tags: {error}"));
            return;
        }
    };
    let mut done = 0;
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
                        note_issue(state, 1, format!("{} · tags: {error}", record.game.title));
                    }
                }
            }
            Err(error) => note_issue(
                state,
                batch.len(),
                format!("Steam tags for {} games: {error}", batch.len()),
            ),
        }
        done += batch.len();
        state.message = format!(
            "Loading Steam tags · {done} of {} games · {} stages need retry",
            pending.len(),
            state.failures
        );
        progress(state.clone());
    }
}

#[cfg(test)]
mod tests;
