//! Bundled demo data. Loading it never reads or changes a personal library.

use crate::model::Game;
use anyhow::{ensure, Context as _, Result};
use std::collections::HashSet;

pub fn games() -> Result<Vec<Game>> {
    let games: Vec<Game> = serde_json::from_str(include_str!("../fixtures/games.json"))
        .context("Could not read the bundled demo library")?;
    let mut ids = HashSet::new();
    for game in &games {
        ensure!(ids.insert(game.id), "Duplicate demo game ID: {}", game.id);
        ensure!(
            game.rating.is_none_or(|rating| (1..=10).contains(&rating)),
            "Invalid demo rating: {}",
            game.title
        );
    }
    Ok(games)
}

/// Synthetic fit evidence is opt-in and never written into a user's library.
pub fn best_on_games() -> Result<Vec<Game>> {
    use gamesync_desktop::{
        records::{GameData, GameRevision},
        suitability::{Assessment, SetupFit},
    };
    use serde::Deserialize;
    use uuid::Uuid;

    #[derive(Deserialize)]
    struct Entry {
        id: u32,
        #[serde(flatten)]
        assessment: Assessment,
    }
    let entries: Vec<Entry> = serde_json::from_str(include_str!("../fixtures/best-on.json"))
        .context("Could not read the Best on demo")?;
    let mut games = games()?;
    for game in &mut games {
        let entry = entries
            .iter()
            .find(|entry| Uuid::from_u128(u128::from(entry.id)) == game.id)
            .context("Missing Best on fixture")?;
        let mut data = GameData::new(game.title.clone());
        data.personal.status = game.status.clone();
        data.personal.rating = game.rating;
        data.personal.tags = game.tags.clone();
        data.personal.favorite = game.favorite;
        data.suitability = Some(entry.assessment.clone());
        game.record = Some(GameRevision {
            schema_version: gamesync_desktop::records::SCHEMA_VERSION,
            game_id: game.id,
            revision_id: Uuid::new_v4(),
            parents: Vec::new(),
            deleted: false,
            game: data,
            extra: Default::default(),
        });
    }
    // A fictional title demonstrates a blocker without inventing a claim about
    // a real game's anti-cheat. It uses the missing-cover fallback.
    let mut blocked = games[0].clone();
    blocked.id = Uuid::from_u128(900_000_001);
    blocked.title = "Arena Lab · blocker example".into();
    blocked.description = "A fictional competitive game for reviewing setup warnings.".into();
    blocked.cover = "covers/missing.jpg".into();
    blocked.status = "backlog".into();
    blocked.rating = None;
    blocked.favorite = false;
    blocked.playtime_minutes = 0;
    blocked.tags = vec!["Demo".into()];
    let mut data = GameData::new(blocked.title.clone());
    data.suitability = Some(Assessment {
        confidence: Some(96),
        steam_deck: SetupFit {
            score: None,
            reason: "Local handheld play is unavailable in this scenario.".into(),
            blocker: Some("Demo blocker: anti-cheat does not support SteamOS.".into()),
            steps: Vec::new(),
        },
        pc: SetupFit {
            score: Some(87),
            reason: "The fictional Windows setup supports this game and mouse aiming.".into(),
            blocker: None,
            steps: Vec::new(),
        },
    });
    blocked.record = Some(GameRevision {
        schema_version: gamesync_desktop::records::SCHEMA_VERSION,
        game_id: blocked.id,
        revision_id: Uuid::new_v4(),
        parents: Vec::new(),
        deleted: false,
        game: data,
        extra: Default::default(),
    });
    games.push(blocked);
    Ok(games)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_games_are_valid() {
        assert!(super::games().unwrap().len() >= 12);
    }
}
