//! Eagle's inspector layout adapted to game metadata.

use crate::{model::Library, ui::thumb_cache::LruImageCache};
use gpui::{div, image_cache, img, prelude::*, px, App, Entity, ObjectFit, Window};
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, StyledExt as _};

pub struct DetailPanel {
    library: Entity<Library>,
    cache: Entity<LruImageCache>,
}

impl DetailPanel {
    pub fn new(
        library: Entity<Library>,
        cache: Entity<LruImageCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        Self { library, cache }
    }
}

impl Render for DetailPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = v_flex()
            .id("detail")
            .w(px(320.))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar);
        let Some(game) = self.library.read(cx).selected_game().cloned() else {
            return panel
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::PanelRight))
                .child(div().text_sm().child("Select a game"))
                .child(div().text_xs().child("Its details will appear here."))
                .into_any_element();
        };

        panel
            .child(
                image_cache(self.cache.clone())
                    .w_full()
                    .h(px(240.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .size_full()
                            .p_3()
                            .flex()
                            .justify_center()
                            .bg(cx.theme().secondary)
                            .child(
                                img(game.cover.clone())
                                    .h_full()
                                    .object_fit(ObjectFit::Contain)
                                    .with_fallback(|| {
                                        div().p_4().child("Cover unavailable").into_any_element()
                                    }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .id("detail-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .gap_4()
                    .child(div().text_lg().font_semibold().child(game.title.clone()))
                    .child(
                        h_flex()
                            .gap_4()
                            .child(field("Status", game.status.label(), cx))
                            .child(field("Your rating", game.rating_label(), cx)),
                    )
                    .child(field(
                        "Time played",
                        format!("{:.1} hours", game.playtime_minutes as f32 / 60.),
                        cx,
                    ))
                    .child(field("About", game.description.clone(), cx))
                    .child(
                        v_flex().gap_2().child(label("Tags", cx)).child(
                            h_flex()
                                .flex_wrap()
                                .gap_1()
                                .children(game.tags.iter().map(|tag| {
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().secondary)
                                        .text_xs()
                                        .child(tag.clone())
                                })),
                        ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Sample details. Editing and saving arrive in the next milestone.",
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn label(text: &str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_owned())
}

fn field(name: &str, value: impl Into<String>, cx: &App) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(label(name, cx))
        .child(div().text_sm().child(value.into()))
}
