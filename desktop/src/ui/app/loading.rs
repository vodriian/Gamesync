//! Folder selection and bounded background refresh, based on Eagle's app flow.
use super::GameSyncApp;
use crate::model::Library;
use anyhow::Context as _;
use futures::{channel::mpsc, StreamExt};
use gamesync_desktop::library_reader::LibraryReader;
use gpui::{prelude::*, Context, PathPromptOptions};
use std::{path::PathBuf, time::Duration};

impl GameSyncApp {
    pub(super) fn choose_folder(&mut self, cx: &mut Context<Self>) {
        if self.loading || !self.can_close(cx) {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Library".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| this.open_folder(path, cx));
                }
            }
        })
        .detach();
    }

    pub(super) fn open_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.loading = true;
        self.refreshing = false;
        self.refreshed = false;
        self.notice = "Opening library…".into();
        let (sender, events) = mpsc::channel(1);
        let refresh = sender.clone();
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let root = path.clone();
            let opened = cx
                .background_spawn(async move {
                    let cache = directories::ProjectDirs::from("app", "GameSync", "GameSync")
                        .context("Local cache directory is unavailable")?
                        .cache_dir()
                        .to_owned();
                    std::fs::create_dir_all(&cache)?;
                    let mut reader = LibraryReader::open(&root, &cache)?;
                    let mut loaded = reader.refresh()?;
                    if let Err(error) = crate::settings::remember_library(reader.root()) {
                        loaded.issues.push(format!(
                            "Opened library, but could not remember its location: {error:#}"
                        ));
                    }
                    let model = Library::from_loaded(&loaded);
                    let (watcher, warning) = match crate::watcher::watch(&root, sender) {
                        Ok(watcher) => (Some(watcher), None),
                        Err(error) => (
                            None,
                            Some(format!(
                                "Live updates unavailable: {error}. Checking every 10 seconds."
                            )),
                        ),
                    };
                    Ok::<_, anyhow::Error>((reader, model, loaded.issues, watcher, warning))
                })
                .await;
            let (reader, model, mut issues, watcher, watch_warning) = match opened {
                Ok(opened) => opened,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.loading = false;
                        this.notice = format!("Could not open library: {error:#}");
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
                this.apply_library(model, &issues, same, cx);
                this.folder = Some(path);
                this.loading = false;
                this.refresh = Some(refresh);
                this.start_refresh(reader, events, watcher, watch_warning, cx);
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
                        let result = reader
                            .refresh()
                            .map(|loaded| (Library::from_loaded(&loaded), loaded.issues));
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
                                this.refreshed = true;
                                this.refreshing = false;
                            }
                            this.apply_library(model, &issues, true, cx);
                        }
                        Err(error) => {
                            this.refreshing = false;
                            this.refreshed = false;
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
