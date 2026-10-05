mod display;
mod loading;
mod omarchy;
mod prices;
mod sync;
mod windows;
// A shared library model with three presentations and a focused game card.

use crate::{
    model::Library,
    theme,
    ui::{
        detail::DetailPanel,
        grid::{GameGrid, LibraryView, OpenGame},
        home::{HomeView, OpenFromHome},
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
use std::{collections::BTreeMap, path::PathBuf};

pub struct GameSyncApp {
    library: Entity<Library>,
    collections_view: Option<Entity<super::collections::Collections>>,
    collection_root: Option<PathBuf>,
    review_folder: Option<PathBuf>,
    collections_window: Option<gpui::AnyWindowHandle>,
    settings_window: Option<gpui::AnyWindowHandle>,
    settings_view: Option<Entity<super::settings::SettingsView>>,
    grid: Entity<GameGrid>,
    home: Entity<HomeView>,
    sidebar: Entity<LibrarySidebar>,
    sidebar_shown: bool,
    sidebar_motion: super::motion::Motion,
    detail: Entity<DetailPanel>,
    search: Entity<InputState>,
    detail_shown: bool,
    view: LibraryView,
    section_views: BTreeMap<String, LibraryView>,
    active_section: Option<String>,
    restore_grid_focus: bool,
    last_sync: Option<u64>,
    theme: gamesync_desktop::appearance::Appearance,
    omarchy_mode: bool,
    omarchy_theme: Option<gamesync_desktop::omarchy::OmarchyTheme>,
    omarchy_watch_task: Option<Task<()>>,
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
    view_save: Option<Task<()>>,
    price_task: Option<Task<()>>,
    /// Detects opening the Wishlist scope, which triggers a price refresh.
    in_wishlist: bool,
    display_pending: usize,
    last_issues: Vec<String>,
    main_window: gpui::AnyWindowHandle,
    sync: Entity<crate::sync_runtime::SyncState>,
}

impl GameSyncApp {
    pub fn new(
        mut library: Library,
        initial_path: Option<PathBuf>,
        initial_theme: gamesync_desktop::appearance::Appearance,
        omarchy_mode: bool,
        omarchy_theme: Option<gamesync_desktop::omarchy::OmarchyTheme>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = crate::settings::load().unwrap_or_default();
        library.display = settings.library_display.clone();
        library.recompute();
        library.show_hidden_games = settings.show_hidden_games;
        cx.set_global(super::motion::MotionPreferences {
            reduced: settings.reduce_motion,
        });
        let library = cx.new(|_| library);
        let sync = cx.new(|_| crate::sync_runtime::SyncState {
            folder: settings.sync_folder.clone(),
            ..Default::default()
        });
        let cache = LruImageCache::new(DEFAULT_BUDGET_BYTES, cx);
        let grid = cx.new(|cx| GameGrid::new(library.clone(), cache.clone(), cx));
        let home = cx.new(|cx| HomeView::new(library.clone(), cache.clone(), cx));
        let sidebar = cx.new(|cx| LibrarySidebar::new(library.clone(), sync.clone(), cx));
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
            if let super::editor::EditorEvent::ShowScope(scope) = event {
                this.library.update(cx, |lib, cx| {
                    lib.set_scope(scope.clone());
                    cx.notify();
                });
                this.detail_shown = false;
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
        let home_subscription = cx.subscribe(&home, |this, _, event: &OpenFromHome, cx| {
            this.library
                .update(cx, |lib, _| lib.select_from_home(event.0));
            this.clear_search = true;
            this.detail_shown = true;
            this.detail.update(cx, |detail, cx| detail.present(cx));
            cx.notify();
        });
        let bulk_subscription =
            cx.subscribe(&grid, |this, _, event: &super::grid::BulkSaved, cx| {
                this.sidebar
                    .update(cx, |sidebar, cx| sidebar.show_toast(&event.0, cx));
            });
        let library_subscription = cx.observe(&library, |this, library, cx| {
            let (section, in_wishlist) = {
                let lib = library.read(cx);
                (
                    (!lib.home).then(|| lib.scope.view_key()),
                    !lib.home && lib.scope == crate::model::Scope::Wishlist,
                )
            };
            if section != this.active_section {
                this.active_section = section.clone();
                if let Some(section) = section {
                    let mut preferred = this
                        .section_views
                        .get(&section)
                        .copied()
                        .unwrap_or_default();
                    // Wishlist games have no status, so the Board does not apply.
                    if in_wishlist && preferred == LibraryView::Board {
                        preferred = LibraryView::Grid;
                    }
                    this.apply_view(preferred, cx);
                }
            }
            if in_wishlist && !this.in_wishlist {
                this.refresh_prices(cx);
            }
            this.in_wishlist = in_wishlist;
            cx.notify();
        });
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
            home,
            sidebar,
            sidebar_shown: true,
            sidebar_motion: super::motion::Motion::new(1.),
            detail,
            search,
            detail_shown: false,
            last_sync: None,
            view: LibraryView::Grid,
            section_views: settings.section_views,
            active_section: None,
            restore_grid_focus: false,
            theme: initial_theme,
            omarchy_mode,
            omarchy_theme,
            omarchy_watch_task: None,
            appearance_revision: Default::default(),
            _subscriptions: vec![
                grid_subscription,
                home_subscription,
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
            view_save: None,
            price_task: None,
            in_wishlist: false,
            display_pending: 0,
            last_issues: Vec::new(),
            main_window: window.window_handle(),
            sync,
        };
        if app.omarchy_mode {
            app.start_omarchy_sync(cx);
        }
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
                "Wait for game changes to save, or close the open editor, before closing.".into();
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
                "Wait for Settings to finish, or cancel the Steam sync, before closing.".into();
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
        if !self.omarchy_mode
            && self.theme.mode == gamesync_desktop::appearance::AppearanceMode::Auto
        {
            theme::apply_choice(&self.theme, window, cx);
        }
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let home = self.library.read(cx).home;
        let mut title = if home {
            "Home".into()
        } else {
            self.library
                .read(cx)
                .scope_label(&self.library.read(cx).scope)
        };
        if self.library.read(cx).best_on_demo {
            title.push_str(" · Demo");
        }
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
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_medium()
                    .child(title),
            )
            // Home is not a list of games, so views, filters, and search do not apply.
            .when(!home, |toolbar| toolbar.child(self.library_controls(cx)))
    }

    fn apply_view(&mut self, view: LibraryView, cx: &mut Context<Self>) {
        if self.view == view {
            return;
        }
        self.view = view;
        self.grid.update(cx, |grid, cx| grid.set_view(view, cx));
    }

    fn choose_view(&mut self, view: LibraryView, cx: &mut Context<Self>) {
        let section = self.library.read(cx).scope.view_key();
        self.section_views.insert(section.clone(), view);
        self.apply_view(view, cx);

        // Serialize rapid choices so the newest view is the final disk write.
        let previous = self.view_save.take();
        self.display_pending += 1;
        self.view_save = Some(cx.spawn(async move |app, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = cx
                .background_spawn(async move {
                    crate::settings::update(move |settings| {
                        settings.section_views.insert(section, view);
                    })
                })
                .await;
            let _ = app.update(cx, |app, cx| {
                app.display_pending = app.display_pending.saturating_sub(1);
                if let Err(error) = result {
                    app.notice = format!("Could not save section view: {error}");
                }
                cx.notify();
            });
        }));
    }

    fn library_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let wishlist = self.library.read(cx).scope == crate::model::Scope::Wishlist;
        h_flex()
            .gap_3()
            .child(
                h_flex()
                    .h(px(38.))
                    .p_1()
                    .gap_1()
                    .rounded(crate::theme::pill_radius(cx))
                    .bg(cx.theme().secondary.opacity(0.70))
                    .children(
                        [
                            (
                                LibraryView::Grid,
                                "Grid",
                                Icon::new(IconName::LayoutDashboard),
                            ),
                            (
                                LibraryView::Cards,
                                "Cards",
                                Icon::new(IconName::GalleryVerticalEnd),
                            ),
                            (LibraryView::Table, "Table", Icon::new(IconName::Menu)),
                            (
                                LibraryView::Board,
                                "Board",
                                Icon::new(crate::assets::BoardIcon),
                            ),
                        ]
                        .into_iter()
                        .filter(|(view, _, _)| !(wishlist && *view == LibraryView::Board))
                        .map(|(view, label, icon)| {
                            Button::new(label)
                                .ghost()
                                .small()
                                .icon(icon)
                                .tooltip(label)
                                .rounded(crate::theme::pill_radius(cx))
                                .w(px(36.))
                                .h(px(30.))
                                .selected(self.view == view)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.choose_view(view, cx);
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
                    .rounded(crate::theme::pill_radius(cx))
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
        let home = self.library.read(cx).home;
        let library_surface = div().size_full().map(|surface| {
            if home {
                surface.child(self.home.clone())
            } else {
                surface.child(self.grid.clone())
            }
        });
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
            .when(!home, |content| {
                content.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .right_0()
                        .occlude()
                        .rounded_tl(crate::theme::interface_radius(cx, px(12.)))
                        .bg(super::card::tabletop(cx))
                        .child(
                            Button::new("library-status")
                                .ghost()
                                .small()
                                .label(format!("{} games", self.library.read(cx).visible.len()))
                                .child(Icon::new(IconName::Info).size_4())
                                .tooltip(format!(
                                    "Arrow keys to browse · Enter to open\n{}",
                                    sync_status
                                )),
                        ),
                )
            });
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
