//! Read-only bridge to the active Omarchy theme.

use anyhow::{bail, Context, Result};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmarchyTheme {
    pub name: String,
    pub mode: String,
    colors: BTreeMap<String, String>,
}

impl OmarchyTheme {
    pub fn load_active() -> Result<Self> {
        let root = current_dir()?;
        let name = fs::read_to_string(root.join("theme.name"))
            .context("the active Omarchy theme name is unavailable")?
            .trim()
            .to_owned();
        let colors = fs::read_to_string(root.join("theme/colors.toml"))
            .context("the active Omarchy colors are unavailable")?;
        Self::parse(&name, &colors)
    }

    pub fn color(&self, key: &str) -> &str {
        &self.colors[key]
    }

    fn parse(name: &str, source: &str) -> Result<Self> {
        let mut values = BTreeMap::new();
        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|value| value.split_once('"').map(|(value, _)| value))
                .unwrap_or(value);
            values.insert(key.trim().to_owned(), value.to_owned());
        }

        let mode = values
            .remove("mode")
            .context("Omarchy colors do not define a mode")?;
        if mode != "light" && mode != "dark" {
            bail!("Omarchy theme mode must be light or dark");
        }
        for key in [
            "accent",
            "selection",
            "muted",
            "background",
            "dark_background",
            "lighter_background",
            "foreground",
            "dark_foreground",
            "red",
            "yellow",
            "green",
            "cyan",
            "blue",
            "magenta",
        ] {
            let value = values
                .get_mut(key)
                .with_context(|| format!("Omarchy colors do not define {key}"))?;
            *value = normalize_color(value)
                .with_context(|| format!("Omarchy color {key} is invalid"))?;
        }
        Ok(Self {
            name: display_name(name),
            mode,
            colors: values,
        })
    }
}

/// Omarchy replaces the theme directory, so callers watch this stable parent.
pub fn current_dir() -> Result<PathBuf> {
    if !cfg!(target_os = "linux") {
        bail!("Omarchy mode is available on Linux only");
    }
    let state = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .context("the user state directory is unavailable")?;
    let current = state.join("omarchy/current");
    if !current.is_dir() {
        bail!("Omarchy theme state was not found");
    }
    Ok(current)
}

pub fn is_available() -> bool {
    current_dir().is_ok()
}

fn normalize_color(value: &str) -> Result<String> {
    let digits = value.strip_prefix('#').context("expected #RRGGBB")?;
    if digits.len() != 6 && digits.len() != 8 {
        bail!("expected #RRGGBB or #RRGGBBAA");
    }
    u32::from_str_radix(digits, 16).context("expected hexadecimal color")?;
    Ok(if digits.len() == 6 {
        format!("#{digits}ff")
    } else {
        value.to_owned()
    })
}

fn display_name(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLORS: &str = r##"
mode = "dark"
accent = "#89b4fa"
selection = "#45475a"
muted = "#585b70"
background = "#1e1e2e"
dark_background = "#161622"
lighter_background = "#313244"
foreground = "#cdd6f4"
dark_foreground = "#6c7086"
red = "#f38ba8"
yellow = "#f9e2af"
green = "#a6e3a1"
cyan = "#94e2d5"
blue = "#89b4fa"
magenta = "#f5c2e7"
"##;

    #[test]
    fn parses_the_active_color_contract() {
        let theme = OmarchyTheme::parse("catppuccin", COLORS).unwrap();
        assert_eq!(theme.name, "Catppuccin");
        assert_eq!(theme.mode, "dark");
        assert_eq!(theme.color("accent"), "#89b4faff");
    }

    #[test]
    fn rejects_missing_or_invalid_required_colors() {
        assert!(OmarchyTheme::parse("broken", &COLORS.replace("accent =", "missing =")).is_err());
        assert!(OmarchyTheme::parse("broken", &COLORS.replace("#89b4fa", "blue")).is_err());
    }
}
