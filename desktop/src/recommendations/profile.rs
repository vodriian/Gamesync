use super::{Activity, Effort, Profile, Stopping};
use crate::records::GameData;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PROFILE_VERSION: u32 = 1;

pub fn fingerprint(game: &GameData) -> String {
    // No personal notes or feedback: those are not game-behavior evidence.
    let metadata = game.steam.as_ref().and_then(|s| s.metadata.as_ref());
    let data = serde_json::json!([
        game.title,
        game.steam.as_ref().and_then(|s| s.description.as_ref()),
        metadata.map(|m| &m.tags),
        metadata.map(|m| &m.genres)
    ]);
    format!("{:x}", Sha256::digest(data.to_string().as_bytes()))
}

#[derive(Debug, Clone)]
pub struct EffectiveProfile {
    pub values: Profile,
    pub sources: BTreeMap<&'static str, &'static str>,
    pub evidence: Vec<String>,
}
impl EffectiveProfile {
    pub fn estimated(&self) -> bool {
        self.sources.values().any(|s| *s != "Your value")
    }
}

/// Rule estimates are intentionally coarse. Precise activity tags may identify
/// mechanics; a broad genre alone cannot identify horses, flying, or low effort.
pub fn estimate(game: &GameData) -> (Profile, Vec<String>) {
    let tags: BTreeSet<String> = game
        .personal
        .tags
        .iter()
        .chain(game.steam.iter().flat_map(|s| {
            s.metadata
                .iter()
                .flat_map(|m| m.tags.iter().chain(&m.genres))
        }))
        .map(|t| t.to_lowercase())
        .collect();
    let has = |names: &[&str]| names.iter().any(|n| tags.contains(*n));
    let mut activities = Vec::new();
    let mut add = |activity, names: &[&str]| {
        if has(names) {
            activities.push(activity);
        }
    };
    add(
        Activity::Shoot,
        &[
            "fps",
            "third-person shooter",
            "shooter",
            "twin stick shooter",
        ],
    );
    add(Activity::Fly, &["flight", "flight simulation", "space sim"]);
    add(Activity::Drive, &["racing", "driving", "automobile sim"]);
    add(Activity::Explore, &["exploration", "open world"]);
    add(Activity::Horse, &["horses", "horse riding", "equestrian"]);
    add(
        Activity::Cards,
        &[
            "card game",
            "card battler",
            "deckbuilding",
            "deckbuilder",
            "roguelike deckbuilder",
        ],
    );
    add(
        Activity::Multiplayer,
        &["multiplayer", "co-op", "online co-op", "local co-op"],
    );
    add(
        Activity::Runs,
        &[
            "roguelike",
            "roguelite",
            "action roguelike",
            "roguelike deckbuilder",
        ],
    );
    add(Activity::Weird, &["surreal", "experimental", "psychedelic"]);
    let mut p = Profile {
        activities: (!activities.is_empty()).then_some(activities),
        ..Default::default()
    };
    let mut rules = Vec::new();
    if p.activities.is_some() {
        rules.push("Activities estimated from matching tags".into());
    }
    // Highest demand wins when several rules apply. No rule turns a card game
    // into a low-cognition game merely because its controls are simple.
    type EstimateRule = (
        &'static str,
        &'static [&'static str],
        Effort,
        Effort,
        u16,
        u16,
    );
    let ruleset: [EstimateRule; 8] = [
        (
            "Relaxed play",
            &["relaxing", "cozy", "walking simulator"],
            Effort::Low,
            Effort::Low,
            15,
            30,
        ),
        (
            "Puzzles and strategy",
            &["puzzle", "strategy", "turn-based strategy"],
            Effort::Low,
            Effort::High,
            15,
            45,
        ),
        (
            "Card games",
            &[
                "card game",
                "card battler",
                "deckbuilding",
                "roguelike deckbuilder",
            ],
            Effort::Low,
            Effort::High,
            10,
            30,
        ),
        (
            "Driving",
            &["racing", "driving"],
            Effort::Medium,
            Effort::Medium,
            15,
            30,
        ),
        (
            "Action",
            &["action", "shooter", "fps", "platformer"],
            Effort::High,
            Effort::Medium,
            20,
            45,
        ),
        (
            "Repeatable runs",
            &["roguelike", "roguelite", "action roguelike"],
            Effort::High,
            Effort::Medium,
            30,
            60,
        ),
        (
            "Story and exploration",
            &["rpg", "story rich", "open world"],
            Effort::Medium,
            Effort::High,
            30,
            60,
        ),
        (
            "Flying",
            &["flight", "flight simulation", "space sim"],
            Effort::High,
            Effort::High,
            30,
            60,
        ),
    ];
    for (id, names, mechanical, cognitive, minimum, ideal) in ruleset {
        if has(names) {
            p.mechanical = Some(p.mechanical.map_or(mechanical, |v| v.max(mechanical)));
            p.cognitive = Some(p.cognitive.map_or(cognitive, |v| v.max(cognitive)));
            p.minimum_minutes = Some(p.minimum_minutes.unwrap_or(0).max(minimum));
            p.ideal_minutes = Some(p.ideal_minutes.unwrap_or(0).max(ideal));
            p.setup_minutes = Some(3);
            rules.push(format!(
                "{id}: {}",
                names
                    .iter()
                    .filter(|n| tags.contains(**n))
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if has(&["story rich", "rpg"]) {
        p.narrative = Some(Effort::High);
    }
    // Tag rules cannot prove tutorial state or saving flexibility. Brain dead
    // waits for reviewed/manual or cached evidence for those fields.
    if has(&["visual novel"]) {
        p.mechanical = Some(Effort::Low);
        p.cognitive = Some(Effort::Medium);
        p.narrative = Some(Effort::High);
    }
    (p, rules)
}

pub fn resolve(game: &GameData) -> EffectiveProfile {
    let (local, evidence) = estimate(game);
    let ai = game
        .recommendation_analysis
        .as_ref()
        .filter(|a| a.version == PROFILE_VERSION && a.fingerprint == fingerprint(game));
    let manual = &game.personal.play_now.profile;
    let mut out = EffectiveProfile {
        values: Profile::default(),
        sources: BTreeMap::new(),
        evidence,
    };
    macro_rules! field {
        ($field:ident) => {
            if let Some(value) = manual.$field.clone() {
                out.values.$field = Some(value);
                out.sources.insert(stringify!($field), "Your value");
            } else if let Some(value) = ai.and_then(|a| a.profile.$field.clone()) {
                out.values.$field = Some(value);
                out.sources.insert(stringify!($field), "AI estimate");
            } else if let Some(value) = local.$field.clone() {
                out.values.$field = Some(value);
                out.sources.insert(stringify!($field), "Local estimate");
            }
        };
    }
    field!(mechanical);
    field!(cognitive);
    field!(narrative);
    field!(onboarding);
    field!(minimum_minutes);
    field!(ideal_minutes);
    field!(setup_minutes);
    field!(stopping);
    field!(activities);
    // A partial override must not make an inconsistent duration look valid.
    if let (Some(min), Some(ideal)) = (out.values.minimum_minutes, out.values.ideal_minutes) {
        if ideal < min {
            out.values.ideal_minutes = None;
            out.sources.remove("ideal_minutes");
        }
    }
    if !out
        .sources
        .values()
        .any(|source| *source == "Local estimate")
    {
        out.evidence.clear();
    }
    out
}

pub fn brain_dead_ready(p: &Profile) -> bool {
    p.cognitive == Some(Effort::Low)
        && p.narrative == Some(Effort::Low)
        && p.onboarding == Some(Effort::Low)
        && p.stopping == Some(Stopping::Flexible)
        && p.setup_minutes.is_some_and(|m| m <= 3)
}
