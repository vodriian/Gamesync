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
    /// Saved demo assessment. Real games calculate theirs from Steam evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suitability: Option<crate::suitability::Assessment>,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteamData {
    pub app_id: u32,
    pub description: Option<String>,
    pub playtime_minutes: u32,
    pub owned: bool,
    /// Unix seconds reported by Steam. None when Steam reports no play.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<i64>,
    #[serde(default, skip_serializing_if = "PlatformMinutes::is_empty")]
    pub platform_minutes: PlatformMinutes,
    /// Present while the game is on the Steam wishlist, or after it left the
    /// wishlist unbought. Cleared when the game becomes owned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wishlist: Option<WishlistEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SteamMetadata>,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WishlistEntry {
    /// Steam wishlist order; lower is more wanted.
    pub priority: u32,
    /// Unix seconds when the game was added on Steam.
    pub added: i64,
    /// The game left the Steam wishlist without a purchase. The record stays
    /// in the Wishlist scope until the user archives it.
    #[serde(default)]
    pub removed: bool,
}

/// Steam playtime by platform. The sum can be less than `playtime_minutes`
/// because Steam does not assign offline play to a platform.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlatformMinutes {
    pub windows: u32,
    pub mac: u32,
    pub linux: u32,
    pub deck: u32,
}

impl PlatformMinutes {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Provider metadata and stage completion. Failed stages remain eligible for retry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SteamMetadata {
    pub genres: Vec<String>,
    pub release_year: Option<String>,
    pub review_label: Option<String>,
    pub review_percent: Option<u8>,
    pub cover: Option<String>,
    /// Steam store tag names, highest community weight first. Never personal tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub details_complete: bool,
    pub reviews_complete: bool,
    pub cover_complete: bool,
    #[serde(default)]
    pub tags_complete: bool,
    /// Steam Deck and controller evidence. None means not fetched yet, so a
    /// later sync retries it. See `crate::suitability::assessment`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<crate::suitability::SetupEvidence>,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_preference: Option<crate::suitability::SetupPreference>,
    /// Stable status key; a label can change without changing this value.
    pub status: String,
    /// Half-star units, 1 through 10. None means unrated.
    pub rating: Option<u8>,
    pub favorite: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collections: Vec<Uuid>,
    pub tags: Vec<String>,
    pub notes: String,
    /// None uses provider text. Some("") is an intentional blank override.
    pub description: Option<String>,
    /// Portable path within media/, never an absolute device path.
    pub cover: Option<String>,
    /// Manual position inside this status's board column. See `crate::board`.
    /// Invalid values from other writers are ignored, not rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_rank: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Default for PersonalData {
    fn default() -> Self {
        Self {
            setup_preference: None,
            status: "backlog".into(),
            rating: None,
            favorite: false,
            hidden: false,
            collections: Vec::new(),
            tags: Vec::new(),
            notes: String::new(),
            description: None,
            cover: None,
            board_rank: None,
            extra: ExtraFields::new(),
        }
    }
}

impl PersonalData {
    /// A board rank orders a game inside one column, so it resets with the status.
    pub fn set_status(&mut self, status: String) {
        if self.status != status {
            self.board_rank = None;
        }
        self.status = status;
    }
}

impl GameData {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            suitability: None,
            steam: None,
            personal: PersonalData::default(),
            extra: ExtraFields::new(),
        }
    }

    pub fn cover(&self) -> Option<&String> {
        self.personal
            .cover
            .as_ref()
            .or_else(|| self.steam.as_ref()?.metadata.as_ref()?.cover.as_ref())
    }

    /// Wishlisted games live only in the Wishlist scope, apart from the library.
    pub fn wishlisted(&self) -> bool {
        self.steam
            .as_ref()
            .is_some_and(|steam| !steam.owned && steam.wishlist.is_some())
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
        check_extra(
            &self.game.extra,
            &["title", "steam", "personal", "suitability"],
        )?;
        check_extra(
            &self.game.personal.extra,
            &[
                "setup_preference",
                "status",
                "rating",
                "favorite",
                "hidden",
                "collections",
                "tags",
                "notes",
                "description",
                "cover",
                "board_rank",
            ],
        )?;
        if let Some(steam) = &self.game.steam {
            if let Some(metadata) = &steam.metadata {
                check_extra(
                    &metadata.extra,
                    &[
                        "genres",
                        "release_year",
                        "review_label",
                        "review_percent",
                        "cover",
                        "details_complete",
                        "reviews_complete",
                        "cover_complete",
                        "setup",
                    ],
                )?;
                ensure!(
                    metadata.review_percent.is_none_or(|p| p <= 100),
                    "Review percent must be at most 100"
                );
                ensure!(
                    !metadata.cover_complete || metadata.cover.is_some(),
                    "Completed cover stage needs a cover"
                );
                ensure!(
                    !metadata.details_complete || steam.description.is_some(),
                    "Completed details stage needs a description"
                );
            }
            check_extra(
                &steam.extra,
                &[
                    "app_id",
                    "description",
                    "playtime_minutes",
                    "owned",
                    "metadata",
                ],
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
        let collection_ids: std::collections::BTreeSet<_> =
            self.game.personal.collections.iter().collect();
        ensure!(
            collection_ids.len() == self.game.personal.collections.len()
                && collection_ids.iter().all(|id| !id.is_nil()),
            "Invalid collection membership"
        );
        for cover in self.game.personal.cover.iter().chain(
            self.game
                .steam
                .as_ref()
                .and_then(|s| s.metadata.as_ref())
                .and_then(|m| m.cover.as_ref()),
        ) {
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
