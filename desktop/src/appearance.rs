//! Device-local appearance and bundled, resolved Baseline palettes.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    #[default]
    Auto,
    Light,
    Dark,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightContrast {
    #[default]
    Normal,
    Tonal,
    Vivid,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DarkContrast {
    #[default]
    Normal,
    Tonal,
    Black,
}
/// Stable catalog identity. Unknown future IDs round-trip and resolve to a safe default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemeId(String);
impl SchemeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl From<&str> for SchemeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
impl From<String> for SchemeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub mode: AppearanceMode,
    pub light_scheme: SchemeId,
    pub dark_scheme: SchemeId,
    pub light_contrast: LightContrast,
    pub dark_contrast: DarkContrast,
    pub interface_accent: bool,
    pub scheme_accent: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::Auto,
            light_scheme: "sanctum".into(),
            dark_scheme: "notion".into(),
            light_contrast: LightContrast::Normal,
            dark_contrast: DarkContrast::Normal,
            interface_accent: true,
            scheme_accent: true,
        }
    }
}
impl Appearance {
    pub fn from_legacy(choice: &str) -> Self {
        let mut result = Self::default();
        let (mode, id) = match choice.to_lowercase().as_str() {
            "light" => (AppearanceMode::Light, "sanctum"),
            "dark" => (AppearanceMode::Dark, "notion"),
            "catppuccin latte" => (AppearanceMode::Light, "latte"),
            "solarized light" => (AppearanceMode::Light, "solarized"),
            "catppuccin mocha" => (AppearanceMode::Dark, "mocha"),
            "dracula" => (AppearanceMode::Dark, "dracula"),
            "gruvbox dark" => (AppearanceMode::Dark, "gruvbox"),
            "nord" => (AppearanceMode::Dark, "nord"),
            "rosé pine" | "rose pine" => (AppearanceMode::Dark, "rose-pine"),
            "tokyo night" => (AppearanceMode::Dark, "notion"),
            _ => (AppearanceMode::Auto, ""),
        };
        result.mode = mode;
        match mode {
            AppearanceMode::Light => result.light_scheme = id.into(),
            AppearanceMode::Dark => result.dark_scheme = id.into(),
            _ => {}
        }
        result
    }
    pub fn is_dark(&self, system_dark: bool) -> bool {
        match self.mode {
            AppearanceMode::Auto => system_dark,
            AppearanceMode::Light => false,
            AppearanceMode::Dark => true,
        }
    }
    pub fn palette(&self, system_dark: bool) -> &'static Palette {
        let dark = self.is_dark(system_dark);
        let mode = if dark { "dark" } else { "light" };
        let id = if dark {
            &self.dark_scheme
        } else {
            &self.light_scheme
        };
        let contrast = if dark {
            match self.dark_contrast {
                DarkContrast::Normal => "normal",
                DarkContrast::Tonal => "tonal",
                DarkContrast::Black => "black",
            }
        } else {
            match self.light_contrast {
                LightContrast::Normal => "normal",
                LightContrast::Tonal => "tonal",
                LightContrast::Vivid => "vivid",
            }
        };
        let supported = catalog()
            .schemes
            .iter()
            .any(|s| s.id == id.as_str() && s.modes.iter().any(|m| m == mode));
        let id = if supported {
            id.as_str()
        } else if dark {
            "notion"
        } else {
            "sanctum"
        };
        catalog()
            .palettes
            .iter()
            .find(|p| {
                p.id == id
                    && p.mode == mode
                    && p.contrast == contrast
                    && p.scheme_accent == self.scheme_accent
            })
            .expect("validated bundled palette")
    }
}
#[derive(Debug, Deserialize)]
pub struct Scheme {
    pub id: String,
    pub name: String,
    pub modes: Vec<String>,
}
#[derive(Debug, Deserialize)]
pub struct Palette {
    pub id: String,
    pub mode: String,
    pub contrast: String,
    pub scheme_accent: bool,
    pub content: BTreeMap<String, String>,
    pub chrome: BTreeMap<String, String>,
}
#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub revision: String,
    pub schemes: Vec<Scheme>,
    pub palettes: Vec<Palette>,
}
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("themes/baseline.json"))
            .expect("validated bundled Baseline data")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_covers_every_supported_combination() {
        let c = catalog();
        assert_eq!(c.revision, "8c56e831e1abb1d3841c4ffdecbe06b5182fbc68");
        assert_eq!(c.schemes.len(), 25);
        assert!(c.schemes.iter().all(|s| s.id != "default"));
        let mut seen = std::collections::BTreeSet::new();
        for p in &c.palettes {
            assert!(seen.insert((&p.id, &p.mode, &p.contrast, p.scheme_accent)));
            assert_eq!(p.content.len(), 26);
            assert_eq!(p.chrome.len(), 26);
            for value in p.content.values().chain(p.chrome.values()) {
                assert_eq!(value.len(), 9);
                assert!(u32::from_str_radix(&value[1..], 16).is_ok());
            }
        }
        for s in &c.schemes {
            for mode in &s.modes {
                assert_eq!(
                    c.palettes
                        .iter()
                        .filter(|p| p.id == s.id && p.mode == *mode)
                        .count(),
                    6
                );
            }
        }
        assert!(!c
            .schemes
            .iter()
            .find(|s| s.id == "dracula")
            .unwrap()
            .modes
            .contains(&"light".into()));
    }
    #[test]
    fn defaults_and_system_changes_preserve_selections() {
        let a = Appearance::default();
        assert_eq!(a.palette(false).id, "sanctum");
        assert_eq!(a.palette(true).id, "notion");
        assert_eq!(a.palette(false).content["background-primary"], "#fdfefeff");
        assert_eq!(a.palette(true).content["background-primary"], "#191919ff");
        let fixed = Appearance {
            mode: AppearanceMode::Light,
            ..a
        };
        assert_eq!(fixed.palette(true).id, "sanctum");
    }
    #[test]
    fn contrast_changes_surfaces_and_preserves_text() {
        let mut a = Appearance {
            light_contrast: LightContrast::Tonal,
            ..Default::default()
        };
        let p = a.palette(false);
        assert_eq!(
            p.content["background-primary"],
            p.content["background-secondary"]
        );
        a.light_contrast = LightContrast::Vivid;
        let p = a.palette(false);
        assert_eq!(p.content["background-primary"], "#fdfefeff");
        assert_eq!(p.chrome["background-secondary"], "#262625ff");
        assert_eq!(p.chrome["text-normal"], "#f4f4f0ff");
        a.dark_contrast = DarkContrast::Black;
        let p = a.palette(true);
        assert_eq!(p.content["background-primary"], "#000000ff");
        assert_eq!(p.chrome["background-secondary"], "#000000ff");
    }
    #[test]
    fn legacy_names_keep_modes_and_choose_equivalents() {
        for (old, id, mode) in [
            ("system", "sanctum", AppearanceMode::Auto),
            ("light", "sanctum", AppearanceMode::Light),
            ("dark", "notion", AppearanceMode::Dark),
            ("Catppuccin Latte", "latte", AppearanceMode::Light),
            ("Catppuccin Mocha", "mocha", AppearanceMode::Dark),
            ("Dracula", "dracula", AppearanceMode::Dark),
            ("Gruvbox Dark", "gruvbox", AppearanceMode::Dark),
            ("Nord", "nord", AppearanceMode::Dark),
            ("Rosé Pine", "rose-pine", AppearanceMode::Dark),
            ("Solarized Light", "solarized", AppearanceMode::Light),
            ("Tokyo Night", "notion", AppearanceMode::Dark),
        ] {
            let a = Appearance::from_legacy(old);
            assert_eq!(a.mode, mode);
            assert_eq!(a.palette(false).id, id);
        }
    }
    #[test]
    fn unsupported_mode_and_unknown_scheme_fall_back() {
        let a = Appearance {
            light_scheme: "dracula".into(),
            dark_scheme: "missing".into(),
            ..Default::default()
        };
        assert_eq!(a.palette(false).id, "sanctum");
        assert_eq!(a.palette(true).id, "notion");
    }
    #[test]
    fn disabling_scheme_accent_uses_shared_blue() {
        let a = Appearance::default();
        let mut b = a.clone();
        b.scheme_accent = false;
        assert_ne!(
            a.palette(false).content["interactive-accent"],
            b.palette(false).content["interactive-accent"]
        );
        assert_eq!(
            b.palette(false).content["interactive-accent"],
            b.palette(true).content["interactive-accent"]
        );
    }
}
