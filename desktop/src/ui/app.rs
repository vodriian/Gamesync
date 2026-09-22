mod display;
mod loading;
mod windows;
// A shared library model with three presentations and a focused game card.

use crate::{
    model::Library,
    theme,
    ui::{
        detail::DetailPanel,
        grid::{GameGrid, LibraryView, OpenGame},
        sidebar::LibrarySidebar,
        thumb_cache::{LruImageCache, DEFAULT_BUDGET_BYTES},
    },
};
use futures::channel::mpsc;
use gpui::Task;
use gpui::{div, prelude::*, px, Entity, Focusable, Subscription, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, StyledExt as _,
};
use std::path::PathBuf;

pub struct GameSyncApp {
    library: Entity<Library>,
    collections_view: Option<Entity<super::collections::Collections>>,
    collection_root: Option<PathBuf>,
    review_folder: Option<PathBuf>,
    collections_window: Option<gpui::AnyWindowHandle>,
    settings_window: Option<gpui::AnyWindowHandle>,
    settings_view: Option<Entity<super::settings::SettingsView>>,
    grid: Entity<GameGrid>,
    sidebar: Entity<LibrarySidebar>,
    sidebar_shown: bool,
    sidebar_motion: super::motion::Motion,
    detail: Entity<DetailPanel>,
    search: Entity<InputState>,
    detail_shown: bool,
    view: LibraryView,
    restore_grid_focus: bool,
    last_sync: Option<u64>,
    theme: gamesync_desktop::appearance::Appearance,
    appearance_revision: std::sync::Arc<std::sync::atomic::AtomicU64>,
    _subscriptions: Vec<Subscription>,
    cache: Entity<LruImageCache>,
    load_task: Option<Task<()>>,
    watch_task: Option<Task<()>>,
    clear_search: bool,
    window_title: String,
    refresh: Option<mpsc::Sender<Result<(), String>>>,
    folder: Option<PathBuf>,
    loading: bool,
    refreshing: bool,
    notice: String,
    display_save: Option<Task<()>>,
    display_pending: usize,
    last_issues: Vec<String>,
}

impl GameSyncApp {
    pub fn new(
        mut library: Library,
        initial_path: Option<PathBuf>,
        initial_theme: gamesync_desktop::appearance::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if let Ok(settings) = crate::settings::load() {
            library.display = settings.library_display;
            library.recompute();
        }
        library.show_hidden_games = crate::settings::load().is_ok_and(|s| s.show_hidden_games);
        cx.set_global(super::motion::MotionPreferences {
            reduced: crate::settings::load().is_ok_and(|s| s.reduce_motion),
        });
        let library = cx.new(|_| library);
        let cache = LruImageCache::new(DEFAULT_BUDGET_BYTES, cx);
        let grid = cx.new(|cx| GameGrid::new(library.clone(), cache.clone(), cx));
        let sidebar = cx.new(|cx| LibrarySidebar::new(library.clone(), cx));
        let detail = cx.new(|cx| DetailPanel::new(library.clone(), cache.clone(), cx));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search games or tags…"));
        let search_subscription = cx.subscribe(&search, |this, search, event, cx| {
            if matches!(event, InputEvent::Change) {
                let value = search.read(cx).value();
                this.library.update(cx, |lib, cx| {
                    lib.set_query(&value);
                    cx.notify();
                });
            }
        });
        let editor_subscription = cx.subscribe(&detail, |this, _, event, cx| {
            if matches!(event, super::editor::EditorEvent::Closed) {
                this.detail_shown = false;
                this.grid
                    .update(cx, |grid, _| grid.preserve_viewport(false));
                this.restore_grid_focus = true;
                cx.notify();
            }
            if matches!(event, super::editor::EditorEvent::Saved) {
                if let Some(sender) = &mut this.refresh {
                    let _ = sender.try_send(Ok(()));
                }
            }
        });
        let grid_subscription = cx.subscribe(&grid, |this, _, _: &OpenGame, cx| {
            this.detail_shown = true;
            this.grid.update(cx, |grid, _| grid.preserve_viewport(true));
            this.detail.update(cx, |detail, cx| detail.present(cx));
            cx.notify();
        });
        let bulk_subscription =
            cx.subscribe(&grid, |this, _, event: &super::grid::BulkSaved, cx| {
                this.sidebar
                    .update(cx, |sidebar, cx| sidebar.show_toast(&event.0, cx));
            });
        let library_subscription = cx.observe(&library, |_, _, cx| cx.notify());
        window.focus(&grid.focus_handle(cx));
        let mut app = Self {
            library,
            collections_window: None,
            collections_view: None,
            collection_root: None,
            review_folder: None,
            settings_window: None,
            settings_view: None,
            grid,
            sidebar,
            sidebar_shown: true,
            sidebar_motion: super::motion::Motion::new(1.),
            detail,
            search,
            detail_shown: false,
            last_sync: None,
            view: LibraryView::Cards,
            restore_grid_focus: false,
            theme: initial_theme,
            appearance_revision: Default::default(),
            _subscriptions: vec![
                grid_subscription,
                bulk_subscription,
                search_subscription,
                library_subscription,
                editor_subscription,
            ],
            cache,
            load_task: None,
            watch_task: None,
            clear_search: false,
            window_title: String::new(),
            refresh: None,
            folder: None,
            loading: false,
            refreshing: false,
            notice: String::new(),
            display_save: None,
            display_pending: 0,
            last_issues: Vec::new(),
        };
        if let Some(path) = initial_path {
            app.open_folder(path, cx);
        }
        app
    }

    pub fn refresh_library(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.refreshing {
            return;
        }
        let Some(sender) = &mut self.refresh else {
            return;
        };
        match sender.try_send(Ok(())) {
            Ok(()) => self.refreshing = true,
            // A pending scan also satisfies this request. Do not grow the queue.
            Err(error) if error.is_full() => self.refreshing = true,
            Err(_) => self.notice = "Refresh is unavailable. Restart GameSync to retry.".into(),
        }
        cx.notify();
    }

    pub fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.display_pending > 0 {
            self.notice = "Wait for display settings to save.".into();
            cx.notify();
            return false;
        }
        if self.grid.read(cx).busy() {
            self.notice =
                "Wait for game changes to save, or close the note, before closing.".into();
            cx.notify();
            return false;
        }
        if self.sidebar.read(cx).busy()
            || self
                .collections_view
                .as_ref()
                .is_some_and(|v| v.read(cx).busy())
        {
            self.notice = "Wait for the collection save to finish.".into();
            cx.notify();
            return false;
        }
        if self
            .settings_view
            .as_ref()
            .is_some_and(|view| view.read(cx).busy())
        {
            self.notice =
                "Wait for Steam or cancel sync in Settings before closing or switching libraries."
                    .into();
            cx.notify();
            return false;
        }
        if self.detail.read(cx).busy(cx) {
            self.detail_shown = true;
            self.grid.update(cx, |grid, _| grid.preserve_viewport(true));
            self.detail.update(cx, |detail, cx| detail.present(cx));
            self.notice =
                "Changes are still saving. If a save failed, resolve it in details before closing."
                    .into();
            cx.notify();
            false
        } else {
            true
        }
    }

    pub fn appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme.mode == gamesync_desktop::appearance::AppearanceMode::Auto {
            theme::apply_choice(&self.theme, window, cx);
        }
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h(px(68.))
            .px_5()
            .gap_3()
            .child(
                Button::new("sidebar-toggle")
                    .ghost()
                    .small()
                    .icon(IconName::PanelLeft)
                    .tooltip("Toggle sidebar")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_shown = !this.sidebar_shown;
                        this.sidebar_motion
                            .set(if this.sidebar_shown { 1. } else { 0. }, cx);
                        cx.notify();
                    })),
            )
            .child(
                div().flex_1().min_w_0().truncate().font_medium().child(
                    self.library
                        .read(cx)
                        .scope_label(&self.library.read(cx).scope),
                ),
            )
            .child(
                h_flex()
                    .h(px(38.))
                    .p_1()
                    .gap_1()
                    .rounded_full()
                    .bg(cx.theme().secondary.opacity(0.70))
                    .children(
                        [
                            (LibraryView::Cards, "Cards", IconName::GalleryVerticalEnd),
                            (LibraryView::Grid, "Grid", IconName::LayoutDashboard),
                            (LibraryView::Table, "Table", IconName::Menu),
                        ]
                        .into_iter()
                        .map(|(view, label, icon)| {
                            Button::new(label)
                                .ghost()
                                .small()
                                .icon(icon)
                                .tooltip(label)
                                .rounded_full()
                                .w(px(36.))
                                .h(px(30.))
                                .selected(self.view == view)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.view = view;
                                    this.grid.update(cx, |grid, cx| grid.set_view(view, cx));
                                    window.focus(&this.grid.focus_handle(cx));
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(self.display_control(cx))
            .child(
                h_flex()
                    .w(px(220.))
                    .h(px(38.))
                    .px_2()
                    .rounded_full()
                    .border_1()
                    .border_color(cx.theme().border.opacity(0.5))
                    .bg(cx.theme().background.opacity(0.75))
                    .child(
                        Input::new(&self.search)
                            .small()
                            .appearance(false)
                            .prefix(Icon::new(IconName::Search).size_4()),
                    ),
            )
    }
}

impl Render for GameSyncApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.restore_grid_focus {
            self.restore_grid_focus = false;
            window.focus(&self.grid.focus_handle(cx));
        }
        if self.clear_search {
            self.clear_search = false;
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
        }
        let title = self.library.read(cx).name.clone();
        if self.window_title != title {
            window.set_window_title(&title);
            self.window_title = title;
        }
        if self.detail_shown {
            return v_flex()
                .size_full()
                .bg(cx.theme().sidebar)
                .child(gpui_component::TitleBar::new().border_b_0())
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .p(px(8.))
                        .child(super::panel::content(self.detail.clone(), cx)),
                )
                .into_any_element();
        }
        let sidebar_progress = if super::motion::reduced(cx) {
            if self.sidebar_shown {
                1.
            } else {
                0.
            }
        } else {
            self.sidebar_motion.value()
        };
        let sidebar_width = px(255. * sidebar_progress);
        let library_surface = div().size_full().child(self.grid.clone());
        let sync_status = if self.loading {
            "Opening library…".to_owned()
        } else {
            crate::settings::sync_label(self.last_sync)
        };
        let content = div()
            .relative()
            .on_action(cx.listener(|this, _: &crate::FocusSearch, window, cx| {
                window.focus(&this.search.focus_handle(cx));
            }))
            .on_action(
                cx.listener(|this, _: &crate::ManageCollections, window, cx| {
                    if this.review_folder.is_some() || this.library.read(cx).write_issue.is_some() {
                        this.open_collections(cx);
                    } else {
                        this.sidebar_shown = true;
                        this.sidebar_motion.set(1., cx);
                        this.sidebar
                            .update(cx, |sidebar, cx| sidebar.begin_name(None, window, cx));
                    }
                }),
            )
            .size_full()
            .bg(super::card::tabletop(cx))
            .text_color(cx.theme().foreground)
            .child(library_surface)
            .child(
                v_flex()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .occlude()
                    .bg(super::card::tabletop(cx))
                    .border_b_1()
                    .border_color(cx.theme().border.opacity(0.25))
                    .child(self.toolbar(cx))
                    .when(!self.notice.is_empty(), |header| {
                        header.child(
                            div()
                                .px_5()
                                .py_2()
                                .text_xs()
                                .bg(cx.theme().muted)
                                .child(self.notice.clone()),
                        )
                    })
                    .when(!self.library.read(cx).conflicts.is_empty(), |header| {
                        header.child(
                            Button::new("review-conflicts")
                                .small()
                                .label("Review conflicts")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.detail_shown = true;
                                    this.grid.update(cx, |grid, _| grid.preserve_viewport(true));
                                    this.detail
                                        .update(cx, |detail, cx| detail.review_conflicts(cx));
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .occlude()
                    .rounded_tl(px(12.))
                    .bg(super::card::tabletop(cx))
                    .child(
                        Button::new("library-status")
                            .ghost()
                            .small()
                            .label(format!("{} games · ⓘ", self.library.read(cx).visible.len()))
                            .tooltip(format!(
                                "Arrow keys to browse · Enter to open\n{}",
                                sync_status
                            )),
                    ),
            );
        h_flex()
            .relative()
            .size_full()
            .bg(cx.theme().sidebar)
            .on_action(
                cx.listener(|this, _: &crate::ManageCollections, window, cx| {
                    if this.review_folder.is_some() || this.library.read(cx).write_issue.is_some() {
                        this.open_collections(cx);
                    } else {
                        this.sidebar_shown = true;
                        this.sidebar_motion.set(1., cx);
                        this.sidebar
                            .update(cx, |sidebar, cx| sidebar.begin_name(None, window, cx));
                    }
                }),
            )
            .when(sidebar_progress > 0., |shell| {
                shell.child(
                    div()
                        .w(sidebar_width)
                        .h_full()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(
                            v_flex()
                                .w(px(255.))
                                .h_full()
                                .ml(sidebar_width - px(255.))
                                .child(div().h(px(34.)).flex_shrink_0())
                                .child(div().flex_1().min_h_0().child(self.sidebar.clone())),
                        ),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    // Reserve traffic-light space continuously while the sidebar slides away.
                    .child(
                        div()
                            .h(px(if cfg!(target_os = "macos") {
                                34. * (1. - f32::from(sidebar_width) / 100.).clamp(0., 1.)
                            } else {
                                34.
                            }))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .p(px(8.))
                            .child(super::panel::content(content, cx)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(if cfg!(target_os = "macos") && sidebar_progress > 0. {
                        sidebar_width.max(px(80.))
                    } else {
                        window.viewport_size().width
                    })
                    .child(gpui_component::TitleBar::new().border_b_0()),
            )
            .into_any_element()
    }
}
