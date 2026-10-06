//! Setup fit and personal choice stay separate. Prototype scores are supplied
//! by fixtures; this module does not claim to measure hardware performance.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BestOn {
    SteamDeck,
    Pc,
    Both,
    NeedsReview,
}

impl BestOn {
    pub const ALL: [Self; 4] = [Self::SteamDeck, Self::Pc, Self::Both, Self::NeedsReview];

    pub fn label(self) -> &'static str {
        match self {
            Self::SteamDeck => "Steam Deck",
            Self::Pc => "PC",
            Self::Both => "Both",
            Self::NeedsReview => "Needs review",
        }
    }
}

/// None in personal data means Automatic. A choice is not compatibility evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupPreference {
    SteamDeck,
    Pc,
    Both,
}

impl SetupPreference {
    pub fn best_on(self) -> BestOn {
        match self {
            Self::SteamDeck => BestOn::SteamDeck,
            Self::Pc => BestOn::Pc,
            Self::Both => BestOn::Both,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetupFit {
    /// Provisional suitability points, 0–100. None is unknown, never zero.
    pub score: Option<u8>,
    pub reason: String,
    pub blocker: Option<String>,
    /// How the score was built, in order, for the fit details. Saved demo
    /// values have none and show `reason` instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<FitStep>,
}

/// One part of a fit: a starting value, or points added or removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitStep {
    pub label: String,
    pub points: i16,
    /// The first value of a fit, shown without a sign.
    #[serde(default)]
    pub start: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub steam_deck: SetupFit,
    pub pc: SetupFit,
    /// Demo fixtures only. Fit is the only metric the interface shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<u8>,
}

impl Assessment {
    pub fn recommendation(&self) -> BestOn {
        let deck = &self.steam_deck;
        let pc = &self.pc;
        match (deck.blocker.is_some(), pc.blocker.is_some()) {
            (true, true) => return BestOn::NeedsReview,
            (true, false) => {
                return if suitable(pc.score) {
                    BestOn::Pc
                } else {
                    BestOn::NeedsReview
                }
            }
            (false, true) => {
                return if suitable(deck.score) {
                    BestOn::SteamDeck
                } else {
                    BestOn::NeedsReview
                }
            }
            (false, false) => {}
        }
        let (Some(deck), Some(pc)) = (deck.score, pc.score) else {
            return BestOn::NeedsReview;
        };
        if deck > 100 || pc > 100 {
            return BestOn::NeedsReview;
        }
        match (suitable(Some(deck)), suitable(Some(pc))) {
            (false, false) => BestOn::NeedsReview,
            (true, false) => BestOn::SteamDeck,
            (false, true) => BestOn::Pc,
            (true, true) if i16::from(deck) - i16::from(pc) >= 15 => BestOn::SteamDeck,
            (true, true) if i16::from(pc) - i16::from(deck) >= 15 => BestOn::Pc,
            (true, true) => BestOn::Both,
        }
    }
}

fn suitable(score: Option<u8>) -> bool {
    score.is_some_and(|score| (50..=100).contains(&score))
}

/// Valve's Steam Deck compatibility rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckRating {
    Verified,
    Playable,
    Unsupported,
    /// Valve has not rated the game, or reported a category this app does not know.
    Unknown,
}

/// One result from Valve's Deck test list, for example "Interface text is not legible".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckNote {
    pub kind: DeckNoteKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckNoteKind {
    Pass,
    Warning,
    Blocker,
    Info,
}

/// Steam store controller categories: 28 is full support, 18 partial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerSupport {
    Full,
    Partial,
    /// The store lists no controller category.
    None,
}

/// Provider evidence saved with Steam metadata. The assessment is calculated
/// from it on each load and never saved, so rule changes apply to old evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetupEvidence {
    pub deck: DeckRating,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deck_notes: Vec<DeckNote>,
    pub controller: ControllerSupport,
    /// Steam Workshop (store category 30). None in evidence saved before this
    /// field existed, so a later sync fetches it again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workshop: Option<bool>,
    /// Unix seconds when Steam returned this evidence.
    pub checked_at: i64,
}

/// The user's Best on rules. They live in app settings and apply after Steam
/// evidence and play habits. A choice on one game still wins over all rules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    /// 4K, HDR, high refresh rate, and room for mods.
    pub high_end_pc: bool,
    /// Steam tag names. A match moves fit toward PC.
    pub prefer_pc: Vec<String>,
    /// Games with Steam Workshop count as a Prefer PC match.
    pub prefer_pc_mods: bool,
    pub prefer_deck: Vec<String>,
    pub equipment: Vec<Equipment>,
}

/// Equipment that connects to the PC, such as a racing wheel. Games with any
/// of its tags are PC only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Equipment {
    pub name: String,
    pub tags: Vec<String>,
}

/// Steam Deck reports for one game from ProtonDB's community export.
/// Percentages are of the reports that answered that question.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtonDeck {
    pub reports: u16,
    pub runs_percent: u8,
    pub performance_percent: Option<u8>,
    pub battery_percent: Option<u8>,
    pub readability_percent: Option<u8>,
}

/// Fewer Deck reports than this are not evidence.
pub const PROTON_MIN_REPORTS: u16 = 5;

/// Everything an assessment reads besides the game itself.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub rules: Rules,
    /// ProtonDB results by Steam App ID. None when enrichment is off.
    pub proton: Option<std::sync::Arc<std::collections::BTreeMap<u32, ProtonDeck>>>,
}

/// The current context. Settings replace it when rules or enrichment change,
/// then the library recalculates; assessments are never saved.
static CONTEXT: std::sync::RwLock<Option<std::sync::Arc<Context>>> = std::sync::RwLock::new(None);

pub fn set_context(context: Context) {
    if let Ok(mut current) = CONTEXT.write() {
        *current = Some(std::sync::Arc::new(context));
    }
}

pub fn context() -> std::sync::Arc<Context> {
    CONTEXT
        .read()
        .ok()
        .and_then(|current| current.clone())
        .unwrap_or_default()
}

/// Minimum playtime before your own habits count as evidence.
const HABIT_MINUTES: u32 = 120;
/// Points a tag preference adds to one setup and removes from the other.
const PREFERENCE_POINTS: i16 = 15;

/// A fit under construction. Unknown stays unknown: points only change a
/// known score, so a rule never invents Deck evidence.
struct Builder {
    score: Option<i16>,
    steps: Vec<FitStep>,
    blocker: Option<String>,
}

impl Builder {
    fn new(start: Option<(i16, String)>, unknown: &str) -> Self {
        let steps = vec![match &start {
            Some((points, label)) => FitStep {
                label: label.clone(),
                points: *points,
                start: true,
            },
            None => FitStep {
                label: unknown.into(),
                points: 0,
                start: true,
            },
        }];
        Self {
            score: start.map(|(points, _)| points),
            steps,
            blocker: None,
        }
    }

    fn add(&mut self, points: i16, label: impl Into<String>) {
        if self.score.is_some() || points == 0 {
            self.score = self.score.map(|score| score + points);
            self.steps.push(FitStep {
                label: label.into(),
                points,
                start: false,
            });
        }
    }

    fn finish(self) -> SetupFit {
        let reason = self
            .steps
            .iter()
            .map(|step| step.label.as_str())
            .collect::<Vec<_>>()
            .join(". ");
        SetupFit {
            score: self
                .score
                .filter(|_| self.blocker.is_none())
                .map(|score| score.clamp(0, 100) as u8),
            reason: format!("{reason}."),
            blocker: self.blocker,
            steps: self.steps,
        }
    }
}

fn has_tag(tags: &[String], wanted: &[String]) -> Option<String> {
    wanted
        .iter()
        .find(|want| tags.iter().any(|tag| tag.eq_ignore_ascii_case(want.trim())))
        .cloned()
}

/// Everything about one game that fit uses.
pub struct Inputs<'a> {
    pub evidence: &'a SetupEvidence,
    /// Total Steam playtime; it includes Deck time.
    pub total_minutes: u32,
    pub deck_minutes: u32,
    /// Steam store tags.
    pub tags: &'a [String],
    pub proton: Option<&'a ProtonDeck>,
}

/// Rule-based assessment. Order: Valve's rating and controller support, where
/// you played, your PC, ProtonDB Deck reports, tag preferences, then equipment.
/// Steam can also count Deck time as Linux, so platform splits are not added.
pub fn assess(inputs: &Inputs, rules: &Rules) -> Assessment {
    let evidence = inputs.evidence;
    let mut deck = Builder::new(
        match evidence.deck {
            DeckRating::Verified => Some((85, "Valve: Steam Deck Verified".into())),
            DeckRating::Playable => Some((65, "Valve: Steam Deck Playable".into())),
            _ => None,
        },
        match evidence.deck {
            DeckRating::Unsupported => "Valve: Unsupported on Steam Deck",
            _ => "Valve has not rated this game for Steam Deck",
        },
    );
    let mut pc = Builder::new(Some((75, "Starting fit for a Windows PC".into())), "");
    if evidence.deck == DeckRating::Unsupported {
        deck.blocker = Some(
            evidence
                .deck_notes
                .iter()
                .find(|note| note.kind == DeckNoteKind::Blocker)
                .map_or("Valve rates this game Unsupported.".to_owned(), |note| {
                    format!("{}.", note.text)
                }),
        );
    }
    let proton = inputs
        .proton
        .filter(|proton| proton.reports >= PROTON_MIN_REPORTS);
    if let Some(proton) = proton.filter(|_| evidence.deck == DeckRating::Unknown) {
        proton_start(&mut deck, proton);
    }
    for note in evidence
        .deck_notes
        .iter()
        .filter(|note| note.kind == DeckNoteKind::Warning)
    {
        deck.add(0, format!("Valve: {}", note.text));
    }
    match evidence.controller {
        ControllerSupport::Full => deck.add(5, "Full controller support"),
        ControllerSupport::Partial => deck.add(0, "Partial controller support"),
        ControllerSupport::None => {
            deck.add(-15, "No controller support");
            pc.add(10, "Built for keyboard and mouse");
        }
    }
    if inputs.total_minutes >= HABIT_MINUTES {
        let share = f64::from(inputs.deck_minutes.min(inputs.total_minutes))
            / f64::from(inputs.total_minutes);
        if share >= 0.6 {
            deck.add(10, "You played most hours on Steam Deck");
        } else if share <= 0.2 {
            pc.add(10, "You played most hours on PC");
        }
    }
    if rules.high_end_pc {
        pc.add(10, "Your PC: High-end");
    }
    if let Some(proton) = proton {
        proton_problems(&mut deck, evidence.deck, proton);
    }
    let prefer_pc = has_tag(inputs.tags, &rules.prefer_pc)
        .map(|tag| format!("Your rule: Prefer PC for {tag}"))
        .or_else(|| {
            (rules.prefer_pc_mods && evidence.workshop == Some(true))
                .then(|| "Your rule: Prefer PC for games with mods".to_owned())
        });
    let prefer_deck = has_tag(inputs.tags, &rules.prefer_deck)
        .map(|tag| format!("Your rule: Prefer Steam Deck for {tag}"));
    // A game in both lists keeps its Steam fit.
    match (prefer_pc, prefer_deck) {
        (Some(label), None) => {
            pc.add(PREFERENCE_POINTS, &label);
            deck.add(-PREFERENCE_POINTS, &label);
        }
        (None, Some(label)) => {
            deck.add(PREFERENCE_POINTS, &label);
            pc.add(-PREFERENCE_POINTS, &label);
        }
        _ => {}
    }
    if let Some(item) = rules
        .equipment
        .iter()
        .find(|item| has_tag(inputs.tags, &item.tags).is_some())
    {
        deck.blocker = Some(format!("Needs your {}.", item.name.trim()));
    }
    Assessment {
        steam_deck: deck.finish(),
        pc: pc.finish(),
        confidence: None,
    }
}

/// ProtonDB sets a Deck fit that Valve has not given, or blocks the Deck
/// when most players report that the game does not run.
fn proton_start(deck: &mut Builder, proton: &ProtonDeck) {
    let label = format!(
        "ProtonDB: {}% of {} Deck reports say it runs",
        proton.runs_percent, proton.reports
    );
    let points = match proton.runs_percent {
        90..=100 => 70,
        70..=89 => 55,
        _ => {
            deck.blocker = Some(format!("{label}."));
            return;
        }
    };
    deck.score = Some(points);
    deck.steps = vec![FitStep {
        label,
        points,
        start: true,
    }];
}

/// Problems that Deck players report lower a known fit.
fn proton_problems(deck: &mut Builder, valve: DeckRating, proton: &ProtonDeck) {
    let reports = proton.reports;
    if valve != DeckRating::Unknown && proton.runs_percent < 70 {
        deck.add(
            -15,
            format!(
                "ProtonDB: only {}% of {reports} Deck reports say it runs",
                proton.runs_percent
            ),
        );
    }
    let problems = [
        (proton.performance_percent, 25, -10, "performance problems"),
        (proton.battery_percent, 30, -5, "battery problems"),
        (proton.readability_percent, 30, -5, "hard-to-read text"),
    ];
    for (percent, limit, points, what) in problems {
        if let Some(percent) = percent.filter(|percent| *percent >= limit) {
            deck.add(
                points,
                format!("ProtonDB: {percent}% of {reports} Deck reports note {what}"),
            );
        }
    }
}

/// A saved demo assessment first; otherwise one calculated from Steam
/// evidence with the current context. None when the game has neither.
pub fn assessment(game: &crate::records::GameData) -> Option<Assessment> {
    assessment_in(game, &context())
}

pub fn assessment_in(game: &crate::records::GameData, context: &Context) -> Option<Assessment> {
    if let Some(saved) = &game.suitability {
        return Some(saved.clone());
    }
    let steam = game.steam.as_ref()?;
    let metadata = steam.metadata.as_ref()?;
    let evidence = metadata.setup.as_ref()?;
    let proton = context
        .proton
        .as_ref()
        .and_then(|proton| proton.get(&steam.app_id));
    Some(assess(
        &Inputs {
            evidence,
            total_minutes: steam.playtime_minutes,
            deck_minutes: steam.platform_minutes.deck,
            tags: &metadata.tags,
            proton,
        },
        &context.rules,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(deck: DeckRating, controller: ControllerSupport) -> SetupEvidence {
        SetupEvidence {
            deck,
            deck_notes: Vec::new(),
            controller,
            workshop: Some(false),
            checked_at: 0,
        }
    }

    fn tags(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn run(
        evidence: &SetupEvidence,
        minutes: (u32, u32),
        tags: &[String],
        proton: Option<&ProtonDeck>,
        rules: &Rules,
    ) -> Assessment {
        assess(
            &Inputs {
                evidence,
                total_minutes: minutes.0,
                deck_minutes: minutes.1,
                tags,
                proton,
            },
            rules,
        )
    }

    #[test]
    fn steam_evidence_maps_to_each_result() {
        use ControllerSupport as C;
        use DeckRating as D;
        let result = |deck, controller, total, on_deck| {
            run(
                &evidence(deck, controller),
                (total, on_deck),
                &[],
                None,
                &Rules::default(),
            )
            .recommendation()
        };
        assert_eq!(result(D::Verified, C::Full, 0, 0), BestOn::SteamDeck);
        assert_eq!(result(D::Playable, C::Full, 0, 0), BestOn::Both);
        assert_eq!(result(D::Playable, C::None, 0, 0), BestOn::Pc);
        assert_eq!(result(D::Unsupported, C::Full, 0, 0), BestOn::Pc);
        // Missing Deck evidence is not Both.
        assert_eq!(result(D::Unknown, C::Full, 0, 0), BestOn::NeedsReview);
        // Your own habits move a close call, but only after two hours.
        assert_eq!(result(D::Verified, C::Partial, 0, 0), BestOn::Both);
        assert_eq!(result(D::Verified, C::Partial, 600, 500), BestOn::SteamDeck);
        assert_eq!(result(D::Playable, C::Full, 600, 0), BestOn::Pc);
        assert_eq!(result(D::Playable, C::Full, 100, 0), BestOn::Both);
    }

    #[test]
    fn blocker_text_follows_the_evidence() {
        let mut blocked = evidence(DeckRating::Unsupported, ControllerSupport::Full);
        blocked.deck_notes = vec![DeckNote {
            kind: DeckNoteKind::Blocker,
            text: "Unsupported anti cheat configuration".into(),
        }];
        let assessment = run(&blocked, (0, 0), &[], None, &Rules::default());
        assert_eq!(assessment.steam_deck.score, None);
        assert_eq!(
            assessment.steam_deck.blocker.as_deref(),
            Some("Unsupported anti cheat configuration.")
        );
        assert_eq!(assessment.confidence, None);
    }

    /// Cyberpunk 2077: Verified, full controller, played on PC. Its Deck
    /// reports are from the October 2026 ProtonDB export.
    #[test]
    fn rules_and_proton_move_a_heavy_verified_game_to_pc() {
        let cyberpunk = evidence(DeckRating::Verified, ControllerSupport::Full);
        let tags = tags(&["Cyberpunk", "Open World", "RPG", "FPS"]);
        let plain = run(&cyberpunk, (2600, 0), &tags, None, &Rules::default());
        assert_eq!(plain.recommendation(), BestOn::Both);
        let proton = ProtonDeck {
            reports: 108,
            runs_percent: 93,
            performance_percent: Some(28),
            battery_percent: Some(33),
            readability_percent: Some(39),
        };
        let with_proton = run(
            &cyberpunk,
            (2600, 0),
            &tags,
            Some(&proton),
            &Rules::default(),
        );
        assert_eq!(with_proton.steam_deck.score, Some(70));
        assert_eq!(with_proton.recommendation(), BestOn::Pc);
        let rules = Rules {
            high_end_pc: true,
            prefer_pc: vec!["open world".into()],
            ..Rules::default()
        };
        let ruled = run(&cyberpunk, (2600, 0), &tags, Some(&proton), &rules);
        assert_eq!(ruled.steam_deck.score, Some(55));
        assert_eq!(ruled.pc.score, Some(100));
        let labels: Vec<_> = ruled
            .steam_deck
            .steps
            .iter()
            .map(|s| s.label.as_str())
            .collect();
        assert_eq!(labels[0], "Valve: Steam Deck Verified");
        assert!(labels.contains(&"Your rule: Prefer PC for open world"));
        assert!(ruled.steam_deck.steps[0].start);
    }

    #[test]
    fn deck_preference_equipment_and_mixed_tags() {
        let verified = evidence(DeckRating::Verified, ControllerSupport::Full);
        let rules = Rules {
            prefer_pc: vec!["Open World".into()],
            prefer_deck: vec!["Casual".into()],
            equipment: vec![Equipment {
                name: "Moza R5 wheel".into(),
                tags: vec!["Racing".into(), "Automobile Sim".into()],
            }],
            ..Rules::default()
        };
        // Balatro: a Deck preference makes the lead clear.
        let balatro = run(
            &verified,
            (0, 0),
            &tags(&["Card Game", "Casual"]),
            None,
            &rules,
        );
        assert_eq!(
            (balatro.steam_deck.score, balatro.pc.score),
            (Some(100), Some(60))
        );
        assert_eq!(balatro.recommendation(), BestOn::SteamDeck);
        // A game in both lists keeps its Steam fit.
        let both = run(
            &verified,
            (0, 0),
            &tags(&["Casual", "Open World"]),
            None,
            &rules,
        );
        assert_eq!(both.steam_deck.score, Some(90));
        // Equipment wins over any Deck evidence.
        let racing = run(
            &verified,
            (0, 0),
            &tags(&["Racing", "Casual"]),
            None,
            &rules,
        );
        assert_eq!(racing.steam_deck.score, None);
        assert_eq!(
            racing.steam_deck.blocker.as_deref(),
            Some("Needs your Moza R5 wheel.")
        );
        assert_eq!(racing.recommendation(), BestOn::Pc);
    }

    #[test]
    fn mods_rule_needs_workshop_evidence() {
        let mut modded = evidence(DeckRating::Verified, ControllerSupport::Full);
        modded.workshop = Some(true);
        let rules = Rules {
            prefer_pc_mods: true,
            ..Rules::default()
        };
        let result = run(&modded, (0, 0), &[], None, &rules);
        assert_eq!(
            (result.steam_deck.score, result.pc.score),
            (Some(75), Some(90))
        );
        modded.workshop = None;
        assert_eq!(run(&modded, (0, 0), &[], None, &rules).pc.score, Some(75));
    }

    #[test]
    fn proton_rates_unknown_games_only_with_enough_reports() {
        let unknown = evidence(DeckRating::Unknown, ControllerSupport::Full);
        let good = ProtonDeck {
            reports: 20,
            runs_percent: 95,
            ..ProtonDeck::default()
        };
        let rated = run(&unknown, (0, 0), &[], Some(&good), &Rules::default());
        assert_eq!(rated.steam_deck.score, Some(75));
        let few = ProtonDeck { reports: 4, ..good };
        let unrated = run(&unknown, (0, 0), &[], Some(&few), &Rules::default());
        assert_eq!(unrated.recommendation(), BestOn::NeedsReview);
        let broken = ProtonDeck {
            runs_percent: 8,
            ..good
        };
        let blocked = run(&unknown, (0, 0), &[], Some(&broken), &Rules::default());
        assert!(blocked.steam_deck.blocker.is_some());
        assert_eq!(blocked.recommendation(), BestOn::Pc);
    }
}
