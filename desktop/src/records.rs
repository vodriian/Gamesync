//! Portable game records. Provider values never replace personal overrides.

use std::collections::BTreeMap;

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

/// Preserve unfamiliar fields when editing a supported schema version.
pub type ExtraFields = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameRevision {
    pub schema_version: u32,
    pub game_id: Uuid,
    pub revision_id: Uuid,
    pub parents: Vec<Uuid>,
    /// An explicit tombstone. Missing files never imply deletion.
    pub deleted: bool,
    pub game: GameData,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameData {
    pub title: String,
    pub steam: Option<SteamData>,
    pub personal: PersonalData,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteamData {
    pub app_id: u32,
    pub description: Option<String>,
    pub playtime_minutes: u32,
    pub owned: bool,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalData {
    /// Stable status key; a label can change without changing this value.
    pub status: String,
    /// Half-star units, 1 through 10. None means unrated.
    pub rating: Option<u8>,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub notes: String,
    /// None uses provider text. Some("") is an intentional blank override.
    pub description: Option<String>,
    /// Portable path within media/, never an absolute device path.
    pub cover: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Default for PersonalData {
    fn default() -> Self {
        Self {
            status: "backlog".into(),
            rating: None,
            favorite: false,
            tags: Vec::new(),
            notes: String::new(),
            description: None,
            cover: None,
            extra: ExtraFields::new(),
        }
    }
}

impl GameData {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            steam: None,
            personal: PersonalData::default(),
            extra: ExtraFields::new(),
        }
    }

    pub fn description(&self) -> Option<&str> {
        self.personal.description.as_deref().or_else(|| {
            self.steam
                .as_ref()
                .and_then(|steam| steam.description.as_deref())
        })
    }
}

impl GameRevision {
    pub fn validate(&self) -> Result<()> {
        check_extra(
            &self.extra,
            &[
                "schema_version",
                "game_id",
                "revision_id",
                "parents",
                "deleted",
                "game",
            ],
        )?;
        check_extra(&self.game.extra, &["title", "steam", "personal"])?;
        check_extra(
            &self.game.personal.extra,
            &[
                "status",
                "rating",
                "favorite",
                "tags",
                "notes",
                "description",
                "cover",
            ],
        )?;
        if let Some(steam) = &self.game.steam {
            check_extra(
                &steam.extra,
                &["app_id", "description", "playtime_minutes", "owned"],
            )?;
        }
        ensure!(
            self.schema_version == SCHEMA_VERSION,
            "Unsupported schema version {}",
            self.schema_version
        );
        ensure!(
            !self.game_id.is_nil() && !self.revision_id.is_nil(),
            "Record IDs must not be nil"
        );
        let parents: std::collections::BTreeSet<_> = self.parents.iter().collect();
        ensure!(
            parents.len() == self.parents.len(),
            "Duplicate revision parent"
        );
        ensure!(
            self.parents
                .iter()
                .all(|id| !id.is_nil() && *id != self.revision_id),
            "Invalid revision parent"
        );
        ensure!(!self.game.title.trim().is_empty(), "Game title is empty");
        ensure!(
            !self.game.personal.status.trim().is_empty(),
            "Game status is empty"
        );
        ensure!(
            self.game
                .personal
                .rating
                .is_none_or(|rating| (1..=10).contains(&rating)),
            "Rating must be from 0.5 to 5 stars"
        );
        ensure!(
            self.game
                .steam
                .as_ref()
                .is_none_or(|steam| steam.app_id != 0),
            "Steam App ID must be positive"
        );
        if let Some(cover) = &self.game.personal.cover {
            // Check portable separators, including Windows paths on a Mac.
            ensure!(
                !cover.contains(['\\', ':'])
                    && cover.starts_with("media/")
                    && cover
                        .split('/')
                        .all(|part| !part.is_empty() && part != "." && part != ".."),
                "Cover must be a relative path inside media/"
            );
        }
        Ok(())
    }
}

pub(crate) fn check_extra(extra: &ExtraFields, reserved: &[&str]) -> Result<()> {
    ensure!(
        extra.keys().all(|key| !reserved.contains(&key.as_str())),
        "Extension fields must not replace known fields"
    );
    Ok(())
}

impl crate::revision_store::Revision for GameRevision {
    fn entity_id(&self) -> Uuid {
        self.game_id
    }
    fn revision_id(&self) -> Uuid {
        self.revision_id
    }
    fn parents(&self) -> &[Uuid] {
        &self.parents
    }
    fn set_revision(&mut self, id: Uuid, parents: Vec<Uuid>) {
        self.revision_id = id;
        self.parents = parents;
    }
    fn validate(&self) -> Result<()> {
        GameRevision::validate(self)
    }
}
