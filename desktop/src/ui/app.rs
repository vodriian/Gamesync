//! Eagle's three-pane shell with a shared game model. Demo preferences stay in memory.

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
use gpui::{div, prelude::*, px, Entity, Focusable, Subscription, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, IconName, Selectable as _, Sizable as _, StyledExt as _,
};

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
}

impl GameSyncApp {
    pub fn new(library: Library, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = cx.new(|_| library);
        let cache = LruImageCache::new(DEFAULT_BUDGET_BYTES, cx);
        let grid = cx.new(|cx| GameGrid::new(library.clone(), cache.clone(), cx));
        let sidebar = cx.new(|cx| LibrarySidebar::new(library.clone(), cx));
        let detail = cx.new(|cx| DetailPanel::new(library.clone(), cache, cx));
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
        let library_subscription = cx.observe(&library, |_, _, cx| cx.notify());
        window.focus(&grid.focus_handle(cx));
        Self {
            library,
            grid,
            sidebar,
            detail,
            search,
            sidebar_shown: true,
            detail_shown: true,
            compact: false,
            theme: theme::SYSTEM_THEME.into(),
            _subscriptions: vec![search_subscription, library_subscription],
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
                div()
                    .font_medium()
                    .text_sm()
                    .child(self.library.read(cx).scope.label()),
            )
            .child(div().flex_1())
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .on_action(cx.listener(|this, _: &crate::FocusSearch, window, cx| {
                window.focus(&this.search.focus_handle(cx));
            }))
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
                    .child("Demo library · Browse with arrow keys"),
            )
    }
}
