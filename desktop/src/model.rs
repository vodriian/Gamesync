//! Game data and filtering. This module has no UI or filesystem dependencies.

use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Status {
    Backlog,
    Wanted,
    Playing,
    Paused,
    Completed,
    Dropped,
}

impl Status {
    pub const ALL: [Self; 6] = [
        Self::Backlog,
        Self::Wanted,
        Self::Playing,
        Self::Paused,
        Self::Completed,
        Self::Dropped,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::Wanted => "Want to play",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Dropped => "Dropped",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Game {
    pub id: u32,
    pub title: String,
    pub cover: String,
    pub description: String,
    pub status: Status,
    /// Half-star units, from 1 to 10. None means unrated.
    pub rating: Option<u8>,
    pub tags: Vec<String>,
    pub playtime_minutes: u32,
    pub favorite: bool,
}

impl Game {
    pub fn rating_label(&self) -> String {
        self.rating.map_or_else(
            || "Unrated".into(),
            |value| format!("{:.1} / 5", f32::from(value) / 2.),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Favorites,
    Status(Status),
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All games",
            Self::Favorites => "Favorites",
            Self::Status(status) => status.label(),
        }
    }

    fn contains(self, game: &Game) -> bool {
        match self {
            Self::All => true,
            Self::Favorites => game.favorite,
            Self::Status(status) => game.status == status,
        }
    }
}

/// One selection shared by the grid and inspector. IDs survive sorting/filtering.
pub struct Library {
    pub games: Vec<Game>,
    pub visible: Arc<Vec<usize>>,
    pub selected: Option<u32>,
    pub scope: Scope,
    query: String,
    search_keys: Vec<String>,
}

impl Library {
    pub fn new(mut games: Vec<Game>) -> Self {
        games.sort_by_key(|game| game.title.to_lowercase());
        let search_keys = games
            .iter()
            .map(|g| format!("{} {}", g.title, g.tags.join(" ")).to_lowercase())
            .collect();
        let visible = Arc::new((0..games.len()).collect());
        Self {
            games,
            visible,
            selected: None,
            scope: Scope::All,
            query: String::new(),
            search_keys,
        }
    }

    pub fn set_scope(&mut self, scope: Scope) {
        self.scope = scope;
        self.recompute();
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
        self.recompute();
    }

    pub fn count(&self, scope: Scope) -> usize {
        self.games
            .iter()
            .filter(|game| scope.contains(game))
            .count()
    }

    pub fn selected_game(&self) -> Option<&Game> {
        let id = self.selected?;
        self.games.iter().find(|game| game.id == id)
    }

    pub fn select_slot(&mut self, slot: usize) {
        self.selected = self.visible.get(slot).map(|&index| self.games[index].id);
    }

    pub fn selected_slot(&self) -> Option<usize> {
        self.visible
            .iter()
            .position(|&index| Some(self.games[index].id) == self.selected)
    }

    pub fn step_selection(&mut self, delta: isize) -> Option<usize> {
        if self.visible.is_empty() {
            return None;
        }
        let slot = self.selected_slot().map_or(0, |slot| {
            slot.saturating_add_signed(delta)
                .min(self.visible.len() - 1)
        });
        self.select_slot(slot);
        Some(slot)
    }

    fn recompute(&mut self) {
        self.visible = Arc::new(
            self.games
                .iter()
                .enumerate()
                .filter_map(|(index, game)| {
                    (self.scope.contains(game) && self.search_keys[index].contains(&self.query))
                        .then_some(index)
                })
                .collect(),
        );
        if self.selected_slot().is_none() {
            self.selected = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::new(serde_json::from_str(include_str!("../fixtures/games.json")).unwrap())
    }

    #[test]
    fn filters_compose_and_clear_hidden_selection() {
        let mut lib = library();
        lib.set_query("hAdEs");
        assert_eq!(lib.visible.len(), 1);
        lib.select_slot(0);
        assert_eq!(lib.selected, Some(1145360));
        lib.set_scope(Scope::Status(Status::Completed));
        assert!(lib.visible.is_empty());
        assert!(lib.selected_game().is_none());
    }

    #[test]
    fn selection_survives_refilter_by_identity() {
        let mut lib = library();
        lib.set_query("hades");
        lib.select_slot(0);
        lib.set_query("");
        assert_eq!(lib.selected_game().unwrap().title, "Hades");
    }

    #[test]
    fn keyboard_stops_at_edges_and_handles_empty_results() {
        let mut lib = library();
        assert_eq!(lib.step_selection(-1), Some(0));
        assert_eq!(lib.step_selection(-1), Some(0));
        assert_eq!(lib.step_selection(10_000), Some(lib.visible.len() - 1));
        lib.set_query("no-match-123");
        assert_eq!(lib.step_selection(1), None);
    }

    #[test]
    fn search_includes_tags_and_counts_are_scope_totals() {
        let mut lib = library();
        let total = lib.count(Scope::All);
        lib.set_query("puzzle");
        assert!(!lib.visible.is_empty());
        assert!(lib.visible.len() < total);
        assert_eq!(lib.count(Scope::All), total);
    }
}
