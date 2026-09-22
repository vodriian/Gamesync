//! One window per auxiliary surface. Opening again brings the existing window forward.
use super::GameSyncApp;
use gpui::{prelude::*, px, size, Bounds, WindowBounds, WindowOptions};
use gpui_component::Root;
impl GameSyncApp {
    pub(super) fn open_collections(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.review_folder.clone().or_else(|| {
            self.library
                .read(cx)
                .source
                .as_ref()
                .map(|(root, _)| root.clone())
        }) else {
            self.notice = "Your games are still loading. Try collections again in a moment.".into();
            cx.notify();
            return;
        };
        if self.collection_root.as_ref() != Some(&root) {
            if let Some(handle) = self.collections_window.take() {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
            self.collections_view = None;
        }
        if let Some(handle) = self.collections_window {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
        }
        self.collection_root = Some(root.clone());
        let created = std::rc::Rc::new(std::cell::RefCell::new(None));
        let result_view = created.clone();
        let bounds = Bounds::centered(None, size(px(540.), px(600.)), cx);
        match cx.open_window(
            WindowOptions {
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Collections".into()),
                    ..gpui_component::TitleBar::title_bar_options()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| crate::ui::collections::Collections::new(root, window, cx));
                *result_view.borrow_mut() = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(window) => {
                self.collections_window = Some(window.into());
                self.collections_view = created.borrow_mut().take();
            }
            Err(error) => self.notice = error.to_string(),
        }
    }

    pub(super) fn set_theme(
        &mut self,
        choice: gamesync_desktop::appearance::Appearance,
        cx: &mut Context<Self>,
    ) {
        self.theme = choice.clone();
        if let Some(view) = &self.settings_view {
            view.update(cx, |view, cx| view.set_theme(choice.clone(), cx));
        }
        // Rapid preview clicks must not let an older background save win.
        let revision_state = self.appearance_revision.clone();
        let revision = revision_state.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::settings::update(|s| {
                        if revision_state.load(std::sync::atomic::Ordering::SeqCst) == revision {
                            s.appearance = Some(choice);
                        }
                    })
                })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.notice = format!("Could not save appearance: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }
    pub fn open_steam_settings(&mut self, cx: &mut Context<Self>) {
        self.open_settings(cx);
        if let Some(view) = &self.settings_view {
            view.update(cx, |view, cx| view.show_general(cx));
        }
    }
    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
        }
        let library = self.library.clone();
        let theme = self.theme.clone();
        let existing = self.settings_view.clone();
        let bounds = Bounds::centered(None, size(px(820.), px(780.)), cx);
        let created = std::rc::Rc::new(std::cell::RefCell::new(None));
        let result_view = created.clone();
        match cx.open_window(
            WindowOptions {
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Settings".into()),
                    ..gpui_component::TitleBar::title_bar_options()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(700.), px(480.))),
                ..Default::default()
            },
            move |window, cx| {
                let view = existing.unwrap_or_else(|| {
                    cx.new(|cx| crate::ui::settings::SettingsView::new(library, theme, window, cx))
                });
                let root = cx.new(|cx| Root::new(view.clone(), window, cx));
                *result_view.borrow_mut() = Some(view);
                root
            },
        ) {
            Ok(window) => {
                self.settings_window = Some(window.into());
                if self.settings_view.is_none() {
                    if let Some(view) = created.borrow_mut().take() {
                        self._subscriptions
                            .push(cx.subscribe(&view, |this, _, event, cx| match event {
                                crate::ui::settings::SettingsEvent::Theme(choice) => {
                                    this.set_theme(choice.clone(), cx)
                                }
                                crate::ui::settings::SettingsEvent::LastSync(time) => {
                                    this.last_sync = *time;
                                    cx.notify();
                                }
                                crate::ui::settings::SettingsEvent::Refresh => {
                                    this.refresh_library(cx)
                                }
                            }));
                        self.settings_view = Some(view);
                    }
                }
            }
            Err(error) => self.notice = error.to_string(),
        }
    }
}
