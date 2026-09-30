//! Compact wishlist price treatment shared by Cards, Grid, Table, and details.

use crate::{assets::DiscountIcon, model::Game};
use gpui::{div, prelude::*, App};
use gpui_component::{h_flex, ActiveTheme as _, Icon, IconName, StyledExt as _};

pub(super) fn wishlist_price(game: &Game, cx: &App) -> gpui::AnyElement {
    let display = game.price_parts();
    let marks = display.discount.is_some() || display.sale_ends_soon;
    // The price stays at the left; the discount and sale alert sit at the
    // right edge of the row.
    h_flex()
        .flex_1()
        .min_w_0()
        .justify_between()
        .gap_1()
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_medium()
                .child(display.price),
        )
        .when(marks, |row| {
            row.child(
                h_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .children(display.discount.map(|discount| {
                        h_flex()
                            .gap_0p5()
                            .text_color(cx.theme().muted_foreground)
                            .child(Icon::new(DiscountIcon).size_3())
                            .child(discount)
                    }))
                    .when(display.sale_ends_soon, |marks| {
                        marks.child(
                            Icon::new(IconName::TriangleAlert)
                                .size_3()
                                .text_color(cx.theme().warning),
                        )
                    }),
            )
        })
        .into_any_element()
}
