//! Follow Omarchy's atomically replaced current-theme directory.

use super::GameSyncApp;
use futures::{channel::mpsc, StreamExt};
use gpui::{prelude::*, Context};
use std::time::Duration;

impl GameSyncApp {
    pub(super) fn set_omarchy_mode(
        &mut self,
        enabled: bool,
        theme: Option<gamesync_desktop::omarchy::OmarchyTheme>,
        cx: &mut Context<Self>,
    ) {
        self.omarchy_mode = enabled;
        self.omarchy_theme = theme;
        self.omarchy_watch_task = None;
        if enabled {
            self.start_omarchy_sync(cx);
        }
        cx.notify();
    }

    pub(super) fn start_omarchy_sync(&mut self, cx: &mut Context<Self>) {
        let window = self.main_window;
        let (sender, mut events) = mpsc::channel(1);
        self.omarchy_watch_task = Some(cx.spawn(async move |this, cx| {
            let watcher = cx
                .background_spawn(async move {
                    let root = gamesync_desktop::omarchy::current_dir()?;
                    crate::watcher::watch(&root, sender)
                })
                .await;
            let mut watcher = watcher.ok();

            loop {
                if watcher.is_some() {
                    let timer = cx.background_executor().timer(Duration::from_secs(30));
                    futures::pin_mut!(timer);
                    if let futures::future::Either::Left((event, _)) =
                        futures::future::select(events.next(), timer).await
                    {
                        if event.is_none() {
                            watcher = None;
                        } else {
                            cx.background_executor()
                                .timer(Duration::from_millis(250))
                                .await;
                            while events.try_recv().is_ok() {}
                        }
                    }
                } else {
                    cx.background_executor()
                        .timer(Duration::from_secs(5))
                        .await;
                }

                let loaded = cx
                    .background_spawn(async {
                        gamesync_desktop::omarchy::OmarchyTheme::load_active()
                    })
                    .await;
                match loaded {
                    Ok(theme) => {
                        let applied = theme.clone();
                        let changed = this
                            .update(cx, |this, cx| {
                                if !this.omarchy_mode {
                                    return false;
                                }
                                let changed = this.omarchy_theme.as_ref() != Some(&theme);
                                this.omarchy_theme = Some(theme.clone());
                                if let Some(view) = &this.settings_view {
                                    view.update(cx, |view, cx| {
                                        view.set_omarchy_theme(&theme, cx)
                                    });
                                }
                                changed
                            })
                            .unwrap_or(false);
                        if changed {
                            let _ = window.update(cx, move |_, window, cx| {
                                crate::theme::apply_omarchy(&applied, window, cx)
                            });
                        }
                    }
                    Err(error) => {
                        let _ = this.update(cx, |this, cx| {
                            if this.omarchy_mode && this.omarchy_theme.is_none() {
                                this.notice = format!(
                                    "Omarchy mode is on, but its current theme is unavailable: {error:#}"
                                );
                                cx.notify();
                            }
                        });
                    }
                }
            }
        }));
    }
}
