//! GPUI clips overflow to rectangles. Paint a rounded frame instead of capturing
//! the whole library into a texture; this keeps virtualization and shadows native.
use gpui::{canvas, div, prelude::*, px, App, Bounds, Pixels};
use gpui_component::ActiveTheme as _;

pub fn content(child: impl IntoElement, cx: &App) -> gpui::Div {
    let chrome = cx.theme().sidebar;
    let border = cx.theme().border;
    div()
        .relative()
        .size_full()
        .rounded(px(12.))
        .overflow_hidden()
        .child(child)
        .child(
            canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window, _| {
                    let w = f32::from(bounds.size.width);
                    let h = f32::from(bounds.size.height);
                    let r = 12_f32.min(w / 2.).min(h / 2.);
                    // The frame lies outside the panel except at its corners. Its
                    // inner radius matches the panel; the rectangular content mask
                    // clips away the rest. Use quads so edge coverage matches GPUI's
                    // rounded outline, including on Retina displays.
                    let mut mask =
                        gpui::outline(bounds.dilate(px(r)), chrome, gpui::BorderStyle::Solid);
                    mask.border_widths = px(r).into();
                    mask.corner_radii = px(r * 2.).into();
                    window.paint_quad(mask);
                    let mut edge = gpui::outline(bounds, border, gpui::BorderStyle::Solid);
                    edge.corner_radii = px(r).into();
                    window.paint_quad(edge);
                },
            )
            .absolute()
            .inset_0(),
        )
}
