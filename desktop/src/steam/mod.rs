//! Sequential, cancellable Steam import. Stage completion lives with provider metadata.
pub mod client;
pub mod covers;
use crate::{library::LibraryStore, record_store::RecordStore, records::SteamMetadata};
use anyhow::{Context, Result};
pub use client::{OwnedGame, SteamClient};
use std::{
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
    run_sync(
        root,
        account,
        key,
        cancel,
        &client,
        Duration::from_millis(1200),
        &mut progress,
    )
}

/// The transport boundary lets recovery tests use fixed responses without real keys.
trait SteamSource {
    fn owned(&self, key: &str, account: &str) -> Result<Vec<OwnedGame>>;
    fn details(&self, id: u32) -> Result<serde_json::Value>;
    fn reviews(&self, id: u32) -> Result<serde_json::Value>;
    fn cover(&self, id: u32) -> Result<Vec<u8>>;
}
impl SteamSource for SteamClient {
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
                0 => client.details(id).and_then(|data| {
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
                    store
                        .update_steam(record.game_id, id, |steam| {
                            steam.description = Some(description);
                            let metadata =
                                steam.metadata.get_or_insert_with(SteamMetadata::default);
                            metadata.genres = genres;
                            metadata.release_year = year;
                            metadata.details_complete = true;
                        })
                        .map(|_| ())
                }),
                1 => client.reviews(id).and_then(|data| {
                    let total = data["total_reviews"]
                        .as_u64()
                        .context("Review count is missing")?;
                    let positive = data["total_positive"]
                        .as_u64()
                        .context("Positive count is missing")?;
                    anyhow::ensure!(positive <= total, "Invalid review counts");
                    store
                        .update_steam(record.game_id, id, |steam| {
                            let metadata =
                                steam.metadata.get_or_insert_with(SteamMetadata::default);
                            metadata.review_label =
                                data["review_score_desc"].as_str().map(str::to_owned);
                            metadata.review_percent = (total > 0)
                                .then(|| ((positive as f64 / total as f64) * 100.).round() as u8);
                            metadata.reviews_complete = true;
                        })
                        .map(|_| ())
                }),
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

#[cfg(test)]
mod tests;
