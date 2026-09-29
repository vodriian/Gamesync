//! Compact wishlist price treatment shared by Cards, Grid, Table, and details.

use crate::{assets::DiscountIcon, model::Game};
use gpui::{div, prelude::*, App};
use gpui_component::{h_flex, ActiveTheme as _, Icon, IconName, StyledExt as _};

pub(super) fn wishlist_price(game: &Game, cx: &App) -> gpui::AnyElement {
    let display = game.price_parts();
    h_flex()
        .min_w_0()
        .gap_1()
        .child(div().flex_shrink_0().font_medium().child(display.price))
        .children(display.discount.map(|discount| {
            h_flex()
                .min_w_0()
                .gap_0p5()
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(DiscountIcon).size_3())
                .child(div().truncate().child(discount))
        }))
        .when(display.sale_ends_soon, |row| {
            row.child(
                Icon::new(IconName::TriangleAlert)
                    .size_3()
                    .text_color(cx.theme().warning),
            )
        })
        .into_any_element()
}
