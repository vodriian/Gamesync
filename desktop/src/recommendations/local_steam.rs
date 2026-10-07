//! Device-local installation evidence. Steam manifests are read on a worker;
//! ownership and Steam Deck availability are never inferred from an install.
use anyhow::{ensure, Context as _, Result};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Default, Clone)]
pub struct Installations {
    pub apps: BTreeSet<u32>,
    pub scanned: bool,
    pub on_deck: bool,
}
impl Installations {
    pub fn for_target(&self, app: u32, device: super::Device) -> Option<bool> {
        (self.scanned && self.on_deck == (device == super::Device::SteamDeck))
            .then(|| self.apps.contains(&app))
    }
}
fn read(path: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}
/// VDF quoted tokens, including escaped paths. Bare braces delimit sections.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut token = String::new();
        while let Some(c) = chars.next() {
            if c == '"' {
                break;
            }
            if c == '\\' {
                if let Some(next) = chars.next() {
                    token.push(next);
                }
            } else {
                token.push(c);
            }
        }
        out.push(token);
    }
    out
}
fn value<'a>(tokens: &'a [String], key: &str) -> Option<&'a str> {
    tokens
        .windows(2)
        .find(|p| p[0].eq_ignore_ascii_case(key))
        .map(|p| p[1].as_str())
}

pub fn scan_roots(roots: &[PathBuf]) -> Installations {
    let mut result = Installations::default();
    let mut folders = BTreeSet::new();
    for root in roots {
        if root.join("steamapps").is_dir() {
            folders.insert(root.join("steamapps"));
        }
        if let Some(vdf) = read(&root.join("steamapps/libraryfolders.vdf")) {
            for pair in tokens(&vdf).windows(2) {
                if pair[0] == "path" {
                    let path = PathBuf::from(&pair[1]);
                    if path.is_absolute() {
                        folders.insert(path.join("steamapps"));
                    }
                }
            }
        }
    }
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        result.scanned = true;
        for entry in entries.flatten().take(20_000) {
            let path = entry.path();
            let Some(app_id) = path
                .file_name()
                .and_then(|s| s.to_str())
                .and_then(|s| {
                    s.strip_prefix("appmanifest_")?
                        .strip_suffix(".acf")?
                        .parse::<u32>()
                        .ok()
                })
                .filter(|id| *id > 0)
            else {
                continue;
            };
            let Some(text) = read(&path) else {
                continue;
            };
            let tokens = tokens(&text);
            let parsed_id = value(&tokens, "appid").and_then(|s| s.parse::<u32>().ok());
            let flags = value(&tokens, "StateFlags")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let directory = value(&tokens, "installdir")
                .filter(|s| !s.is_empty() && !s.contains(['/', '\\']) && *s != "." && *s != "..");
            if parsed_id == Some(app_id)
                && flags & 4 != 0
                && directory.is_some_and(|d| folder.join("common").join(d).is_dir())
            {
                result.apps.insert(app_id);
            }
        }
    }
    result
}
pub fn scan() -> Installations {
    let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_owned()) else {
        return Installations::default();
    };
    let roots = if cfg!(target_os = "macos") {
        vec![home.join("Library/Application Support/Steam")]
    } else {
        vec![
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ]
    };
    let mut result = scan_roots(&roots);
    result.on_deck = cfg!(target_os = "linux")
        && read(Path::new("/sys/devices/virtual/dmi/id/product_name"))
            .is_some_and(|name| matches!(name.trim(), "Jupiter" | "Galileo"));
    result
}

/// Success means the OS accepted the URI. It does not prove that Steam started
/// the game or that the user played it. Never pass a title or external URL here.
pub fn request_launch(app_id: u32) -> Result<()> {
    ensure!(app_id > 0, "Invalid Steam App ID");
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "linux") {
        "xdg-open"
    } else {
        anyhow::bail!("Steam launch is not supported on this platform")
    };
    let output = std::process::Command::new(program)
        .arg(format!("steam://rungameid/{app_id}"))
        .output()
        .context("Could not ask the system to open Steam")?;
    ensure!(
        output.status.success(),
        "The system could not open Steam. Check that the Steam client is installed, then retry."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifests_require_installed_state_matching_id_and_directory() {
        let root = tempfile::tempdir().unwrap();
        let steamapps = root.path().join("steamapps");
        std::fs::create_dir_all(steamapps.join("common/Example")).unwrap();
        std::fs::write(
            steamapps.join("appmanifest_42.acf"),
            "\"AppState\" { \"appid\" \"42\" \"StateFlags\" \"4\" \"installdir\" \"Example\" }",
        )
        .unwrap();
        std::fs::write(
            steamapps.join("appmanifest_43.acf"),
            "\"appid\" \"43\" \"StateFlags\" \"2\" \"installdir\" \"Example\"",
        )
        .unwrap();
        let result = scan_roots(&[root.path().to_owned()]);
        assert!(result.scanned);
        assert_eq!(result.apps, BTreeSet::from([42]));
        assert_eq!(result.for_target(42, super::super::Device::SteamDeck), None);
    }
}
