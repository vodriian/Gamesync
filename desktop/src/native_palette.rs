//! Platform-native palettes expressed in the Baseline keys that the theme resolver reads.
//!
//! macOS values come from the Apple macOS 27 UI kit (Sketch): labels, fills,
//! separators, window backgrounds, and system colors. The sidebar is translucent
//! in AppKit; GPUI paints it opaque here, so its value approximates vibrancy
//! over a neutral desktop. Windows values follow WinUI Fluent tokens and are
//! unverified until a Windows visual check.
use crate::appearance::Palette;
use std::collections::BTreeMap;

/// An sRGB accent color as 0xRRGGBB.
pub type Accent = u32;

struct Neutrals {
    window: &'static str,
    sidebar: &'static str,
    raised: &'static str,
    control: &'static str,
    hover: &'static str,
    border: &'static str,
    text: &'static str,
    muted: &'static str,
    faint: &'static str,
    sidebar_selection: &'static str,
}

struct System {
    red: &'static str,
    green: &'static str,
    blue: &'static str,
    yellow: &'static str,
    purple: &'static str,
    cyan: &'static str,
}

pub fn macos(dark: bool, accent: Option<Accent>) -> Palette {
    let (neutrals, system, default_accent) = if dark {
        (
            Neutrals {
                window: "#1e1e1eff",
                sidebar: "#262626ff",
                raised: "#ffffff08",
                control: "#ffffff14",
                hover: "#ffffff0d",
                border: "#ffffff1a",
                text: "#ffffffff",
                muted: "#ffffff8c",
                faint: "#ffffff40",
                sidebar_selection: "#ffffff1c",
            },
            System {
                red: "#ff4245ff",
                green: "#30d158ff",
                blue: "#0091ffff",
                yellow: "#ffd600ff",
                purple: "#db34f2ff",
                cyan: "#3cd3feff",
            },
            0x0091ff,
        )
    } else {
        (
            Neutrals {
                window: "#ffffffff",
                sidebar: "#f2f2f2ff",
                raised: "#00000008",
                control: "#00000014",
                hover: "#0000000d",
                border: "#3c3c434a",
                text: "#000000d9",
                muted: "#00000080",
                faint: "#00000040",
                sidebar_selection: "#0000001c",
            },
            System {
                red: "#ff383cff",
                green: "#34c759ff",
                blue: "#0088ffff",
                yellow: "#ffcc00ff",
                purple: "#cb30e0ff",
                cyan: "#00c0e8ff",
            },
            0x0088ff,
        )
    };
    build(
        "macos",
        dark,
        &neutrals,
        &system,
        accent.unwrap_or(default_accent),
    )
}

pub fn windows(dark: bool, accent: Option<Accent>) -> Palette {
    let (neutrals, default_accent) = if dark {
        (
            Neutrals {
                window: "#272727ff",
                sidebar: "#202020ff",
                raised: "#ffffff0b",
                control: "#ffffff0f",
                hover: "#ffffff0f",
                border: "#ffffff15",
                text: "#ffffffff",
                muted: "#ffffffc5",
                faint: "#ffffff87",
                sidebar_selection: "#ffffff0f",
            },
            0x60cdff,
        )
    } else {
        (
            Neutrals {
                window: "#fbfbfbff",
                sidebar: "#f3f3f3ff",
                raised: "#00000005",
                control: "#0000000a",
                hover: "#00000009",
                border: "#0000000f",
                text: "#000000e4",
                muted: "#0000009e",
                faint: "#00000072",
                sidebar_selection: "#00000009",
            },
            0x005fb8,
        )
    };
    let system = if dark {
        System {
            red: "#ff99a4ff",
            green: "#6ccb5fff",
            blue: "#60cdffff",
            yellow: "#fce100ff",
            purple: "#d59dffff",
            cyan: "#5fd4e5ff",
        }
    } else {
        System {
            red: "#c42b1cff",
            green: "#0f7b0fff",
            blue: "#005fb8ff",
            yellow: "#9d5d00ff",
            purple: "#8764b8ff",
            cyan: "#0078a0ff",
        }
    };
    build(
        "windows",
        dark,
        &neutrals,
        &system,
        accent.unwrap_or(default_accent),
    )
}

/// Composite a translucent #RRGGBBAA color over an opaque surface. Baseline
/// palettes keep text and borders opaque; offscreen card faces and overlapping
/// primitives rely on that, so native tokens are flattened the same way.
fn over(top: &str, surface: &str) -> String {
    let channels = |value: &str| {
        let v = u32::from_str_radix(&value[1..], 16).expect("static palette color");
        [
            (v >> 24) & 0xff,
            (v >> 16) & 0xff,
            (v >> 8) & 0xff,
            v & 0xff,
        ]
    };
    let [r, g, b, a] = channels(top);
    let [sr, sg, sb, _] = channels(surface);
    let mix = |c: u32, s: u32| (c * a + s * (255 - a) + 127) / 255;
    format!("#{:02x}{:02x}{:02x}ff", mix(r, sr), mix(g, sg), mix(b, sb))
}

fn hex(rgb: Accent, alpha: u8) -> String {
    format!("#{:06x}{alpha:02x}", rgb & 0x00ff_ffff)
}

fn build(id: &str, dark: bool, n: &Neutrals, s: &System, accent: Accent) -> Palette {
    let mut content = BTreeMap::new();
    let mut set = |key: &str, value: String| {
        content.insert(key.to_owned(), value);
    };
    set("background-primary", n.window.into());
    set("background-primary-alt", n.raised.into());
    set("background-secondary", n.sidebar.into());
    set("background-secondary-alt", n.sidebar.into());
    set("background-modifier-border", over(n.border, n.window));
    set("background-modifier-border-hover", over(n.border, n.window));
    set("background-modifier-border-focus", hex(accent, 0xff));
    set("background-modifier-hover", n.hover.into());
    set("interactive-normal", n.control.into());
    set("interactive-hover", n.hover.into());
    set("interactive-accent", hex(accent, 0xff));
    set("interactive-accent-hover", hex(accent, 0xe6));
    set("text-normal", over(n.text, n.window));
    set("text-muted", over(n.muted, n.window));
    set("text-faint", over(n.faint, n.window));
    set("text-on-accent", "#ffffffff".into());
    set("text-selection", hex(accent, 0x40));
    set("color-red", s.red.into());
    set("color-green", s.green.into());
    set("color-blue", s.blue.into());
    set("color-yellow", s.yellow.into());
    set("color-purple", s.purple.into());
    set("color-cyan", s.cyan.into());
    let mut chrome = content.clone();
    // Native sidebars select with a neutral fill; the accent stays on icons.
    chrome.insert("text-selection".into(), n.sidebar_selection.into());
    for (key, value) in [
        ("background-modifier-border", n.border),
        ("text-normal", n.text),
        ("text-muted", n.muted),
        ("text-faint", n.faint),
    ] {
        chrome.insert(key.into(), over(value, n.sidebar));
    }
    Palette {
        id: id.into(),
        mode: if dark { "dark" } else { "light" }.into(),
        contrast: "normal".into(),
        scheme_accent: false,
        content,
        chrome,
    }
}
