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
                    std::fs::create_dir_all(&cache)?;
                    let mut reader = LibraryReader::open(&root, &cache)?;
                    let loaded = reader.refresh()?;
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
                let (returned, result) = cx
                    .background_spawn(async move {
                        let result = reader.refresh().map(|loaded| {
                            let mut model = Library::from_loaded(&loaded);
                            model.demo = sample_mode;
                            (model, loaded.issues)
                        });
                        (reader, result)
                    })
                    .await;
                reader = returned;
                if this
                    .update(cx, |this, cx| match result {
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
                            this.notice =
                                format!("Could not refresh: {error:#}. Showing last valid data.");
                            cx.notify();
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
