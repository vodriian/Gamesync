//! Game data and filtering. This module has no UI or filesystem dependencies.

use serde::Deserialize;
use std::sync::Arc;

use gamesync_desktop::{
    library::{LibraryDefinitions, StatusDefinition},
    library_reader::LoadedLibrary,
};
use std::path::PathBuf;
use uuid::Uuid;

fn demo_id<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Uuid, D::Error> {
    u32::deserialize(deserializer).map(|id| Uuid::from_u128(id as u128))
}
fn status_key<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    String::deserialize(deserializer).map(|status| status.to_lowercase())
}

#[derive(Debug, Clone, Deserialize)]
pub struct Game {
    #[serde(deserialize_with = "demo_id")]
    pub id: Uuid,
    pub title: String,
    pub cover: String,
    pub description: String,
    #[serde(deserialize_with = "status_key")]
    pub status: String,
    #[serde(skip)]
    pub status_label: String,
    #[serde(skip)]
    pub collections: Vec<String>,
    #[serde(skip)]
    pub cover_path: Option<PathBuf>,
    /// Half-star units, from 1 to 10. None means unrated.
    pub rating: Option<u8>,
    pub tags: Vec<String>,
    pub playtime_minutes: u32,
    pub favorite: bool,
    #[serde(skip)]
    pub record: Option<gamesync_desktop::records::GameRevision>,
}

impl Game {
    pub fn rating_label(&self) -> String {
        self.rating.map_or_else(
            || "Unrated".into(),
            |value| format!("{:.1} / 5", f32::from(value) / 2.),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    All,
    Favorites,
    Status(String),
    Collection(Uuid),
}
impl Scope {
    fn contains(&self, game: &Game) -> bool {
        match self {
            Self::Collection(id) => game
                .record
                .as_ref()
                .is_some_and(|r| r.game.personal.collections.contains(id)),
            Self::All => true,
            Self::Favorites => game.favorite,
            Self::Status(status) => game.status == *status,
        }
    }
}

/// One selection shared by the grid and inspector. IDs survive sorting/filtering.
pub struct Library {
    pub games: Vec<Game>,
    pub visible: Arc<Vec<usize>>,
    pub selected: Option<Uuid>,
    pub name: String,
    pub statuses: Vec<StatusDefinition>,
    pub demo: bool,
    pub source: Option<(PathBuf, gamesync_desktop::library::LibraryRevision)>,
    pub write_issue: Option<String>,
    pub conflicts: std::collections::BTreeMap<Uuid, String>,
    pub media_version: u64,
    pub scope: Scope,
    query: String,
    search_keys: Vec<String>,
}

impl Library {
    pub fn new(mut games: Vec<Game>) -> Self {
        let statuses = LibraryDefinitions::new("Demo library").statuses;
        for game in &mut games {
            if game.status_label.is_empty() {
                game.status_label = statuses
                    .iter()
                    .find(|status| status.key == game.status)
                    .map(|status| status.label.clone())
                    .unwrap_or_else(|| game.status.clone());
            }
        }
        games.sort_by_key(|game| game.title.to_lowercase());
        let search_keys = games
            .iter()
            .map(|g| format!("{} {}", g.title, g.tags.join(" ")).to_lowercase())
            .collect();
        let visible = Arc::new((0..games.len()).collect());
        Self {
            games,
            visible,
            name: "Demo library".into(),
            statuses,
            demo: true,
            source: None,
            write_issue: None,
            conflicts: Default::default(),
            media_version: 0,
            selected: None,
            scope: Scope::All,
            query: String::new(),
            search_keys,
        }
    }

    pub fn from_loaded(loaded: &LoadedLibrary) -> Self {
        let games = loaded
            .games
            .iter()
            .map(|record| {
                let personal = &record.game.personal;
                Game {
                    id: record.game_id,
                    record: Some(record.clone()),
                    title: record.game.title.clone(),
                    cover: "covers/missing.jpg".into(),
                    cover_path: loaded.covers.get(&record.game_id).cloned(),
                    description: record.game.description().unwrap_or("").into(),
                    status: personal.status.clone(),
                    status_label: loaded
                        .manifest
                        .definitions
                        .status(&personal.status)
                        .map(|status| status.label.clone())
                        .unwrap_or_else(|| personal.status.clone()),
                    collections: loaded
                        .manifest
                        .definitions
                        .collections
                        .iter()
                        .filter(|collection| {
                            !collection.archived && personal.collections.contains(&collection.id)
                        })
                        .map(|collection| collection.name.clone())
                        .collect(),
                    rating: personal.rating,
                    tags: personal.tags.clone(),
                    favorite: personal.favorite,
                    playtime_minutes: record
                        .game
                        .steam
                        .as_ref()
                        .map_or(0, |steam| steam.playtime_minutes),
                }
            })
            .collect();
        let mut library = Self::new(games);
        library.statuses = loaded.manifest.definitions.statuses.clone();
        library.name = loaded.manifest.definitions.name.clone();
        library.demo = false;
        library.source = Some((loaded.root.clone(), loaded.manifest.clone()));
        library.write_issue = loaded.write_issue.clone();
        library.conflicts = loaded.conflicts.clone();
        library.media_version = loaded.media_version;
        library
    }

    pub fn scope_label(&self, scope: &Scope) -> String {
        match scope {
            Scope::Collection(id) => self
                .source
                .as_ref()
                .and_then(|(_, m)| m.definitions.collections.iter().find(|c| c.id == *id))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Collection".into()),
            Scope::All => "All games".into(),
            Scope::Favorites => "Favorites".into(),
            Scope::Status(key) => self
                .statuses
                .iter()
                .find(|status| status.key == *key)
                .map(|status| status.label.clone())
                .unwrap_or_else(|| key.clone()),
        }
    }

    /// Preserve selection and viewport when a refresh only changes metadata.
    pub fn replace(&mut self, mut next: Self, same_folder: bool) {
        if same_folder {
            next.query = self.query.clone();
            next.scope = self.scope.clone();
            next.selected = self.selected;
            if let Scope::Status(key) = &next.scope {
                if !next.statuses.iter().any(|status| status.key == *key) {
                    next.scope = Scope::All;
                }
            }
            if let Scope::Collection(id) = &next.scope {
                if !next.source.as_ref().is_some_and(|(_, m)| {
                    m.definitions
                        .collections
                        .iter()
                        .any(|c| c.id == *id && !c.archived)
                }) {
                    next.scope = Scope::All;
                }
            }
            next.recompute();
            if next
                .games
                .iter()
                .map(|game| game.id)
                .eq(self.games.iter().map(|game| game.id))
                && next.visible == self.visible
            {
                next.visible = self.visible.clone();
            }
        }
        *self = next;
    }

    pub fn set_scope(&mut self, scope: Scope) {
        self.scope = scope;
        self.recompute();
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
        self.recompute();
    }

    pub fn count(&self, scope: &Scope) -> usize {
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
        assert_eq!(lib.selected, Some(Uuid::from_u128(1145360)));
        lib.set_scope(Scope::Status("completed".into()));
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
        let total = lib.count(&Scope::All);
        lib.set_query("puzzle");
        assert!(!lib.visible.is_empty());
        assert!(lib.visible.len() < total);
        assert_eq!(lib.count(&Scope::All), total);
    }

    #[test]
    fn refresh_keeps_selection_search_and_viewport_identity() {
        let mut lib = library();
        lib.set_query("hades");
        lib.select_slot(0);
        let visible = lib.visible.clone();
        let mut refreshed = library();
        let hades = refreshed
            .games
            .iter_mut()
            .find(|game| game.title == "Hades")
            .unwrap();
        hades.rating = Some(7);
        lib.replace(refreshed, true);
        assert_eq!(lib.visible.len(), 1);
        assert_eq!(lib.selected_game().unwrap().rating, Some(7));
        assert!(Arc::ptr_eq(&visible, &lib.visible));
        lib.replace(library(), false);
        assert_eq!(lib.visible.len(), 12);
        assert!(lib.selected.is_none());
    }

    #[test]
    fn custom_status_filters_use_keys_and_display_manifest_labels() {
        let mut lib = library();
        lib.statuses.push(StatusDefinition {
            key: "weekend".into(),
            label: "For the weekend".into(),
            recommendation_eligible: true,
            extra: Default::default(),
        });
        lib.games[0].status = "weekend".into();
        lib.set_scope(Scope::Status("weekend".into()));
        assert_eq!(lib.visible.len(), 1);
        assert_eq!(lib.scope_label(&lib.scope), "For the weekend");
    }
}
