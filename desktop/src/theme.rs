//! System-driven appearance: take the desktop's colours for the app's chrome.
//!
//! Only the chrome follows the desktop — sidebar, toolbars, filmstrip, status
//! bar. The canvas the images sit on stays the theme's own neutral dark, because
//! a tinted backdrop shifts how a photograph reads and this is a tool for
//! looking at photographs. Eagle makes the same choice.
//!
//! What each platform is willing to tell us differs sharply, and the ceiling is
//! low:
//!
//! - **KDE** writes a full semantic palette to `kdeglobals`, which is what this
//!   reads. It is the only desktop that hands over a usable palette.
//! - **Other Linux desktops** expose only `color-scheme`, `accent-color` and
//!   `contrast` through the XDG appearance portal — no surfaces. GNOME's palette
//!   lives in GTK CSS with no API in front of it.
//! - **macOS** has no user-defined window background at all; the palette is
//!   fixed per light/dark and only the accent is configurable. "Respecting the
//!   system theme" there means light/dark plus accent, which is already what
//!   native apps do.
//!
//! So this returns `None` on anything but KDE for now, and the app keeps its
//! own palette. See [`chrome`] for where the other platforms slot in.

use std::{path::PathBuf, rc::Rc};

use gpui::{App, Hsla, Rgba, Window};
use gpui_component::{Theme, ThemeConfig, ThemeMode, ThemeRegistry, ThemeSet};

pub const SYSTEM_THEME: &str = "system";

/// Themes bundled with the app.
///
/// gpui-component ships only its own light and dark; the named schemes below
/// are ordinary `ThemeSet` JSON, which is a supported public format. They are
/// compiled in rather than loaded through `ThemeRegistry` because the registry
/// only ever reads from a watched directory on disk and exposes no way to
/// register a theme programmatically.
const BUNDLED: &[&str] = &[
    include_str!("themes/catppuccin-latte.json"),
    include_str!("themes/catppuccin-mocha.json"),
    include_str!("themes/dracula.json"),
    include_str!("themes/gruvbox-dark.json"),
    include_str!("themes/nord.json"),
    include_str!("themes/rose-pine.json"),
    include_str!("themes/solarized-light.json"),
    include_str!("themes/tokyo-night.json"),
];

/// The fixed choices, in the order the settings panel lists them.
pub const LIGHT_THEME: &str = "light";
pub const DARK_THEME: &str = "dark";

/// Every bundled theme, parsed. Names are what `the theme selector` stores.
pub fn bundled() -> Vec<Rc<ThemeConfig>> {
    BUNDLED
        .iter()
        .filter_map(|raw| match serde_json::from_str::<ThemeSet>(raw) {
            Ok(set) => Some(set.themes),
            Err(err) => {
                // Compiled in, so this is a bug in our own JSON rather than
                // anything the user can fix — but do not take the app down.
                log::error!("bundled theme is malformed: {err}");
                None
            }
        })
        .flatten()
        .map(Rc::new)
        .collect()
}

/// Names of the bundled themes, for the settings panel.
pub fn bundled_names() -> Vec<String> {
    bundled().iter().map(|t| t.name.to_string()).collect()
}

/// Put the saved theme into effect.
///
/// This is the single entry point: startup, a settings change and a desktop
/// appearance change all go through it, so there is one description of what
/// "the current theme" means.
pub fn apply_choice(choice: &str, window: &mut Window, cx: &mut App) {
    match choice {
        SYSTEM_THEME => {
            restore_default_configs(cx);
            Theme::sync_system_appearance(Some(window), cx);
            // Only system mode takes the desktop's colours. A named scheme is
            // an explicit choice to override them, and layering KDE's window
            // colour over Dracula would leave neither.
            apply(cx);
        }
        LIGHT_THEME => {
            restore_default_configs(cx);
            Theme::change(ThemeMode::Light, Some(window), cx);
        }
        DARK_THEME => {
            restore_default_configs(cx);
            Theme::change(ThemeMode::Dark, Some(window), cx);
        }
        name => {
            let Some(config) = bundled().into_iter().find(|t| t.name.as_ref() == name) else {
                log::warn!("unknown theme {name:?}; falling back to the system theme");
                restore_default_configs(cx);
                Theme::sync_system_appearance(Some(window), cx);
                apply(cx);
                return;
            };
            // A named scheme carries its own mode, so it stops following the
            // desktop: `Theme` holds exactly one light and one dark config, and
            // most of these palettes only define one of the two.
            let mode = config.mode;
            let theme = Theme::global_mut(cx);
            match mode {
                ThemeMode::Light => theme.light_theme = config.clone(),
                ThemeMode::Dark => theme.dark_theme = config.clone(),
            }
            theme.apply_config(&config);
            theme.mode = mode;
            window.refresh();
        }
    }
}

/// Put gpui-component's own light and dark configs back into `Theme`.
///
/// `Theme` holds exactly one light and one dark `ThemeConfig`, and selecting a
/// bundled scheme overwrites whichever of the two matches its mode. Nothing in
/// gpui-component ever puts the default back: `Theme::change` seeds those two
/// slots from the registry only when the global does not exist yet, which is
/// once, at startup.
///
/// So this has to run before every light/dark/system apply. Without it,
/// choosing Dracula and then System leaves System on Dracula's palette — the
/// desktop's light/dark preference is followed, its colours are not, and the
/// app no longer looks like anything else on the machine.
fn restore_default_configs(cx: &mut App) {
    if !cx.has_global::<Theme>() || !cx.has_global::<ThemeRegistry>() {
        return;
    }
    // Cloned out first: both globals cannot be borrowed at once.
    let registry = ThemeRegistry::global(cx);
    let light = registry.default_light_theme().clone();
    let dark = registry.default_dark_theme().clone();
    let theme = Theme::global_mut(cx);
    theme.light_theme = light;
    theme.dark_theme = dark;
}

/// The handful of colours worth taking from a desktop theme.
#[derive(Debug, Clone, Copy)]
pub struct Chrome {
    /// Window/chrome background — KDE's `Colors:Window`.
    pub window: Hsla,
    /// The lighter companion, used for borders and separators.
    pub window_alt: Hsla,
    pub foreground: Hsla,
    /// Selected-row background, and the focus ring.
    pub selection: Hsla,
    pub selection_foreground: Hsla,
}

/// Read the desktop's chrome colours, or `None` if it does not publish any.
pub fn chrome() -> Option<Chrome> {
    // The place to add other platforms: a portal `accent-color` read for
    // non-KDE Linux, and `AppleHighlightColor` on macOS. Both give an accent
    // and nothing else, so they would fill `selection` and leave the surfaces
    // to the app's own theme.
    kde_chrome()
}

/// Apply the desktop's chrome over the current theme.
///
/// Must run *after* every `Theme::sync_system_appearance`, because applying a
/// light/dark config overwrites `colors` wholesale and would drop these.
///
/// Only reached in system mode — see [`apply_choice`].
fn apply(cx: &mut App) {
    let Some(chrome) = chrome() else {
        return;
    };
    // `background`, `list` and `table` are deliberately absent below: those are
    // the canvas, and they keep whatever the base light/dark theme chose.
    let theme = Theme::global_mut(cx);
    theme.colors.sidebar = chrome.window;
    theme.colors.sidebar_foreground = chrome.foreground;
    theme.colors.sidebar_border = chrome.window_alt;
    theme.colors.sidebar_accent = chrome.selection;
    theme.colors.sidebar_accent_foreground = chrome.selection_foreground;
    theme.colors.title_bar = chrome.window;
    theme.colors.title_bar_border = chrome.window_alt;
    theme.colors.border = chrome.window_alt;
    theme.colors.ring = chrome.selection;
    // Menus and popovers are chrome too. Without these they keep the base
    // light/dark palette while everything around them takes the desktop's,
    // and a menu opened over the sidebar is visibly a different colour.
    theme.colors.popover = chrome.window;
    theme.colors.popover_foreground = chrome.foreground;
    // What a menu row highlights with — `sidebar_accent`'s counterpart for
    // popovers, and it derives from `secondary` otherwise, which the desktop
    // never sets.
    theme.colors.accent = chrome.selection;
    theme.colors.accent_foreground = chrome.selection_foreground;
}

fn kdeglobals() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().join(".config")))?;
    let path = base.join("kdeglobals");
    path.is_file().then_some(path)
}

/// Parse the sections we need out of `kdeglobals`.
///
/// A hand-rolled scan rather than an INI crate: the file is flat, the four keys
/// wanted are `R,G,B` triples, and a malformed one should degrade to the app's
/// own theme rather than fail anything.
fn kde_chrome() -> Option<Chrome> {
    let text = std::fs::read_to_string(kdeglobals()?).ok()?;

    let mut section = String::new();
    let mut find = |wanted_section: &str, wanted_key: &str| -> Option<Hsla> {
        section.clear();
        for line in text.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.to_string();
                continue;
            }
            if section != wanted_section {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == wanted_key {
                    return parse_rgb(value.trim());
                }
            }
        }
        None
    };

    let window = find("Colors:Window", "BackgroundNormal")?;
    let foreground = find("Colors:Window", "ForegroundNormal")?;
    // Each of these has a sane stand-in, so only the two above are required.
    let window_alt = find("Colors:Window", "BackgroundAlternate").unwrap_or(window);
    let selection = find("Colors:Selection", "BackgroundNormal").unwrap_or(window_alt);
    let selection_foreground = find("Colors:Selection", "ForegroundNormal").unwrap_or(foreground);

    Some(Chrome {
        window,
        window_alt,
        foreground,
        selection,
        selection_foreground,
    })
}

/// `"49,45,54"` — KDE writes decimal triples, occasionally with a fourth alpha
/// component, which is ignored.
fn parse_rgb(value: &str) -> Option<Hsla> {
    let mut parts = value.split(',').map(|p| p.trim().parse::<f32>());
    let r = parts.next()?.ok()?;
    let g = parts.next()?.ok()?;
    let b = parts.next()?.ok()?;
    if !(0. ..=255.).contains(&r) || !(0. ..=255.).contains(&g) || !(0. ..=255.).contains(&b) {
        return None;
    }
    Some(
        Rgba {
            r: r / 255.,
            g: g / 255.,
            b: b / 255.,
            a: 1.,
        }
        .into(),
    )
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    /// Every bundled theme must parse. They are compiled in, so a typo here is
    /// a build-time mistake that would otherwise only show up as a theme that
    /// silently renders in shadcn's default colours.
    #[test]
    fn every_bundled_theme_parses() {
        assert_eq!(
            bundled().len(),
            BUNDLED.len(),
            "a bundled theme failed to parse"
        );
    }

    /// A mistyped colour key is not an error to serde — the field simply stays
    /// `None` and silently falls back to shadcn's default. Assert the exact
    /// key spellings against the raw JSON, which is the only place that catches
    /// `"muted"` written where `"muted.background"` was meant.
    #[test]
    fn every_bundled_theme_sets_the_colours_that_have_no_fallback() {
        // These are the keys `ThemeColor::apply_config` has no fallback for;
        // everything else derives from them.
        const REQUIRED: &[&str] = &[
            "background",
            "foreground",
            "border",
            "muted.background",
            "primary.background",
            "secondary.background",
            "overlay",
            "base.red",
            "base.green",
            "base.blue",
            "base.yellow",
            "base.magenta",
            "base.cyan",
        ];

        for raw in BUNDLED {
            let set: serde_json::Value = serde_json::from_str(raw).unwrap();
            let name = set["name"].as_str().unwrap().to_string();
            let colors = set["themes"][0]["colors"].as_object().unwrap();
            for key in REQUIRED {
                assert!(colors.contains_key(*key), "{name} is missing {key:?}");
            }
            for (key, value) in colors {
                let value = value.as_str().unwrap_or_default();
                assert!(
                    value.starts_with('#') && (value.len() == 7 || value.len() == 9),
                    "{name}: {key:?} is not a #RRGGBB or #RRGGBBAA colour ({value:?})"
                );
            }
        }
    }

    /// Names are what `the theme selector` stores, so a duplicate would make one
    /// theme unreachable.
    #[test]
    fn bundled_theme_names_are_unique_and_not_reserved() {
        let names = bundled_names();
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            assert!(seen.insert(name.clone()), "duplicate theme name {name:?}");
            for reserved in [SYSTEM_THEME, LIGHT_THEME, DARK_THEME] {
                assert!(
                    !name.eq_ignore_ascii_case(reserved),
                    "{name:?} collides with the reserved choice {reserved:?}"
                );
            }
        }
        assert_eq!(names.len(), BUNDLED.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_kde_triple() {
        let c = parse_rgb("49,45,54").expect("valid triple");
        let rgba = Rgba::from(c);
        assert_eq!((rgba.r * 255.).round(), 49.);
        assert_eq!((rgba.g * 255.).round(), 45.);
        assert_eq!((rgba.b * 255.).round(), 54.);
    }

    #[test]
    fn ignores_a_trailing_alpha_component() {
        assert!(parse_rgb("104, 128, 228, 255").is_some());
    }

    #[test]
    fn rejects_junk_rather_than_guessing() {
        for bad in ["", "49,45", "a,b,c", "300,0,0", "-1,0,0"] {
            assert!(parse_rgb(bad).is_none(), "{bad:?} should not parse");
        }
    }
}
