//! Smart collection rules. Membership is computed from records, never stored.
//! Rules are serializable so saved user smart collections can reuse them later.

use crate::records::GameData;
use serde::{Deserialize, Serialize};

/// The sidebar groups, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartKind {
    Genre,
    SteamTag,
    MyTag,
    Rating,
    Playtime,
}

impl SmartKind {
    pub const ALL: [Self; 5] = [
        Self::Genre,
        Self::SteamTag,
        Self::MyTag,
        Self::Rating,
        Self::Playtime,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Genre => "Genres",
            Self::SteamTag => "Steam tags",
            Self::MyTag => "My tags",
            Self::Rating => "Rating",
            Self::Playtime => "Time played",
        }
    }
}

/// Whole stars rounded down from half-star units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RatingBand {
    Five,
    Four,
    Three,
    OneToTwo,
    Unrated,
}

impl RatingBand {
    pub const ALL: [Self; 5] = [
        Self::Five,
        Self::Four,
        Self::Three,
        Self::OneToTwo,
        Self::Unrated,
    ];

    /// `rating` uses half-star units, 1 through 10.
    pub fn of(rating: Option<u8>) -> Self {
        match rating {
            Some(10..) => Self::Five,
            Some(8..=9) => Self::Four,
            Some(6..=7) => Self::Three,
            Some(1..=5) => Self::OneToTwo,
            Some(0) | None => Self::Unrated,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Five => "5 stars",
            Self::Four => "4 stars",
            Self::Three => "3 stars",
            Self::OneToTwo => "1–2 stars",
            Self::Unrated => "Unrated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaytimeBand {
    NotPlayed,
    UnderOneHour,
    OneToTen,
    TenToFifty,
    FiftyPlus,
}

impl PlaytimeBand {
    pub const ALL: [Self; 5] = [
        Self::NotPlayed,
        Self::UnderOneHour,
        Self::OneToTen,
        Self::TenToFifty,
        Self::FiftyPlus,
    ];

    pub fn of(minutes: u32) -> Self {
        match minutes {
            0 => Self::NotPlayed,
            1..60 => Self::UnderOneHour,
            60..600 => Self::OneToTen,
            600..3000 => Self::TenToFifty,
            _ => Self::FiftyPlus,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NotPlayed => "Not played",
            Self::UnderOneHour => "Under 1 h",
            Self::OneToTen => "1–10 h",
            Self::TenToFifty => "10–50 h",
            Self::FiftyPlus => "50 h or more",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum SmartRule {
    Genre(String),
    SteamTag(String),
    MyTag(String),
    Rating(RatingBand),
    Playtime(PlaytimeBand),
}

impl SmartRule {
    pub fn kind(&self) -> SmartKind {
        match self {
            Self::Genre(_) => SmartKind::Genre,
            Self::SteamTag(_) => SmartKind::SteamTag,
            Self::MyTag(_) => SmartKind::MyTag,
            Self::Rating(_) => SmartKind::Rating,
            Self::Playtime(_) => SmartKind::Playtime,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Genre(name) | Self::SteamTag(name) | Self::MyTag(name) => name,
            Self::Rating(band) => band.label(),
            Self::Playtime(band) => band.label(),
        }
    }

    pub fn matches(&self, game: &GameData) -> bool {
        rules_for(game, self.kind()).any(|rule| rule == *self)
    }
}

/// Every rule of one kind that a game belongs to. Text values are
/// deduplicated, so a repeated tag counts the game once.
pub fn rules_for(game: &GameData, kind: SmartKind) -> impl Iterator<Item = SmartRule> + '_ {
    let metadata = game
        .steam
        .as_ref()
        .and_then(|steam| steam.metadata.as_ref());
    let texts: Vec<&String> = match kind {
        SmartKind::Genre => metadata.map_or(&[][..], |m| &m.genres).iter().collect(),
        SmartKind::SteamTag => metadata.map_or(&[][..], |m| &m.tags).iter().collect(),
        SmartKind::MyTag => game.personal.tags.iter().collect(),
        SmartKind::Rating | SmartKind::Playtime => Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    let text_rules = texts.into_iter().filter_map(move |text| {
        let text = text.trim();
        (!text.is_empty() && seen.insert(text.to_owned())).then(|| match kind {
            SmartKind::Genre => SmartRule::Genre(text.into()),
            SmartKind::SteamTag => SmartRule::SteamTag(text.into()),
            _ => SmartRule::MyTag(text.into()),
        })
    });
    let band = match kind {
        SmartKind::Rating => Some(SmartRule::Rating(RatingBand::of(game.personal.rating))),
        // Only Steam games have playtime. The app does not guess it for manual games.
        SmartKind::Playtime => game
            .steam
            .as_ref()
            .map(|steam| SmartRule::Playtime(PlaytimeBand::of(steam.playtime_minutes))),
        _ => None,
    };
    text_rules.chain(band)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::{SteamData, SteamMetadata};

    fn game(rating: Option<u8>, steam_minutes: Option<u32>) -> GameData {
        let mut game = GameData::new("Test");
        game.personal.rating = rating;
        game.personal.tags = vec!["Cozy".into(), " Cozy ".into(), "".into()];
        game.steam = steam_minutes.map(|minutes| SteamData {
            app_id: 42,
            description: None,
            playtime_minutes: minutes,
            owned: true,
            last_played: None,
            platform_minutes: Default::default(),
            wishlist: None,
            metadata: Some(SteamMetadata {
                genres: vec!["Puzzle".into()],
                tags: vec!["Cozy".into(), "Relaxing".into()],
                ..Default::default()
            }),
            extra: Default::default(),
        });
        game
    }

    #[test]
    fn rating_bands_round_half_stars_down() {
        let bands = [
            None,
            Some(1),
            Some(5),
            Some(6),
            Some(7),
            Some(8),
            Some(9),
            Some(10),
        ]
        .map(RatingBand::of);
        use RatingBand::*;
        assert_eq!(
            bands,
            [Unrated, OneToTwo, OneToTwo, Three, Three, Four, Four, Five]
        );
    }

    #[test]
    fn playtime_band_edges() {
        let bands = [0, 1, 59, 60, 599, 600, 2999, 3000].map(PlaytimeBand::of);
        use PlaytimeBand::*;
        assert_eq!(
            bands,
            [
                NotPlayed,
                UnderOneHour,
                UnderOneHour,
                OneToTen,
                OneToTen,
                TenToFifty,
                TenToFifty,
                FiftyPlus
            ]
        );
    }

    #[test]
    fn steam_and_personal_tags_are_separate_rules() {
        let game = game(Some(9), Some(120));
        assert!(SmartRule::SteamTag("Relaxing".into()).matches(&game));
        assert!(!SmartRule::MyTag("Relaxing".into()).matches(&game));
        assert!(SmartRule::MyTag("Cozy".into()).matches(&game));
        assert!(SmartRule::Genre("Puzzle".into()).matches(&game));
        assert!(SmartRule::Rating(RatingBand::Four).matches(&game));
        assert!(SmartRule::Playtime(PlaytimeBand::OneToTen).matches(&game));
        // Duplicate and blank personal tags produce one rule.
        assert_eq!(rules_for(&game, SmartKind::MyTag).count(), 1);
    }

    #[test]
    fn manual_games_have_no_playtime_band() {
        let game = game(None, None);
        assert_eq!(rules_for(&game, SmartKind::Playtime).count(), 0);
        assert!(SmartRule::Rating(RatingBand::Unrated).matches(&game));
    }

    #[test]
    fn rules_round_trip_for_future_saved_collections() {
        let rule = SmartRule::Playtime(PlaytimeBand::FiftyPlus);
        let json = serde_json::to_string(&rule).unwrap();
        assert_eq!(json, r#"{"kind":"playtime","value":"fifty_plus"}"#);
        assert_eq!(serde_json::from_str::<SmartRule>(&json).unwrap(), rule);
    }
}
