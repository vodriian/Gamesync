//! Baseline colors resolved offline; native controls consume one semantic palette.
use gamesync_desktop::{
    appearance::{Appearance, Palette},
    omarchy::OmarchyTheme,
};
use gpui::{px, App, Hsla, Pixels, Window, WindowAppearance};
use gpui_component::{Theme, ThemeColor, ThemeMode};
use std::collections::BTreeMap;

fn color(tokens: &BTreeMap<String, String>, key: &str) -> Hsla {
    let value = &tokens[key];
    gpui::rgba(u32::from_str_radix(&value[1..], 16).expect("validated palette color")).into()
}
fn ink(background: Hsla) -> Hsla {
    let c = gpui::Rgba::from(background);
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    gpui::rgb(
        if 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b) > 0.179 {
            0x161616
        } else {
            0xffffff
        },
    )
    .into()
}
pub fn apply_choice(choice: &Appearance, window: &mut Window, cx: &mut App) {
    gpui::set_linux_card_compositor_enabled(false);
    let dark = matches!(
        window.appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    let palette = choice.palette(dark);
    let mode = if palette.mode == "dark" {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    Theme::change(mode, Some(window), cx);
    Theme::global_mut(cx).colors = resolve(palette, choice.interface_accent);
    cx.refresh_windows();
}

pub fn apply_omarchy(theme: &OmarchyTheme, window: &mut Window, cx: &mut App) {
    gpui::set_linux_card_compositor_enabled(true);
    let palette = omarchy_palette(theme);
    let mode = if theme.mode == "dark" {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    Theme::change(mode, Some(window), cx);
    let native = Theme::global_mut(cx);
    native.colors = resolve(&palette, true);
    // Omarchy uses square corners throughout its application design language.
    // Custom GameSync surfaces use this value as their mode signal too.
    native.radius = px(0.);
    native.radius_lg = px(0.);
    cx.refresh_windows();
    log::info!("Applied Omarchy theme: {}", theme.name);
}

/// Keep each surface's current radius outside Omarchy, but square it when the
/// global control radius marks the active Omarchy design language.
pub fn interface_radius(cx: &App, normal: Pixels) -> Pixels {
    if omarchy_mode(cx) {
        px(0.)
    } else {
        normal
    }
}

pub fn omarchy_mode(cx: &App) -> bool {
    Theme::global(cx).radius == px(0.)
}

pub fn pill_radius(cx: &App) -> Pixels {
    interface_radius(cx, px(999.))
}

fn omarchy_palette(theme: &OmarchyTheme) -> Palette {
    let mut content = BTreeMap::new();
    let mut set = |target: &str, source: &str| {
        content.insert(target.to_owned(), theme.color(source).to_owned());
    };
    set("background-primary", "background");
    set("background-primary-alt", "lighter_background");
    set("background-modifier-border", "muted");
    set("background-modifier-hover", "selection");
    set("interactive-normal", "lighter_background");
    set("interactive-accent", "accent");
    set("interactive-accent-hover", "accent");
    set("text-normal", "foreground");
    set("text-muted", "dark_foreground");
    set("text-selection", "selection");
    set("color-red", "red");
    set("color-green", "green");
    set("color-blue", "blue");
    set("color-yellow", "yellow");
    set("color-purple", "magenta");
    set("color-cyan", "cyan");

    let mut chrome = content.clone();
    // Omarchy's shell uses the primary theme background for bars, popups, and
    // menus. Keep the sidebar on that same surface instead of inventing a
    // separate dark chrome shade.
    chrome.insert(
        "background-secondary".into(),
        theme.color("background").to_owned(),
    );
    Palette {
        id: "omarchy".into(),
        mode: theme.mode.clone(),
        contrast: "normal".into(),
        scheme_accent: true,
        content,
        chrome,
    }
}
fn resolve(p: &Palette, accented: bool) -> ThemeColor {
    let c = |key| color(&p.content, key);
    let h = |key| color(&p.chrome, key);
    let bg = c("background-primary");
    let fg = c("text-normal");
    let muted = c("text-muted");
    let border = c("background-modifier-border");
    let raised = bg.blend(c("background-primary-alt"));
    let control = bg.blend(c("interactive-normal"));
    let hover = bg.blend(c("background-modifier-hover"));
    let primary = if accented {
        c("interactive-accent")
    } else {
        fg
    };
    let primary_hover = if accented {
        c("interactive-accent-hover")
    } else {
        fg.opacity(0.8)
    };
    let selection = bg.blend(c("text-selection"));
    let chrome = h("background-secondary");
    let chrome_fg = h("text-normal");
    let chrome_hover = chrome.blend(h("background-modifier-hover"));
    let chrome_selection = chrome.blend(if accented {
        h("text-selection")
    } else {
        h("background-modifier-hover")
    });
    let red = c("color-red");
    let green = c("color-green");
    let blue = c("color-blue");
    let yellow = c("color-yellow");
    let purple = c("color-purple");
    let cyan = c("color-cyan");
    ThemeColor {
        accent: chrome_hover,
        accent_foreground: chrome_fg,
        accordion: bg,
        accordion_hover: hover,
        background: bg,
        border,
        group_box: raised,
        group_box_foreground: fg,
        caret: primary,
        chart_1: blue,
        chart_2: green,
        chart_3: yellow,
        chart_4: purple,
        chart_5: cyan,
        danger: red,
        danger_active: red,
        danger_foreground: ink(red),
        danger_hover: red,
        description_list_label: raised,
        description_list_label_foreground: muted,
        drag_border: primary,
        drop_target: selection,
        foreground: fg,
        info: blue,
        info_active: blue,
        info_foreground: ink(blue),
        info_hover: blue,
        input: border,
        link: primary,
        link_active: primary,
        link_hover: primary,
        list: bg,
        list_active: selection,
        list_active_border: fg,
        list_even: raised,
        list_head: raised,
        list_hover: hover,
        muted: raised,
        muted_foreground: muted,
        popover: chrome,
        popover_foreground: chrome_fg,
        primary,
        primary_active: primary_hover,
        primary_foreground: ink(primary),
        primary_hover,
        progress_bar: primary,
        ring: primary,
        scrollbar: gpui::transparent_black(),
        scrollbar_thumb: fg.opacity(0.18),
        scrollbar_thumb_hover: fg.opacity(0.35),
        secondary: control,
        secondary_active: hover,
        secondary_foreground: fg,
        secondary_hover: hover,
        selection,
        sidebar: chrome,
        sidebar_accent: chrome_selection,
        sidebar_accent_foreground: chrome_fg,
        sidebar_border: h("background-modifier-border"),
        sidebar_foreground: chrome_fg,
        sidebar_primary: primary,
        sidebar_primary_foreground: ink(primary),
        skeleton: raised,
        slider_bar: primary,
        slider_thumb: ink(primary),
        success: green,
        success_foreground: ink(green),
        success_hover: green,
        success_active: green,
        bullish: green,
        bearish: red,
        switch: control,
        switch_thumb: ink(primary),
        tab: raised,
        tab_active: bg,
        tab_active_foreground: fg,
        tab_bar: raised,
        tab_bar_segmented: raised,
        tab_foreground: muted,
        table: bg,
        table_active: selection,
        table_active_border: primary,
        table_even: raised,
        table_head: raised,
        table_head_foreground: muted,
        table_hover: hover,
        table_row_border: border,
        title_bar: chrome,
        title_bar_border: h("background-modifier-border"),
        tiles: bg,
        warning: yellow,
        warning_active: yellow,
        warning_hover: yellow,
        warning_foreground: ink(yellow),
        overlay: gpui::rgba(0x00000059).into(),
        window_border: border,
        red,
        red_light: red,
        green,
        green_light: green,
        blue,
        blue_light: blue,
        yellow,
        yellow_light: yellow,
        magenta: purple,
        magenta_light: purple,
        cyan,
        cyan_light: cyan,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_palette_produces_finite_native_tokens() {
        for palette in &gamesync_desktop::appearance::catalog().palettes {
            for accented in [false, true] {
                let colors = resolve(palette, accented);
                let value = serde_json::to_value(colors).unwrap();
                assert!(value.is_object());
                assert_ne!(colors.background, colors.foreground);
                assert_ne!(colors.sidebar, colors.sidebar_foreground);
                assert_ne!(colors.primary, colors.primary_foreground);
            }
        }
    }
    #[test]
    fn neutral_controls_keep_semantic_colors() {
        let a = Appearance::default();
        let normal = resolve(a.palette(false), true);
        let neutral = resolve(a.palette(false), false);
        assert_eq!(neutral.primary, neutral.foreground);
        assert_ne!(normal.primary, neutral.primary);
        assert_eq!(normal.danger, neutral.danger);
        assert_eq!(normal.success, neutral.success);
    }
}
