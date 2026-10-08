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

/// Invented profiles from the approved HTML, isolated from real libraries.
pub fn play_now_games() -> Result<Vec<Game>> {
    use gamesync_desktop::{
        recommendations::{Activity as A, Effort as E, Profile, Stopping},
        records::{GameData, GameRevision},
    };
    let mut games = games()?;
    for game in &mut games {
        let (mechanical, cognitive, narrative, onboarding, min, ideal, setup, activities) =
            match game.id.as_u128() {
                1145360 => (E::Medium, E::Low, E::Low, E::Low, 20, 35, 2, vec![A::Runs]),
                413150 => (
                    E::Low,
                    E::Medium,
                    E::Low,
                    E::Low,
                    15,
                    30,
                    2,
                    vec![A::Explore],
                ),
                367520 => (
                    E::High,
                    E::Medium,
                    E::Low,
                    E::Low,
                    25,
                    60,
                    2,
                    vec![A::Explore, A::Weird],
                ),
                620 => (
                    E::Medium,
                    E::High,
                    E::Low,
                    E::Low,
                    15,
                    30,
                    2,
                    vec![A::Weird, A::Multiplayer],
                ),
                1091500 => (
                    E::Medium,
                    E::High,
                    E::High,
                    E::Low,
                    45,
                    90,
                    4,
                    vec![A::Shoot, A::Drive, A::Explore],
                ),
                990080 => (
                    E::Medium,
                    E::Medium,
                    E::Medium,
                    E::High,
                    30,
                    60,
                    4,
                    vec![A::Fly, A::Explore],
                ),
                646570 => (
                    E::Low,
                    E::High,
                    E::Low,
                    E::Low,
                    10,
                    45,
                    1,
                    vec![A::Cards, A::Runs],
                ),
                753640 => (
                    E::Medium,
                    E::High,
                    E::High,
                    E::Medium,
                    22,
                    50,
                    3,
                    vec![A::Fly, A::Explore, A::Weird],
                ),
                2379780 => (
                    E::Low,
                    E::Medium,
                    E::Low,
                    E::Low,
                    10,
                    30,
                    1,
                    vec![A::Cards, A::Runs, A::Weird],
                ),
                1055540 => (E::Low, E::Low, E::Low, E::Low, 10, 25, 1, vec![A::Explore]),
                632360 => (
                    E::High,
                    E::Medium,
                    E::Low,
                    E::Low,
                    35,
                    60,
                    3,
                    vec![A::Shoot, A::Multiplayer, A::Runs],
                ),
                _ => (
                    E::High,
                    E::High,
                    E::Medium,
                    E::Low,
                    30,
                    90,
                    3,
                    vec![A::Horse, A::Explore],
                ),
            };
        if game.id.as_u128() == 1245620 || game.id.as_u128() == 1055540 {
            game.status = "backlog".into();
        }
        let mut data = GameData::new(game.title.clone());
        data.steam = Some(gamesync_desktop::records::SteamData {
            app_id: game.id.as_u128() as u32,
            description: Some(game.description.clone()),
            playtime_minutes: game.playtime_minutes,
            owned: !matches!(game.id.as_u128(), 990080 | 753640 | 1245620),
            last_played: None,
            platform_minutes: Default::default(),
            wishlist: None,
            metadata: None,
            extra: Default::default(),
        });
        data.personal.status = game.status.clone();
        data.personal.hidden = game.id.as_u128() == 990080;
        data.personal.play_now.profile = Profile {
            mechanical: Some(mechanical),
            cognitive: Some(cognitive),
            narrative: Some(narrative),
            onboarding: Some(onboarding),
            minimum_minutes: Some(min),
            ideal_minutes: Some(ideal),
            setup_minutes: Some(setup),
            stopping: Some(Stopping::Flexible),
            activities: Some(activities),
        };
        game.record = Some(GameRevision {
            schema_version: 1,
            game_id: game.id,
            revision_id: uuid::Uuid::new_v4(),
            parents: vec![],
            deleted: false,
            game: data,
            extra: Default::default(),
        });
    }
    Ok(games)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_games_are_valid() {
        assert!(super::games().unwrap().len() >= 12);
    }
}
