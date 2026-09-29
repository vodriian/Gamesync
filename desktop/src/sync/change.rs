//! The unit of sync: one value for one field of one target.

use super::Stamp;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, str::FromStr};
use uuid::Uuid;

/// Longest field path, status key, secret name, or unknown target text.
const MAX_NAME_LEN: usize = 128;
/// A resolution lists every head it replaces; more than this is not credible.
const MAX_BASE_LEN: usize = 64;

/// What a change applies to. Stored as text such as `steam:620`.
///
/// Local game IDs differ on each device, so a Steam game is known by its
/// Steam App ID. Only manual games use a shared UUID.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    Steam(u32),
    Game(Uuid),
    Status(String),
    Collection(Uuid),
    Library,
    Settings,
    Secret(String),
    /// A kind from a newer app version. It is kept and passed on unchanged.
    Other(String),
}

/// Lowercase ASCII letters, digits, `_`, `-`, and `.`, as used by status keys
/// and field paths. This keeps targets safe in file names and JSON keys.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
        })
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let (kind, id) = match text.split_once(':') {
            Some((kind, id)) => (kind, Some(id)),
            None => (text, None),
        };
        let target = match (kind, id) {
            ("steam", Some(id)) => Target::Steam(
                id.parse()
                    .ok()
                    .filter(|id| *id != 0)
                    .context("Invalid Steam App ID")?,
            ),
            ("game", Some(id)) => Target::Game(id.parse().context("Invalid game ID")?),
            ("collection", Some(id)) => {
                Target::Collection(id.parse().context("Invalid collection ID")?)
            }
            ("status", Some(key)) if valid_name(key) && !key.contains('.') => {
                Target::Status(key.into())
            }
            ("secret", Some(name)) if valid_name(name) => Target::Secret(name.into()),
            ("library", None) => Target::Library,
            ("settings", None) => Target::Settings,
            ("steam" | "game" | "collection" | "status" | "secret" | "library" | "settings", _) => {
                bail!("Invalid sync target: {text}")
            }
            _ if valid_name(kind) && id.is_none_or(valid_name) => Target::Other(text.into()),
            _ => bail!("Invalid sync target: {text}"),
        };
        Ok(target)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Steam(id) => write!(f, "steam:{id}"),
            Target::Game(id) => write!(f, "game:{id}"),
            Target::Status(key) => write!(f, "status:{key}"),
            Target::Collection(id) => write!(f, "collection:{id}"),
            Target::Library => f.write_str("library"),
            Target::Settings => f.write_str("settings"),
            Target::Secret(name) => write!(f, "secret:{name}"),
            Target::Other(text) => f.write_str(text),
        }
    }
}

impl Serialize for Target {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// One edit on one device. Changes are immutable once written.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Change {
    pub id: Uuid,
    pub at: Stamp,
    pub target: Target,
    /// A dotted path such as `personal.rating`. Any field outside Steam data
    /// can sync, including fields this version does not know.
    pub field: String,
    /// The new value. `null` clears the field. The type is not checked here,
    /// because the field can be one this version does not know; the code that
    /// applies a known field checks its type.
    pub value: serde_json::Value,
    /// The changes to this field that the edit replaced. Empty for a first value.
    #[serde(default)]
    pub base: Vec<Uuid>,
    /// Properties from a newer format, kept so they survive a round trip.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Change {
    /// Reject a change that a well-behaved device cannot produce. Changes come
    /// from other devices and cloud folders, so they are not trusted.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            valid_name(&self.field)
                && !self.field.starts_with('.')
                && !self.field.ends_with('.')
                && !self.field.contains(".."),
            "Invalid sync field: {}",
            self.field
        );
        ensure!(
            self.base.len() <= MAX_BASE_LEN,
            "Sync change {} replaces too many changes",
            self.id
        );
        ensure!(
            !self.base.contains(&self.id),
            "Sync change {} replaces itself",
            self.id
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn targets_round_trip_as_text() {
        let id = Uuid::from_u128(7);
        for target in [
            Target::Steam(620),
            Target::Game(id),
            Target::Status("want-to-play".into()),
            Target::Collection(id),
            Target::Library,
            Target::Settings,
            Target::Secret("ai.openai".into()),
            Target::Other("view:recent".into()),
        ] {
            let text = target.to_string();
            assert_eq!(text.parse::<Target>().unwrap(), target, "{text}");
        }
    }

    #[test]
    fn invalid_targets_are_rejected() {
        for text in [
            "",
            "steam:0",
            "steam:abc",
            "game:1",
            "status:",
            "status:Backlog",
            "status:a.b",
            "library:x",
            "secret:",
            "Other",
            "view:a/b",
        ] {
            assert!(text.parse::<Target>().is_err(), "{text}");
        }
    }

    #[test]
    fn change_keeps_unknown_properties() {
        let text = json!({
            "id": Uuid::from_u128(1),
            "at": {"ms": 5, "n": 0, "device": Uuid::from_u128(2)},
            "target": "steam:620",
            "field": "personal.rating",
            "value": 8,
            "base": [],
            "signed_by": "future"
        });
        let change: Change = serde_json::from_value(text.clone()).unwrap();
        change.validate().unwrap();
        assert_eq!(serde_json::to_value(&change).unwrap(), text);
    }

    #[test]
    fn invalid_fields_and_bases_are_rejected() {
        let change = |field: &str, base: Vec<Uuid>| Change {
            id: Uuid::from_u128(1),
            at: Stamp {
                ms: 1,
                n: 0,
                device: Uuid::from_u128(2),
            },
            target: Target::Library,
            field: field.into(),
            value: json!(null),
            base,
            extra: BTreeMap::new(),
        };
        assert!(change("name", vec![]).validate().is_ok());
        for field in ["", ".name", "name.", "a..b", "Name", "a/b"] {
            assert!(change(field, vec![]).validate().is_err(), "{field}");
        }
        assert!(change("name", vec![Uuid::from_u128(1)]).validate().is_err());
        assert!(change("name", vec![Uuid::from_u128(3); 65])
            .validate()
            .is_err());
    }
}
