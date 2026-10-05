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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub steam_deck: SetupFit,
    pub pc: SetupFit,
    /// Recommendation confidence, 0–100, independent of setup fit. Demo only
    /// until evidence quality is assessed; missing values remain unknown.
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
    /// Unix seconds when Steam returned this evidence.
    pub checked_at: i64,
}

/// Minimum playtime before your own habits count as evidence.
const HABIT_MINUTES: u32 = 120;

/// Rule-based assessment from Steam evidence. PC assumes a capable Windows PC.
/// Each source adjusts a setup once: Valve's rating, controller support, and
/// where you played. `total_minutes` includes Deck time; Steam can also count
/// Deck time as Linux, so platform splits are not added together.
pub fn assess(evidence: &SetupEvidence, total_minutes: u32, deck_minutes: u32) -> Assessment {
    let warnings: Vec<&str> = evidence
        .deck_notes
        .iter()
        .filter(|note| note.kind == DeckNoteKind::Warning)
        .map(|note| note.text.as_str())
        .collect();
    let (mut deck_score, mut deck_reason) = match evidence.deck {
        DeckRating::Verified => (Some(85), "Steam Deck Verified.".to_owned()),
        DeckRating::Playable => (Some(65), "Steam Deck Playable.".to_owned()),
        DeckRating::Unsupported => (None, "Unsupported on Steam Deck.".to_owned()),
        DeckRating::Unknown => (
            None,
            "Valve has not rated this game for Steam Deck.".to_owned(),
        ),
    };
    if !warnings.is_empty() {
        deck_reason.push_str(&format!(" {}.", warnings.join(". ")));
    }
    let deck_blocker = (evidence.deck == DeckRating::Unsupported).then(|| {
        evidence
            .deck_notes
            .iter()
            .find(|note| note.kind == DeckNoteKind::Blocker)
            .map_or("Valve rates this game Unsupported.".to_owned(), |note| {
                format!("{}.", note.text)
            })
    });
    let (mut pc_score, mut pc_reason) = (75, "Assumes a capable Windows PC.".to_owned());
    match evidence.controller {
        ControllerSupport::Full => {
            deck_score = deck_score.map(|score| score + 5);
            deck_reason.push_str(" Full controller support.");
        }
        ControllerSupport::Partial => deck_reason.push_str(" Partial controller support."),
        ControllerSupport::None => {
            deck_score = deck_score.map(|score: i32| score - 15);
            pc_score += 10;
            pc_reason.push_str(" Built for keyboard and mouse.");
        }
    }
    let habit = total_minutes >= HABIT_MINUTES;
    if habit {
        let deck_share = f64::from(deck_minutes.min(total_minutes)) / f64::from(total_minutes);
        if deck_share >= 0.6 {
            deck_score = deck_score.map(|score| score + 10);
            deck_reason.push_str(" You played most hours on Steam Deck.");
        } else if deck_share <= 0.2 {
            pc_score += 10;
            pc_reason.push_str(" You played most hours on PC.");
        }
    }
    // Coverage of the evidence, not accuracy: an unrated Deck stays uncertain.
    let confidence =
        15 + if evidence.deck == DeckRating::Unknown {
            0
        } else {
            50
        } + 15
            + if habit { 20 } else { 0 };
    Assessment {
        steam_deck: SetupFit {
            score: deck_score.map(|score| score.clamp(0, 100) as u8),
            reason: deck_reason,
            blocker: deck_blocker,
        },
        pc: SetupFit {
            score: Some(pc_score.clamp(0, 100) as u8),
            reason: pc_reason,
            blocker: None,
        },
        confidence: Some(confidence),
    }
}

/// A saved demo assessment first; otherwise one calculated from Steam evidence.
/// None when the game has neither.
pub fn assessment(game: &crate::records::GameData) -> Option<Assessment> {
    if let Some(saved) = &game.suitability {
        return Some(saved.clone());
    }
    let steam = game.steam.as_ref()?;
    let evidence = steam.metadata.as_ref()?.setup.as_ref()?;
    Some(assess(
        evidence,
        steam.playtime_minutes,
        steam.platform_minutes.deck,
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
            checked_at: 0,
        }
    }

    #[test]
    fn steam_evidence_maps_to_each_result() {
        use ControllerSupport as C;
        use DeckRating as D;
        let result = |deck, controller, total, on_deck| {
            assess(&evidence(deck, controller), total, on_deck).recommendation()
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
    fn blocker_text_and_confidence_follow_the_evidence() {
        let mut blocked = evidence(DeckRating::Unsupported, ControllerSupport::Full);
        blocked.deck_notes = vec![DeckNote {
            kind: DeckNoteKind::Blocker,
            text: "Unsupported anti cheat configuration".into(),
        }];
        let assessment = assess(&blocked, 0, 0);
        assert_eq!(assessment.steam_deck.score, None);
        assert_eq!(
            assessment.steam_deck.blocker.as_deref(),
            Some("Unsupported anti cheat configuration.")
        );
        let unknown = assess(
            &evidence(DeckRating::Unknown, ControllerSupport::Full),
            0,
            0,
        );
        assert!(unknown.confidence < assessment.confidence);
    }
}
