//! App-managed storage initialization and bounded background refresh.
use super::GameSyncApp;
use crate::model::Library;
use anyhow::Context as _;
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use gamesync_desktop::library_reader::{LibraryReader, LoadedLibrary};
use gpui::{prelude::*, Context};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

impl GameSyncApp {
    pub(super) fn open_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.loading = true;
        self.refreshing = false;
        self.notice = "Preparing your games…".into();
        let samples = self.library.read(cx).games.clone();
        let sample_mode = !samples.is_empty();
        let (sender, events) = mpsc::channel(1);
        let refresh = sender.clone();
        // The worker starts now, while the window draws its first frame. It
        // sends the library that the index last recorded, then scans the
        // folder. The scan reads every revision and takes much longer.
        let (cached_sender, cached) = oneshot::channel();
        let root = path.clone();
        let load = cx.background_spawn(async move {
            // A store created now has no earlier contents to show.
            let existed = root.exists();
            crate::managed_storage::prepare(&root, &samples)?;
            crate::startup_trace::mark("storage_ready");
            let cache = directories::ProjectDirs::from("app", "GameSync", "GameSync")
                .context("Local cache directory is unavailable")?
                .cache_dir()
                .to_owned();
            let cache = crate::settings::preview_dir()
                .map(|p| p.join("cache"))
                .unwrap_or(cache);
            std::fs::create_dir_all(&cache)?;
            let mut reader = LibraryReader::open(&root, &cache)?;
            crate::startup_trace::mark("index_open");
            // An unusable index only delays the first view until the scan.
            let mut shown = None;
            if let Some(loaded) = existed
                .then(|| reader.last_valid().ok().flatten())
                .flatten()
            {
                shown = Some(loaded.digest());
                let mut model = Library::from_loaded(&loaded);
                model.demo = sample_mode;
                let _ = cached_sender.send((reader.root().to_path_buf(), model, loaded.issues));
            } else {
                drop(cached_sender);
            }

            let mut loaded = reader.refresh()?;
            crate::startup_trace::mark("scan_done");
            if sample_mode {
                match crate::managed_storage::fill_samples(&root, &loaded) {
                    Ok(true) => loaded = reader.refresh()?,
                    Ok(false) => {}
                    // Sample values are optional; the demo still opens.
                    Err(error) => log::warn!("Could not fill sample values: {error:#}"),
                }
            }
            // The scan usually confirms the indexed library. Then the view stays as it is.
            let model = (shown != Some(loaded.digest())).then(|| {
                let mut model = Library::from_loaded(&loaded);
                model.demo = sample_mode;
                model
            });
            let (watcher, warning) = match crate::watcher::watch(&root, sender) {
                Ok(watcher) => (Some(watcher), None),
                Err(error) => (
                    None,
                    Some(format!(
                        "Live updates unavailable: {error}. Checking every 10 seconds."
                    )),
                ),
            };
            let last_sync = crate::settings::load()?
                .last_sync
                .get(&loaded.manifest.library_id.to_string())
                .copied();
            Ok::<_, anyhow::Error>((reader, loaded, model, watcher, warning, last_sync))
        });
        self.load_task = Some(cx.spawn(async move |this, cx| {
            // A dropped sender means no indexed library; the scan result follows.
            if let Ok((path, model, issues)) = cached.await {
                let shown = this.update(cx, |this, cx| {
                    this.show_library(&path, model, &issues, cx);
                    crate::startup_trace::mark("cached_applied");
                });
                if shown.is_err() {
                    return;
                }
            }
            let (reader, loaded, model, watcher, watch_warning, last_sync) = match load.await {
                Ok(loaded) => loaded,
                Err(error) => {
                    // A library shown from the index stays on screen.
                    let _ = this.update(cx, |this, cx| {
                        this.loading = false;
                        this.review_folder = Some(path);
                        this.notice = format!("Could not load your games: {error:#}. Use Manage collections to review library definitions.");
                        cx.notify();
                    });
                    return;
                }
            };
            let path = reader.root().to_path_buf();
            let mut issues = loaded.issues.clone();
            if let Some(warning) = &watch_warning {
                issues.push(warning.clone());
            }
            let _ = this.update(cx, |this, cx| {
                this.last_sync = last_sync;
                match model {
                    Some(model) => this.show_library(&path, model, &issues, cx),
                    None => this.show_issues(&issues, cx),
                }
                crate::startup_trace::mark("library_applied");
                this.loading = false;
                this.refresh = Some(refresh);
                // Sync without waiting for the first periodic round.
                let sync_now = this.sync.read(cx).enabled() && !sample_mode;
                let live = LiveUpdates {
                    events,
                    watcher,
                    warning: watch_warning,
                    sample_mode,
                    sync_now,
                };
                this.start_refresh(reader, loaded, live, cx);
                // The status tooltip reads `loading` and `last_sync`.
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Display a library read from `path`. A second call for the same folder
    /// keeps selection, filters, and scroll position.
    fn show_library(
        &mut self,
        path: &std::path::Path,
        model: Library,
        issues: &[String],
        cx: &mut Context<Self>,
    ) {
        let same = self.folder.as_deref() == Some(path);
        if !same {
            if let Some(handle) = self.collections_window.take() {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
            self.collections_view = None;
            self.collection_root = None;
        }
        self.apply_library(model, issues, same, cx);
        self.folder = Some(path.to_path_buf());
        self.review_folder = None;
    }

    /// Keep the view current. A watcher event or a manual refresh scans the
    /// folder. While a working watcher reports no change, the last read is
    /// still the folder's state: sync rounds reuse it, and no scan runs.
    fn start_refresh(
        &mut self,
        mut reader: LibraryReader,
        loaded: LoadedLibrary,
        live: LiveUpdates,
        cx: &mut Context<Self>,
    ) {
        let LiveUpdates {
            mut events,
            watcher,
            warning: mut watch_warning,
            sample_mode,
            sync_now,
        } = live;
        self.watch_task = Some(cx.spawn(async move |this, cx| {
            let _watcher = watcher;
            // The library on screen, and the read that it came from.
            let mut shown = loaded.digest();
            let mut current = Some(loaded);
            let mut scanned = Instant::now();
            let mut wait = !sync_now;
            loop {
                let mut changed = false;
                if std::mem::replace(&mut wait, true) {
                    let timer = cx.background_executor().timer(ROUND_INTERVAL);
                    futures::pin_mut!(timer);
                    match futures::future::select(events.next(), timer).await {
                        futures::future::Either::Left((Some(event), _)) => {
                            changed = true;
                            if let Err(warning) = event {
                                watch_warning = Some(warning);
                            }
                            cx.background_executor()
                                .timer(Duration::from_millis(500))
                                .await;
                            // Drain the bounded burst before the next scan.
                            while let Ok(event) = events.try_recv() {
                                if let Err(warning) = event {
                                    watch_warning = Some(warning);
                                }
                            }
                        }
                        // Every sender is gone: the app is closing.
                        futures::future::Either::Left((None, _)) => return,
                        futures::future::Either::Right(_) => {}
                    }
                }
                let (sync_handle, sync_folder) = match this.update(cx, |this, cx| {
                    let sync = this.sync.read(cx);
                    (sync.handle.clone(), sync.folder.clone())
                }) {
                    Ok(sync) => sync,
                    Err(_) => return,
                };
                // Sample libraries never sync.
                let sync_folder = sync_folder.filter(|_| !sample_mode);
                // A failed watcher falls back to a scan on each tick. The
                // safety scan bounds the effect of an event that never arrived.
                let scan = changed
                    || current.is_none()
                    || watch_warning.is_some()
                    || scanned.elapsed() >= SAFETY_SCAN;
                if !scan && sync_folder.is_none() {
                    continue;
                }
                let previous = current.take().filter(|_| !scan);
                let (returned, result, synced, timing) = cx
                    .background_spawn(async move {
                        let mut synced = None;
                        let started = Instant::now();
                        let mut timing = (0., 0.);
                        let read = match previous {
                            Some(loaded) => Ok(loaded),
                            None => reader.refresh(),
                        };
                        let result = read.map(|mut loaded| {
                            timing.0 = started.elapsed().as_secs_f64() * 1000.;
                            // A round needs the library as it is on disk. Its
                            // writes check each game's revision, so a read that
                            // missed a change cannot replace newer data.
                            if let Some(folder) = sync_folder {
                                let round = Instant::now();
                                let outcome =
                                    sync_round(&mut reader, &mut loaded, sync_handle, &folder);
                                timing.1 = round.elapsed().as_secs_f64() * 1000.;
                                synced = Some(outcome);
                            }
                            let digest = loaded.digest();
                            let model = (digest != shown).then(|| {
                                let mut model = Library::from_loaded(&loaded);
                                model.demo = sample_mode;
                                model
                            });
                            (loaded, digest, model)
                        });
                        (reader, result, synced, timing)
                    })
                    .await;
                reader = returned;
                if scan {
                    scanned = Instant::now();
                }
                // After an error `current` stays empty, so the next tick scans.
                let result = result.map(|(loaded, digest, model)| {
                    let mut issues = loaded.issues.clone();
                    if let Some(warning) = &watch_warning {
                        issues.push(warning.clone());
                    }
                    current = Some(loaded);
                    shown = digest;
                    (model, issues)
                });
                crate::startup_trace::idle(timing.0, timing.1, matches!(&result, Ok((Some(_), _))));
                if this
                    .update(cx, |this, cx| {
                        if let Some(synced) = synced {
                            this.apply_sync(synced, cx);
                        }
                        this.refreshing = false;
                        match result {
                            Ok((Some(model), issues)) => {
                                this.apply_library(model, &issues, true, cx)
                            }
                            // Nothing changed. Leave the model and the view alone.
                            Ok((None, issues)) => this.show_issues(&issues, cx),
                            Err(error) => {
                                this.notice = format!(
                                    "Could not refresh: {error:#}. Showing last valid data."
                                );
                                cx.notify();
                            }
                        }
                    })
                    .is_err()
                {
                    return;
                }
            }
        }));
    }

    fn apply_library(
        &mut self,
        model: Library,
        issues: &[String],
        same: bool,
        cx: &mut Context<Self>,
    ) {
        if !same {
            self.detail_shown = false;
            self.restore_grid_focus = true;
            self.clear_search = true;
            self.detail.update(cx, |detail, cx| detail.clear(cx));
        }
        if !same || model.media_version != self.library.read(cx).media_version {
            self.cache.update(cx, |cache, cx| cache.clear(cx));
        }
        self.library.update(cx, |library, cx| {
            library.replace(model, same);
            cx.notify();
        });
        self.show_issues(issues, cx);
        cx.notify();
    }

    /// Show the first library issue as the notice. With no issue this clears
    /// a notice that an earlier action left. Renders only when the text changes.
    fn show_issues(&mut self, issues: &[String], cx: &mut Context<Self>) {
        let notice = if issues.is_empty() {
            String::new()
        } else {
            format!(
                "{}{}",
                issues[0],
                if issues.len() > 1 {
                    format!(" (+{} other issues)", issues.len() - 1)
                } else {
                    String::new()
                }
            )
        };
        if self.last_issues != issues {
            for issue in issues.iter().take(20) {
                log::warn!("{issue}");
            }
            self.last_issues = issues.to_vec();
        }
        if self.notice != notice {
            self.notice = notice;
            cx.notify();
        }
    }
}

/// Sync rounds run at this interval. See `plan/data-sync.md`.
const ROUND_INTERVAL: Duration = Duration::from_secs(10);
/// A full scan runs at least this often, also while the watcher reports no change.
const SAFETY_SCAN: Duration = Duration::from_secs(300);

/// What the refresh loop needs besides the reader and its first read.
struct LiveUpdates {
    events: mpsc::Receiver<Result<(), String>>,
    watcher: Option<notify::RecommendedWatcher>,
    warning: Option<String>,
    sample_mode: bool,
    /// Run a sync round at once, with the read from startup.
    sync_now: bool,
}

/// The result of one sync attempt in the refresh loop.
pub(super) struct SyncOutcome {
    /// An engine opened in this attempt; the app keeps it.
    pub opened: Option<std::sync::Arc<crate::sync_runtime::Handle>>,
    pub report: Result<crate::sync_runtime::Report, String>,
}

/// Open sync if needed, run one round, and read the library again when the
/// round wrote to it. A folder that cannot open is retried next time.
fn sync_round(
    reader: &mut LibraryReader,
    loaded: &mut LoadedLibrary,
    handle: Option<std::sync::Arc<crate::sync_runtime::Handle>>,
    folder: &std::path::Path,
) -> SyncOutcome {
    let (handle, opened) = match handle {
        Some(handle) => (handle, None),
        None => match crate::sync_runtime::open(folder, false) {
            Ok(handle) => (handle.clone(), Some(handle)),
            Err(error) => {
                return SyncOutcome {
                    opened: None,
                    report: Err(format!("{error:#}")),
                }
            }
        },
    };
    let report = crate::sync_runtime::run_round(&handle, reader.root(), loaded)
        .map_err(|error| format!("{error:#}"));
    if report.as_ref().is_ok_and(|r| r.library_changed) {
        match reader.refresh() {
            Ok(fresh) => *loaded = fresh,
            Err(error) => log::warn!("Could not read the library after sync: {error:#}"),
        }
    }
    SyncOutcome { opened, report }
}
