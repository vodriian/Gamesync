//! Sync results in the open app: status, and shared settings from other devices.
use super::{loading::SyncOutcome, GameSyncApp};
use crate::settings::LibraryView;
use gpui::Context;

impl GameSyncApp {
    /// Store a round result and show shared settings that it changed.
    pub(super) fn apply_sync(&mut self, outcome: SyncOutcome, cx: &mut Context<Self>) {
        let mut settings_changed = false;
        let mut keys_changed = false;
        self.sync.update(cx, |sync, cx| {
            // The user stopped sync while this round ran.
            if !sync.enabled() {
                return;
            }
            if sync.handle.is_none() {
                sync.handle = outcome.opened;
            }
            match outcome.report {
                Ok(report) => {
                    settings_changed = report.settings_changed;
                    keys_changed = report.keys_changed;
                    sync.apply(report);
                }
                Err(error) => sync.issues = vec![error],
            }
            cx.notify();
        });
        if keys_changed {
            if let Some(view) = &self.settings_view {
                view.update(cx, |view, cx| view.reload_connection(cx));
            }
        }
        if settings_changed {
            self.reload_shared_settings(cx);
        }
    }

    /// Read shared settings that sync wrote and show them. Device-local
    /// settings, such as the theme, are not synced, so they stay as they are.
    fn reload_shared_settings(&mut self, cx: &mut Context<Self>) {
        let settings = match crate::settings::load() {
            Ok(settings) => settings,
            Err(error) => {
                self.notice = format!("Could not read synced settings: {error:#}");
                return;
            }
        };
        self.section_views = settings.section_views.clone();
        let (section, in_wishlist) = {
            let lib = self.library.read(cx);
            (
                (!lib.home).then(|| lib.scope.view_key()),
                !lib.home && lib.scope == crate::model::Scope::Wishlist,
            )
        };
        if let Some(section) = section {
            let mut view = self
                .section_views
                .get(&section)
                .copied()
                .unwrap_or_default();
            if in_wishlist && view == LibraryView::Board {
                view = LibraryView::Grid;
            }
            self.apply_view(view, cx);
        }
        self.library.update(cx, |lib, cx| {
            lib.display = settings.library_display.clone();
            lib.set_show_hidden_games(settings.show_hidden_games);
            lib.recompute();
            cx.notify();
        });
        cx.global_mut::<crate::ui::motion::MotionPreferences>()
            .reduced = settings.reduce_motion;
        if let Some(view) = &self.settings_view {
            let reduce = settings.reduce_motion;
            view.update(cx, |view, cx| view.set_reduce_motion(reduce, cx));
        }
        cx.notify();
    }
}
