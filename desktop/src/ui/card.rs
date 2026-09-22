//! Native card face. Artwork is composited once before applying its rounded mask.
use crate::model::Game;
use gpui::{
    div, img, linear_color_stop, linear_gradient, point, prelude::*, px, rgb, rgba, App, BoxShadow,
    ObjectFit,
};
use gpui_component::{h_flex, v_flex, ActiveTheme as _};

pub fn tabletop(cx: &App) -> gpui::Hsla {
    cx.theme().background
}

pub fn front(
    game: &Game,
    width: f32,
    light: Option<(f32, f32)>,
    _selected: bool,
    cx: &App,
) -> gpui::Div {
    let height = width * 1.46;
    let artwork_size = gpui::size(px(width - 20.), px(height - 49.));
    let edge = cx.theme().border;
    let paper = cx.theme().background;
    let angle = light.map_or(140., |(x, y)| 105. + x * 55. + y * 25.);
    let shine = if light.is_some() { 0.23 } else { 0.045 };
    v_flex()
        .relative()
        .w(px(width))
        .h(px(height))
        .flex_shrink_0()
        .p(px(9.))
        .gap(px(7.))
        .rounded(px(17.))
        .bg(linear_gradient(
            145.,
            linear_color_stop(paper, 0.),
            linear_color_stop(cx.theme().secondary, 1.),
        ))
        .border_1()
        .border_color(edge.opacity(0.55))
        .shadow(vec![
            BoxShadow {
                color: rgba(0x17202c12).into(),
                offset: point(px(0.), px(10.)),
                blur_radius: px(34.),
                spread_radius: px(-6.),
            },
            BoxShadow {
                color: rgba(0x17202c08).into(),
                offset: point(px(0.), px(3.)),
                blur_radius: px(12.),
                spread_radius: px(0.),
            },
        ])
        .child(gpui::card_layer(
            surface_id(game.id, 0),
            artwork_size,
            gpui::CardPose::default(),
            px(10.),
            div()
                .relative()
                .w(artwork_size.width)
                .h(artwork_size.height)
                .overflow_hidden()
                .bg(cx.theme().secondary)
                .child(
                    img(game
                        .cover_path
                        .clone()
                        .map(gpui::ImageSource::from)
                        .unwrap_or_else(|| game.cover.clone().into()))
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_fallback(|| {
                        v_flex()
                            .size_full()
                            .justify_center()
                            .items_center()
                            .gap_3()
                            .child(div().text_3xl().child("✦"))
                            .child(div().text_sm().child("Cover unavailable"))
                            .into_any_element()
                    }),
                )
                .child(div().absolute().inset_0().bg(linear_gradient(
                    180.,
                    linear_color_stop(rgba(0x10131a00), 0.42),
                    linear_color_stop(rgba(0x10131af2), 1.),
                )))
                .child(div().absolute().inset_0().bg(linear_gradient(
                    angle,
                    linear_color_stop(gpui::Hsla::from(rgb(0xffffff)).opacity(shine), 0.),
                    linear_color_stop(
                        gpui::Hsla::from(rgb(0xa8c9f5)).opacity(if light.is_some() {
                            0.07
                        } else {
                            0.
                        }),
                        0.75,
                    ),
                )))
                .child(
                    v_flex()
                        .absolute()
                        .bottom(px(18.))
                        .left(px(17.))
                        .right(px(17.))
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(if width > 350. { 32. } else { 23. }))
                                .font_family("Georgia")
                                .line_clamp(3)
                                .text_color(rgb(0xffffff))
                                .line_height(px(if width > 350. { 35. } else { 27. }))
                                .child(game.title.clone()),
                        ),
                ),
        ))
        .child(
            h_flex()
                .h(px(22.))
                .flex_shrink_0()
                .px_1()
                .justify_between()
                .gap_2()
                .text_size(px(10.))
                .text_color(cx.theme().muted_foreground)
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .overflow_hidden()
                        .child(badge(game.status_label.clone(), cx))
                        .children(game.collections.first().map(|name| badge(name.clone(), cx)))
                        .when(game.collections.len() > 1, |row| {
                            row.child(
                                div()
                                    .flex_shrink_0()
                                    .child(format!("+{}", game.collections.len() - 1)),
                            )
                        }),
                )
                .child(
                    div().flex_shrink_0().child(
                        game.rating
                            .map_or("✦".to_owned(), |r| format!("★ {:.1}", f32::from(r) / 2.)),
                    ),
                ),
        )
}

/// Face roles keep the artwork, front and editor textures independent.
pub fn surface_id(id: uuid::Uuid, role: u8) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (id, role).hash(&mut hash);
    hash.finish()
}

/// Broad, low-opacity shadow for the settled native editor.
pub fn floating_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: rgba(0x10182716).into(),
        offset: point(px(0.), px(18.)),
        blur_radius: px(48.),
        spread_radius: px(-8.),
    }]
}

fn badge(label: String, cx: &App) -> gpui::Div {
    div()
        .min_w_0()
        .max_w(px(125.))
        .px(px(6.))
        .py(px(2.))
        .rounded_full()
        .bg(cx.theme().muted_foreground.opacity(0.10))
        .text_color(cx.theme().foreground.opacity(0.75))
        .truncate()
        .child(label)
}
