//! App-managed storage initialization and bounded background refresh.
use super::GameSyncApp;
use crate::model::Library;
use anyhow::Context as _;
use futures::{channel::mpsc, StreamExt};
use gamesync_desktop::library_reader::LibraryReader;
use gpui::{prelude::*, Context};
use std::{path::PathBuf, time::Duration};

impl GameSyncApp {
    pub(super) fn open_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.loading = true;
        self.refreshing = false;
        self.notice = "Preparing your games…".into();
        let samples = self.library.read(cx).games.clone();
        let sample_mode = !samples.is_empty();
        let (sender, events) = mpsc::channel(1);
        let refresh = sender.clone();
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let root = path.clone();
            let opened = cx
                .background_spawn(async move {
                    crate::managed_storage::prepare(&root, &samples)?;
                    let cache = directories::ProjectDirs::from("app", "GameSync", "GameSync")
                        .context("Local cache directory is unavailable")?
                        .cache_dir()
                        .to_owned();
                    let cache = crate::settings::preview_dir().map(|p| p.join("cache")).unwrap_or(cache);
                    std::fs::create_dir_all(&cache)?;
                    let mut reader = LibraryReader::open(&root, &cache)?;
                    let mut loaded = reader.refresh()?;
                    if sample_mode {
                        match crate::managed_storage::fill_samples(&root, &loaded) {
                            Ok(true) => loaded = reader.refresh()?,
                            Ok(false) => {}
                            // Sample values are optional; the demo still opens.
                            Err(error) => log::warn!("Could not fill sample values: {error:#}"),
                        }
                    }
                    let mut model = Library::from_loaded(&loaded);
                    model.demo = sample_mode;
                    let (watcher, warning) = match crate::watcher::watch(&root, sender) {
                        Ok(watcher) => (Some(watcher), None),
                        Err(error) => (
                            None,
                            Some(format!(
                                "Live updates unavailable: {error}. Checking every 10 seconds."
                            )),
                        ),
                    };
                    let last_sync = crate::settings::load()?.last_sync.get(&loaded.manifest.library_id.to_string()).copied();
                    Ok::<_, anyhow::Error>((reader, model, loaded.issues, watcher, warning, last_sync))
                })
                .await;
            let (reader, model, mut issues, watcher, watch_warning, last_sync) = match opened {
                Ok(opened) => opened,
                Err(error) => {
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
            if let Some(warning) = &watch_warning {
                issues.push(warning.clone());
            }
            let _ = this.update(cx, |this, cx| {
                let same = this.folder.as_ref() == Some(&path);
                if !same {
                    if let Some(handle) = this.collections_window.take() { let _ = handle.update(cx, |_, window, _| window.remove_window()); }
                    this.collections_view = None;
                    this.collection_root = None;
                }
                this.last_sync = last_sync;
                this.apply_library(model, &issues, same, cx);
                this.folder = Some(path);
                this.review_folder = None;
                this.loading = false;
                this.refresh = Some(refresh);
                this.start_refresh(reader, events, watcher, watch_warning, sample_mode, cx);
                // Sync without waiting for the first periodic scan.
                if this.sync.read(cx).enabled() && !sample_mode {
                    this.refresh_library(cx);
                }
            });
        }));
        cx.notify();
    }

    fn start_refresh(
        &mut self,
        mut reader: LibraryReader,
        mut events: mpsc::Receiver<Result<(), String>>,
        watcher: Option<notify::RecommendedWatcher>,
        mut watch_warning: Option<String>,
        sample_mode: bool,
        cx: &mut Context<Self>,
    ) {
        self.watch_task = Some(cx.spawn(async move |this, cx| {
            let _watcher = watcher;
            loop {
                let timer = cx.background_executor().timer(Duration::from_secs(10));
                futures::pin_mut!(timer);
                if let futures::future::Either::Left((Some(event), _)) =
                    futures::future::select(events.next(), timer).await
                {
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
                let (sync_handle, sync_folder) = match this.update(cx, |this, cx| {
                    let sync = this.sync.read(cx);
                    (sync.handle.clone(), sync.folder.clone())
                }) {
                    Ok(sync) => sync,
                    Err(_) => return,
                };
                let (returned, result, synced) = cx
                    .background_spawn(async move {
                        let mut synced = None;
                        let result = reader.refresh().map(|mut loaded| {
                            // Sample libraries never sync. A round uses the
                            // library just read, as the engine requires.
                            if let Some(folder) = sync_folder.filter(|_| !sample_mode) {
                                let outcome =
                                    sync_round(&mut reader, &mut loaded, sync_handle, &folder);
                                synced = Some(outcome);
                            }
                            let mut model = Library::from_loaded(&loaded);
                            model.demo = sample_mode;
                            (model, loaded.issues)
                        });
                        (reader, result, synced)
                    })
                    .await;
                reader = returned;
                if this
                    .update(cx, |this, cx| {
                        if let Some(synced) = synced {
                            this.apply_sync(synced, cx);
                        }
                        match result {
                            Ok((model, mut issues)) => {
                                if let Some(warning) = &watch_warning {
                                    issues.push(warning.clone());
                                }
                                if this.refreshing {
                                    this.refreshing = false;
                                }
                                this.apply_library(model, &issues, true, cx);
                            }
                            Err(error) => {
                                this.refreshing = false;
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
        self.notice = if issues.is_empty() {
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
        cx.notify();
    }
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
    loaded: &mut gamesync_desktop::library_reader::LoadedLibrary,
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
