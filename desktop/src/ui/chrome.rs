//! Window chrome geometry shared by every GameSync window.
//!
//! macOS: the Apple UI kit places the window controls (68 x 14) at (18, 19)
//! inside a 52 pt unified toolbar row. GPUI applies the traffic-light position
//! only when a window opens, so macOS uses this geometry in every look.
use gpui::{point, px, App, SharedString, TitlebarOptions};

/// Height reserved at the window top for the traffic lights and titlebar drag area.
pub const TITLEBAR: f32 = if cfg!(target_os = "macos") { 52. } else { 34. };

/// Trailing edge of the macOS window controls plus the kit's 8 pt toolbar gap.
pub const TRAFFIC_LIGHTS_END: f32 = 18. + 68. + 8.;

/// Library toolbar height under the bundled Theme look; content offsets assume it.
const THEME_TOOLBAR: f32 = 68.;

/// Title row of auxiliary windows (Settings, Collections): the standard macOS
/// titlebar height, with the window controls centered in it.
pub const COMPACT_TITLEBAR: f32 = 34.;

/// Main window: the controls sit in the unified toolbar row.
pub fn titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    with_traffic_lights(title, 18., 19.)
}

/// Auxiliary windows put a title row above their own toolbar, as macOS Settings windows do.
pub fn compact_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    with_traffic_lights(title, 13., (COMPACT_TITLEBAR - 14.) / 2.)
}

fn with_traffic_lights(title: impl Into<SharedString>, x: f32, y: f32) -> TitlebarOptions {
    let defaults = gpui_component::TitleBar::title_bar_options();
    TitlebarOptions {
        title: Some(title.into()),
        traffic_light_position: if cfg!(target_os = "macos") {
            Some(point(px(x), px(y)))
        } else {
            defaults.traffic_light_position
        },
        ..defaults
    }
}

/// The window-top drag area. Native macOS windows have one unified surface, so
/// the row takes the window background instead of a separate titlebar color.
pub fn titlebar(cx: &App) -> gpui_component::TitleBar {
    titlebar_with_height(TITLEBAR, cx)
}

pub fn compact_titlebar(cx: &App) -> gpui_component::TitleBar {
    titlebar_with_height(
        if cfg!(target_os = "macos") {
            COMPACT_TITLEBAR
        } else {
            TITLEBAR
        },
        cx,
    )
}

fn titlebar_with_height(height: f32, cx: &App) -> gpui_component::TitleBar {
    use gpui::Styled as _;
    let bar = gpui_component::TitleBar::new().border_b_0().h(px(height));
    if crate::theme::macos_shell(cx) {
        bar.bg(gpui::transparent_black())
    } else {
        bar
    }
}

/// The library toolbar shares the 52 pt titlebar row in the native macOS shell.
pub fn toolbar_height(cx: &App) -> f32 {
    if crate::theme::macos_shell(cx) {
        TITLEBAR
    } else {
        THEME_TOOLBAR
    }
}

/// Add to content top offsets that were tuned for the Theme toolbar height.
pub fn toolbar_shift(cx: &App) -> f32 {
    toolbar_height(cx) - THEME_TOOLBAR
}
