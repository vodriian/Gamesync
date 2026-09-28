//! Home page. Panels come from `dashboard::Dashboard`; this view only renders them.
//! Games open their card through `OpenFromHome`; genres and collections open scopes.
use super::thumb_cache::LruImageCache;
use crate::{
    dashboard::{CollectionTile, Dashboard},
    model::{Library, Scope},
};
use gamesync_desktop::smart::SmartRule;
use gpui::{div, image_cache, img, prelude::*, px, relative, Entity, ObjectFit, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, StyledExt as _,
};
use uuid::Uuid;

pub struct OpenFromHome(pub Uuid);

pub struct HomeView {
    library: Entity<Library>,
    cache: Entity<LruImageCache>,
    dashboard: Dashboard,
    /// Rebuild on the next render. Changes while Home is closed only set this.
    stale: bool,
}

impl gpui::EventEmitter<OpenFromHome> for HomeView {}

impl HomeView {
    pub fn new(
        library: Entity<Library>,
        cache: Entity<LruImageCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |this, _, cx| {
            this.stale = true;
            cx.notify();
        })
        .detach();
        Self {
            library,
            cache,
            dashboard: Dashboard::default(),
            stale: true,
        }
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let library = self.library.read(cx);
        let collections: Vec<(Uuid, String)> = library
            .source
            .iter()
            .flat_map(|(_, m)| &m.definitions.collections)
            .filter(|c| !c.archived)
            .map(|c| (c.id, c.name.clone()))
            .collect();
        self.dashboard = Dashboard::build(&library.games, &collections);
        self.stale = false;
    }
}

fn hours(minutes: u64) -> String {
    if minutes < 600 {
        format!("{:.1} h", minutes as f64 / 60.)
    } else {
        format!("{} h", (minutes + 30) / 60)
    }
}

fn games_label(count: usize) -> String {
    if count == 1 {
        "1 game".into()
    } else {
        format!("{count} games")
    }
}

impl HomeView {
    fn heading(&self, title: &str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .text_sm()
            .font_semibold()
            .text_color(cx.theme().muted_foreground)
            .child(title.to_owned())
    }

    fn section(
        &self,
        title: &str,
        body: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex().gap_3().child(self.heading(title, cx)).child(body)
    }

    fn note(&self, text: &str, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text.to_owned())
            .into_any_element()
    }

    /// Panels share a row and wrap below this width.
    fn panel(&self, title: &str, cx: &mut Context<Self>) -> gpui::Div {
        v_flex()
            .flex_1()
            .min_w(px(280.))
            .p_4()
            .gap_3()
            .rounded(crate::theme::interface_radius(cx, px(12.)))
            .border_1()
            .border_color(cx.theme().border.opacity(0.4))
            .bg(cx.theme().background.opacity(0.55))
            .child(self.heading(title, cx))
    }

    fn cover_row(&self, games: &[usize], cx: &mut Context<Self>) -> gpui::AnyElement {
        let library = self.library.read(cx);
        let tiles: Vec<_> = games
            .iter()
            .filter_map(|&i| library.games.get(i).cloned())
            .collect();
        h_flex()
            .flex_wrap()
            .gap_4()
            .children(tiles.into_iter().map(|game| {
                let id = game.id;
                v_flex()
                    .id(gpui::SharedString::from(format!("home-game-{id}")))
                    .w(px(120.))
                    .gap_1()
                    .cursor_pointer()
                    .child(
                        div()
                            .w(px(120.))
                            .h(px(180.))
                            .overflow_hidden()
                            .rounded(crate::theme::interface_radius(cx, px(8.)))
                            .border_2()
                            .border_color(cx.theme().transparent)
                            .hover(|style| style.border_color(cx.theme().ring.opacity(0.6)))
                            .bg(cx.theme().muted)
                            .child(
                                img(game
                                    .cover_path
                                    .clone()
                                    .map(gpui::ImageSource::from)
                                    .unwrap_or_else(|| game.cover.clone().into()))
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .with_fallback(|| div().size_full().into_any_element()),
                            ),
                    )
                    .child(div().text_xs().truncate().child(game.title))
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(OpenFromHome(id))))
            }))
            .into_any_element()
    }

    fn platforms(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let totals = self.dashboard.platforms;
        if totals.total() == 0 {
            return self.note("No Steam playtime yet.", cx);
        }
        let max = totals
            .rows()
            .iter()
            .map(|(_, m)| *m)
            .max()
            .unwrap_or(1)
            .max(1);
        v_flex()
            .gap_2()
            .children(
                totals
                    .rows()
                    .into_iter()
                    .filter(|(_, minutes)| *minutes > 0)
                    .map(|(label, minutes)| {
                        h_flex()
                            .gap_3()
                            .text_sm()
                            .child(div().w(px(84.)).flex_shrink_0().child(label))
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(cx.theme().muted)
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(minutes as f32 / max as f32))
                                            .rounded_full()
                                            .bg(cx.theme().primary),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(64.))
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(hours(minutes)),
                            )
                    }),
            )
            .when(totals.other > 0, |rows| {
                rows.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Other is playtime that Steam does not assign to a platform."),
                )
            })
            .into_any_element()
    }

    fn genres(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.dashboard.genres.is_empty() {
            return self.note("Genres appear after a Steam sync.", cx);
        }
        v_flex()
            .gap_1()
            .children(self.dashboard.genres.iter().map(|(name, count)| {
                let rule = SmartRule::Genre(name.clone());
                h_flex()
                    .id(gpui::SharedString::from(format!("home-genre-{name}")))
                    .px_2()
                    .py_1()
                    .gap_2()
                    .text_sm()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().secondary))
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(games_label(*count)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.library.update(cx, |lib, cx| {
                            lib.set_scope(Scope::Smart(rule.clone()));
                            cx.notify();
                        });
                    }))
            }))
            .into_any_element()
    }

    fn collection_tile(&self, tile: &CollectionTile, cx: &mut Context<Self>) -> impl IntoElement {
        let id = tile.id;
        let covers: Vec<_> = {
            let library = self.library.read(cx);
            tile.covers
                .iter()
                .filter_map(|&i| library.games.get(i))
                .map(|game| {
                    game.cover_path
                        .clone()
                        .map(gpui::ImageSource::from)
                        .unwrap_or_else(|| game.cover.clone().into())
                })
                .collect()
        };
        h_flex()
            .id(gpui::SharedString::from(format!("home-collection-{id}")))
            .w(px(260.))
            .p_3()
            .gap_3()
            .rounded(crate::theme::interface_radius(cx, px(12.)))
            .border_1()
            .border_color(cx.theme().border.opacity(0.4))
            .bg(cx.theme().background.opacity(0.55))
            .cursor_pointer()
            .hover(|style| style.border_color(cx.theme().ring.opacity(0.6)))
            .child(h_flex().w(px(92.)).h(px(60.)).flex_shrink_0().children(
                covers.into_iter().enumerate().map(|(n, source)| {
                    div()
                        .w(px(40.))
                        .h(px(60.))
                        .when(n > 0, |cover| cover.ml(px(-14.)))
                        .overflow_hidden()
                        .rounded(crate::theme::interface_radius(cx, px(4.)))
                        .border_1()
                        .border_color(cx.theme().background)
                        .bg(cx.theme().muted)
                        .child(
                            img(source)
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .with_fallback(|| div().size_full().into_any_element()),
                        )
                }),
            ))
            .child(
                v_flex()
                    .min_w_0()
                    .child(div().font_medium().truncate().child(tile.name.clone()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(games_label(tile.count)),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.library.update(cx, |lib, cx| {
                    lib.set_scope(Scope::Collection(id));
                    cx.notify();
                });
            }))
    }
}

impl Render for HomeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.stale {
            self.rebuild(cx);
        }
        let (connected, demo) = {
            let library = self.library.read(cx);
            (
                library
                    .source
                    .as_ref()
                    .is_some_and(|(_, m)| m.definitions.steam_account.is_some()),
                library.demo,
            )
        };
        let dash = self.dashboard.clone();
        let recent = if !dash.recent.is_empty() {
            self.cover_row(&dash.recent, cx)
        } else if dash.steam_games > 0 {
            self.note("Sync Steam to see recent games.", cx)
        } else {
            self.note("Recently played Steam games appear here.", cx)
        };
        let favorites = if dash.favorites.is_empty() {
            self.note("Mark games as favorites to see them here.", cx)
        } else {
            self.cover_row(&dash.favorites, cx)
        };
        let collections = if dash.collections.is_empty() {
            self.note("Create a collection in the sidebar to see it here.", cx)
        } else {
            h_flex()
                .flex_wrap()
                .gap_3()
                .children(
                    dash.collections
                        .iter()
                        .map(|tile| self.collection_tile(tile, cx)),
                )
                .into_any_element()
        };
        let platforms = self.platforms(cx);
        let genres = self.genres(cx);
        div().id("home").size_full().overflow_y_scroll().child(
            image_cache(self.cache.clone()).child(
                v_flex()
                    .pt(px(84.))
                    .pb(px(40.))
                    .px(px(28.))
                    .gap(px(32.))
                    .when(!connected && !demo, |page| {
                        page.child(
                            h_flex()
                                .p_4()
                                .gap_4()
                                .rounded(crate::theme::interface_radius(cx, px(12.)))
                                .bg(cx.theme().secondary)
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .child("Connect Steam to import your games and playtime."),
                                )
                                .child(
                                    Button::new("home-connect-steam")
                                        .primary()
                                        .label("Connect Steam")
                                        .on_click(|_, window, cx| {
                                            window
                                                .dispatch_action(Box::new(crate::ConnectSteam), cx)
                                        }),
                                ),
                        )
                    })
                    .child(self.section("Continue playing", recent, cx))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_start()
                            .gap_4()
                            .child(self.panel("Playtime by platform", cx).child(platforms))
                            .child(self.panel("Favorite genres", cx).child(genres)),
                    )
                    .child(self.section("Favorite games", favorites, cx))
                    .child(self.section("Collections", collections, cx)),
            ),
        )
    }
}
