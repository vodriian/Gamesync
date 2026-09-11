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

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_games_are_valid() {
        assert!(super::games().unwrap().len() >= 12);
    }
}
