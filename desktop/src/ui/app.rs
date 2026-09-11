mod loading;
// Eagle's three-pane shell with a shared game model. Appearance stays in memory.

use crate::{
    model::Library,
    theme,
    ui::{
        detail::DetailPanel,
        grid::GameGrid,
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
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
    StyledExt as _,
};
use std::path::PathBuf;

pub struct GameSyncApp {
    library: Entity<Library>,
    grid: Entity<GameGrid>,
    sidebar: Entity<LibrarySidebar>,
    detail: Entity<DetailPanel>,
    search: Entity<InputState>,
    sidebar_shown: bool,
    detail_shown: bool,
    compact: bool,
    theme: String,
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
    refreshed: bool,
    notice: String,
    last_issues: Vec<String>,
}

impl GameSyncApp {
    pub fn new(
        library: Library,
        initial_path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
        let editor_subscription = cx.subscribe(&detail, |this, _, event, _| {
            if matches!(event, super::editor::EditorEvent::Saved) {
                if let Some(sender) = &mut this.refresh {
                    let _ = sender.try_send(Ok(()));
                }
            }
        });
        let library_subscription = cx.observe(&library, |_, _, cx| cx.notify());
        window.focus(&grid.focus_handle(cx));
        let mut app = Self {
            library,
            grid,
            sidebar,
            detail,
            search,
            sidebar_shown: true,
            detail_shown: true,
            compact: false,
            theme: theme::SYSTEM_THEME.into(),
            _subscriptions: vec![
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
            refreshed: false,
            notice: String::new(),
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
            Err(_) => self.notice = "Refresh is unavailable. Open the library again.".into(),
        }
        self.refreshed = false;
        cx.notify();
    }

    pub fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.detail.read(cx).busy(cx) {
            self.detail_shown = true;
            self.notice =
                "Save or discard your draft before closing or opening another library.".into();
            cx.notify();
            false
        } else {
            true
        }
    }

    pub fn appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme == theme::SYSTEM_THEME {
            theme::apply_choice(&self.theme, window, cx);
        }
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let selected_theme = self.theme.clone();
        h_flex()
            .h(px(48.))
            .px_3()
            .gap_2()
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                Button::new("sidebar-toggle")
                    .ghost()
                    .small()
                    .icon(IconName::PanelLeft)
                    .tooltip("Toggle sidebar")
                    .selected(self.sidebar_shown)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_shown = !this.sidebar_shown;
                        cx.notify();
                    })),
            )
            .child(
                div().font_medium().text_sm().child(
                    self.library
                        .read(cx)
                        .scope_label(&self.library.read(cx).scope),
                ),
            )
            .child(div().flex_1())
            .child(
                Button::new("open-library")
                    .ghost()
                    .small()
                    .icon(IconName::Folder)
                    .tooltip("Open library (⌘/Ctrl O)")
                    .on_click(cx.listener(|this, _, _, cx| this.choose_folder(cx))),
            )
            .child(
                Button::new("refresh-library")
                    .ghost()
                    .small()
                    .label("Refresh")
                    .tooltip("Refresh library (⌘/Ctrl R)")
                    .disabled(self.loading || self.refreshing || self.refresh.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.refresh_library(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(80.))
                    .max_w(px(220.))
                    .child(Input::new(&self.search).small()),
            )
            .child(
                Button::new("density-toggle")
                    .ghost()
                    .small()
                    .icon(IconName::LayoutDashboard)
                    .tooltip("Toggle compact covers")
                    .selected(self.compact)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.compact = !this.compact;
                        this.grid
                            .update(cx, |grid, cx| grid.set_density(this.compact, cx));
                    })),
            )
            .child(
                Button::new("theme-menu")
                    .ghost()
                    .small()
                    .icon(IconName::Sun)
                    .tooltip("Appearance")
                    .dropdown_menu(move |mut menu, _, _| {
                        let choices = ["system".to_owned(), "light".to_owned(), "dark".to_owned()]
                            .into_iter()
                            .chain(theme::bundled_names());
                        for choice in choices {
                            let target = entity.clone();
                            menu = menu.item(
                                PopupMenuItem::new(choice.clone())
                                    .checked(choice == selected_theme)
                                    .on_click(move |_, window, cx| {
                                        target.update(cx, |this, cx| {
                                            this.theme = choice.clone();
                                            theme::apply_choice(&choice, window, cx);
                                            cx.notify();
                                        })
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .child(
                Button::new("detail-toggle")
                    .ghost()
                    .small()
                    .icon(IconName::PanelRight)
                    .tooltip("Toggle details")
                    .selected(self.detail_shown)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.detail_shown = !this.detail_shown;
                        cx.notify();
                    })),
            )
    }
}

impl Render for GameSyncApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.clear_search {
            self.clear_search = false;
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
        }
        let title = format!("GameSync — {}", self.library.read(cx).name);
        if self.window_title != title {
            window.set_window_title(&title);
            self.window_title = title;
        }
        v_flex()
            .on_action(cx.listener(|this, _: &crate::FocusSearch, window, cx| {
                window.focus(&this.search.focus_handle(cx));
            }))
            .on_action(cx.listener(|this, _: &crate::OpenLibrary, _, cx| this.choose_folder(cx)))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.sidebar_shown, |row| row.child(self.sidebar.clone()))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.toolbar(cx))
                            .when(!self.notice.is_empty(), |column| {
                                column.child(
                                    div()
                                        .px_3()
                                        .py_2()
                                        .text_xs()
                                        .bg(cx.theme().sidebar)
                                        .child(self.notice.clone()),
                                )
                            })
                            .child(div().flex_1().min_h_0().p_2().child(self.grid.clone())),
                    )
                    .when(self.detail_shown, |row| row.child(self.detail.clone())),
            )
            .child(
                h_flex()
                    .h(px(28.))
                    .px_3()
                    .justify_between()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} of {} games",
                        self.library.read(cx).visible.len(),
                        self.library.read(cx).games.len()
                    ))
                    .child(if self.loading {
                        "Opening library…"
                    } else if self.refreshing {
                        "Refreshing library…"
                    } else if self.refreshed {
                        "Library refreshed · ⌘/Ctrl R to refresh"
                    } else if self.library.read(cx).demo {
                        "Demo library · Browse with arrow keys"
                    } else {
                        "Library · ⌘/Ctrl R to refresh"
                    }),
            )
    }
}
