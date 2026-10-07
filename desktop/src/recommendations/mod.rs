//! Offline recommendation contracts. Personal corrections, cached analysis, and
//! local estimates remain separate; the selector has no disk, network, or clock.
pub mod local_steam;
mod playing;
mod profile;
pub use playing::*;
mod selector;
pub use profile::*;
pub use selector::*;

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
}
impl Effort {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Shoot,
    Fly,
    Drive,
    Explore,
    Horse,
    Cards,
    Multiplayer,
    Runs,
    Weird,
}
impl Activity {
    pub const ALL: [Self; 9] = [
        Self::Shoot,
        Self::Fly,
        Self::Drive,
        Self::Explore,
        Self::Horse,
        Self::Cards,
        Self::Multiplayer,
        Self::Runs,
        Self::Weird,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Shoot => "Shoot",
            Self::Fly => "Fly",
            Self::Drive => "Drive",
            Self::Explore => "Explore",
            Self::Horse => "Ride a horse",
            Self::Cards => "Play cards",
            Self::Multiplayer => "Multiplayer",
            Self::Runs => "Never-ending",
            Self::Weird => "Weird",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stopping {
    Flexible,
    Checkpoints,
    LongSession,
}
impl Stopping {
    pub const ALL: [Self; 3] = [Self::Flexible, Self::Checkpoints, Self::LongSession];
    pub fn label(self) -> &'static str {
        match self {
            Self::Flexible => "Any time",
            Self::Checkpoints => "At checkpoints",
            Self::LongSession => "Long session",
        }
    }
}

/// None is unknown (or automatic for a personal field). Some(empty) is an
/// intentional activity correction and must not fall back to AI or tag rules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub mechanical: Option<Effort>,
    pub cognitive: Option<Effort>,
    pub narrative: Option<Effort>,
    pub onboarding: Option<Effort>,
    pub minimum_minutes: Option<u16>,
    pub ideal_minutes: Option<u16>,
    pub setup_minutes: Option<u16>,
    pub stopping: Option<Stopping>,
    pub activities: Option<Vec<Activity>>,
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.minimum_minutes.is_none_or(|v| (1..=1440).contains(&v)),
            "Minimum session must be 1–1440 minutes"
        );
        ensure!(
            self.ideal_minutes.is_none_or(|v| (1..=1440).contains(&v)),
            "Ideal session must be 1–1440 minutes"
        );
        ensure!(
            self.setup_minutes.is_none_or(|v| v <= 120),
            "Startup must be 0–120 minutes"
        );
        if let (Some(min), Some(ideal)) = (self.minimum_minutes, self.ideal_minutes) {
            ensure!(
                min <= ideal,
                "Ideal session must not be shorter than minimum"
            );
        }
        if let Some(activities) = &self.activities {
            ensure!(
                activities.len()
                    == activities
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                "Activities must be unique"
            );
        }
        Ok(())
    }
    pub fn energy(&self) -> Option<Effort> {
        Some(self.mechanical?.max(self.cognitive?))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Device {
    #[default]
    Pc,
    SteamDeck,
}
impl Device {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pc => "PC",
            Self::SteamDeck => "Steam Deck",
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Any,
    Unplayed,
    Playing,
}
impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "All eligible games",
            Self::Unplayed => "Unplayed",
            Self::Playing => "Playing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    /// None explicitly means no time limit.
    pub minutes: Option<u16>,
    pub energy: Effort,
    pub activity: Option<Activity>,
    pub brain_dead: bool,
    pub device: Device,
    pub installed_only: bool,
    pub favorites_only: bool,
    pub scope: Scope,
}
impl Default for Context {
    fn default() -> Self {
        Self {
            minutes: Some(30),
            energy: Effort::Low,
            activity: None,
            brain_dead: false,
            device: Device::Pc,
            installed_only: false,
            favorites_only: false,
            scope: Scope::Any,
        }
    }
}
impl Context {
    pub fn time_label(&self) -> String {
        match self.minutes {
            None => "No time limit".into(),
            Some(60) => "1 hour".into(),
            Some(120) => "2 hours".into(),
            Some(n) => format!("{n} min"),
        }
    }
}

/// OS handoff is not evidence that Steam or a game actually started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchOutcome {
    #[default]
    NotRequested,
    Accepted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub at: i64,
    pub session: Uuid,
    pub context: Context,
    /// Choosing is separate from asking the operating system to open Steam.
    #[serde(default)]
    pub launch: LaunchOutcome,
    /// Explicit session end; never merged into Steam playtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersonalRecommendations {
    pub profile: Profile,
    pub saved: bool,
    pub excluded: bool,
    /// Bounded explicit choices, not inferred play sessions or generated hands.
    pub recent: Vec<Choice>,
}
impl PersonalRecommendations {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        ensure!(self.recent.len() <= 20, "Too many recent choices");
        ensure!(
            self.recent.iter().all(|c| c.at >= 0
                && !c.session.is_nil()
                && c.finished_at.is_none_or(|end| end >= c.at)
                && c.context.minutes.is_none_or(|m| m > 0 && m <= 1440)),
            "Invalid choice history"
        );
        Ok(())
    }
    pub fn record_choice(&mut self, choice: Choice) {
        // Launch retries and Done playing update the same explicit session.
        self.recent
            .retain(|existing| existing.session != choice.session);
        self.recent.insert(0, choice);
        self.recent.truncate(20);
    }
}

/// Future enrichment writes a result by stable game ID, never batch position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub game_id: Uuid,
    pub version: u32,
    pub provider: String,
    pub model: String,
    pub fingerprint: String,
    pub analyzed_at: i64,
    pub confidence: u8,
    pub profile: Profile,
}
impl Analysis {
    pub fn validate(&self, game_id: Uuid) -> Result<()> {
        ensure!(
            self.game_id == game_id && !game_id.is_nil(),
            "Analysis game ID does not match"
        );
        ensure!(
            self.version == PROFILE_VERSION,
            "Unsupported recommendation profile version"
        );
        ensure!(
            !self.provider.is_empty()
                && !self.model.is_empty()
                && self.provider.len() <= 100
                && self.model.len() <= 200,
            "Invalid analysis source"
        );
        ensure!(
            self.fingerprint.len() == 64 && self.fingerprint.bytes().all(|c| c.is_ascii_hexdigit()),
            "Invalid analysis fingerprint"
        );
        ensure!(
            self.confidence <= 100 && self.analyzed_at >= 0,
            "Invalid analysis confidence or date"
        );
        self.profile.validate()
    }
}
