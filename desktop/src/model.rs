//! Game data and filtering. This module has no UI or filesystem dependencies.

use crate::settings::{GroupBy, LibraryDisplay, SortBy};
use serde::Deserialize;
use std::sync::Arc;

use gamesync_desktop::{
    library::{LibraryDefinitions, StatusDefinition},
    library_reader::LoadedLibrary,
    prices::{PriceCache, Quote},
    smart::{self, PlaytimeBand, RatingBand, SmartKind, SmartRule},
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
    /// Landscape Steam header for the open book's left page.
    pub banner_path: Option<PathBuf>,
    /// Half-star units, from 1 to 10. None means unrated.
    pub rating: Option<u8>,
    pub tags: Vec<String>,
    pub playtime_minutes: u32,
    pub favorite: bool,
    #[serde(skip)]
    pub record: Option<gamesync_desktop::records::GameRevision>,
    /// Store price for a wishlisted game. `None` means not fetched yet.
    #[serde(skip)]
    pub price: Option<Quote>,
}

impl Game {
    /// Valid manual board position. Invalid ranks from other writers count as none.
    pub fn board_rank(&self) -> Option<&str> {
        self.record
            .as_ref()?
            .game
            .personal
            .board_rank
            .as_deref()
            .filter(|rank| gamesync_desktop::board::valid_rank(rank))
    }

    /// Wishlisted games are not owned: no status, rating, favorite, or
    /// collections. Views show price and sale details instead.
    pub fn wishlisted(&self) -> bool {
        self.record.as_ref().is_some_and(|r| r.game.wishlisted())
    }

    /// Localized price, discount savings, and time-sensitive sale state.
    pub fn price_parts(&self) -> gamesync_desktop::prices::PriceDisplay {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        match &self.price {
            Some(quote) => quote.display(now),
            None => gamesync_desktop::prices::PriceDisplay {
                price: "Price loading…".into(),
                discount: None,
                sale_ends_soon: false,
                sale_label: None,
            },
        }
    }

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
    Hidden,
    /// Wishlisted games that are not owned. They appear in no other scope.
    Wishlist,
    Collection(Uuid),
    /// Computed membership. See `gamesync_desktop::smart`.
    Smart(SmartRule),
}

/// Sorts that apply only in the Wishlist scope. Each has one natural
/// direction, so the Ascending and Descending choices do not apply.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WishlistSort {
    /// Steam wishlist order.
    #[default]
    Priority,
    /// Lowest first; games without a price last.
    Price,
    /// Largest discount first.
    Discount,
    /// Newest first.
    Added,
    Name,
}

impl WishlistSort {
    pub const ALL: [Self; 5] = [
        Self::Priority,
        Self::Price,
        Self::Discount,
        Self::Added,
        Self::Name,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Priority => "Wishlist order",
            Self::Price => "Price",
            Self::Discount => "Discount",
            Self::Added => "Date added",
            Self::Name => "Name",
        }
    }
}

fn wishlist_entry(game: &Game) -> Option<gamesync_desktop::records::WishlistEntry> {
    game.record.as_ref()?.game.steam.as_ref()?.wishlist
}

/// One sidebar smart group with its values and game counts. Rating and time
/// bands keep their fixed order; text values are sorted by count, then name.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartGroup {
    pub kind: SmartKind,
    pub values: Vec<(SmartRule, usize)>,
}

/// Counts cover games in All games, so hidden games are excluded.
fn smart_groups(games: &[Game]) -> Vec<SmartGroup> {
    let band_order = |rule: &SmartRule| match rule {
        SmartRule::BestOn(band) => gamesync_desktop::suitability::BestOn::ALL
            .iter()
            .position(|b| b == band),
        SmartRule::Rating(band) => RatingBand::ALL.iter().position(|b| b == band),
        SmartRule::Playtime(band) => PlaytimeBand::ALL.iter().position(|b| b == band),
        _ => None,
    };
    SmartKind::ALL
        .iter()
        .map(|&kind| {
            let mut counts = std::collections::HashMap::<SmartRule, usize>::new();
            for game in games.iter().filter(|game| Scope::All.contains(game)) {
                if let Some(record) = &game.record {
                    for rule in smart::rules_for(&record.game, kind) {
                        *counts.entry(rule).or_default() += 1;
                    }
                }
            }
            let mut values: Vec<_> = counts.into_iter().collect();
            values.sort_by(|(a, a_count), (b, b_count)| {
                band_order(a)
                    .cmp(&band_order(b))
                    .then(b_count.cmp(a_count))
                    .then_with(|| a.label().to_lowercase().cmp(&b.label().to_lowercase()))
            });
            SmartGroup { kind, values }
        })
        .collect()
}

impl Scope {
    /// Stable device-local key for this section's presentation preference.
    pub fn view_key(&self) -> String {
        match self {
            Self::All => "all".into(),
            Self::Favorites => "favorites".into(),
            Self::Hidden => "hidden".into(),
            Self::Wishlist => "wishlist".into(),
            Self::Collection(id) => format!("collection:{id}"),
            Self::Smart(rule) => format!(
                "smart:{}",
                serde_json::to_string(rule).expect("smart rules are serializable")
            ),
        }
    }

    pub fn contains(&self, game: &Game) -> bool {
        let hidden = game.record.as_ref().is_some_and(|r| r.game.personal.hidden);
        if matches!(self, Self::Hidden) {
            return hidden;
        }
        if hidden {
            return false;
        }
        let wishlisted = game.record.as_ref().is_some_and(|r| r.game.wishlisted());
        match self {
            Self::Hidden => unreachable!(),
            Self::Wishlist => wishlisted,
            _ if wishlisted => false,
            Self::Collection(id) => game
                .record
                .as_ref()
                .is_some_and(|r| r.game.personal.collections.contains(id)),
            Self::Smart(rule) => game.record.as_ref().is_some_and(|r| rule.matches(&r.game)),
            Self::All => true,
            Self::Favorites => game.favorite,
        }
    }
}

/// One board column. `key` is `None` for games whose status is not defined here,
/// for example a status added on another computer before its definitions arrive.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardColumn {
    pub key: Option<String>,
    pub label: String,
    /// Game indices, ranked games first, then the Sort menu order.
    pub games: Vec<usize>,
}

/// One selection shared by the grid and inspector. IDs survive sorting/filtering.
pub struct Library {
    /// Explicit, in-memory prototype. Never inferred from a library name or path.
    pub best_on_demo: bool,
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
    /// Home is a page over the whole library, not a scope. The app opens on it.
    /// `scope` keeps the last library scope for when the user leaves Home.
    pub home: bool,
    pub play_now: bool,
    pub show_hidden_games: bool,
    pub display: LibraryDisplay,
    pub filter_status: Option<String>,
    pub filter_collection: Option<Uuid>,
    pub filter_favorites: bool,
    /// Applies only in the Wishlist scope.
    pub wishlist_sort: WishlistSort,
    /// Device-local prices. See `gamesync_desktop::prices`.
    pub price_cache: Arc<PriceCache>,
    pub groups: Vec<(String, Vec<usize>)>,
    /// Refreshed when game data changes, not on every search or filter change.
    pub smart: Vec<SmartGroup>,
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
        let visible = Arc::new(
            games
                .iter()
                .enumerate()
                .filter_map(|(index, game)| Scope::All.contains(game).then_some(index))
                .collect(),
        );
        let smart = smart_groups(&games);
        Self {
            games,
            best_on_demo: false,
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
            home: true,
            play_now: false,
            show_hidden_games: false,
            display: LibraryDisplay::default(),
            filter_status: None,
            filter_collection: None,
            filter_favorites: false,
            wishlist_sort: WishlistSort::default(),
            price_cache: Default::default(),
            groups: Vec::new(),
            smart,
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
                    banner_path: loaded.banners.get(&record.game_id).cloned(),
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
                    price: None,
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
            Scope::Hidden => "Hidden games".into(),
            Scope::Wishlist => "Wishlist".into(),
            Scope::Smart(rule) => rule.label().into(),
        }
    }

    /// Preserve selection and viewport when a refresh only changes metadata.
    pub fn replace(&mut self, mut next: Self, same_folder: bool) {
        next.show_hidden_games = self.show_hidden_games;
        next.home = self.home;
        next.play_now = self.play_now;
        next.price_cache = self.price_cache.clone();
        next.apply_quotes();
        next.display = self.display.clone();
        next.best_on_demo = self.best_on_demo;
        // The Best on demo opens on its Steam Deck collection, before its
        // fresh store first loads.
        if self.best_on_demo && !same_folder {
            next.scope = self.scope.clone();
        }
        if same_folder {
            next.filter_status = self.filter_status.clone();
            next.filter_collection = self.filter_collection;
            next.filter_favorites = self.filter_favorites;
            next.wishlist_sort = self.wishlist_sort;
            next.query = self.query.clone();
            next.scope = self.scope.clone();
            next.selected = self.selected;
            if next
                .filter_status
                .as_ref()
                .is_some_and(|key| !next.statuses.iter().any(|status| status.key == *key))
            {
                next.filter_status = None;
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
        }
        next.recompute();
        if same_folder
            && next
                .games
                .iter()
                .map(|g| g.id)
                .eq(self.games.iter().map(|g| g.id))
            && next.visible == self.visible
        {
            next.visible = self.visible.clone();
        }
        *self = next;
    }

    /// Apply saved library definitions in one step. The sidebar, board, menus,
    /// and game labels all read these values, so they cannot disagree.
    pub fn apply_definitions(
        &mut self,
        root: PathBuf,
        saved: gamesync_desktop::library::LibraryRevision,
    ) {
        self.statuses = saved.definitions.statuses.clone();
        for game in &mut self.games {
            game.status_label = saved
                .definitions
                .status(&game.status)
                .map(|status| status.label.clone())
                .unwrap_or_else(|| game.status.clone());
            game.collections = saved
                .definitions
                .collections
                .iter()
                .filter(|c| {
                    !c.archived
                        && game
                            .record
                            .as_ref()
                            .is_some_and(|r| r.game.personal.collections.contains(&c.id))
                })
                .map(|c| c.name.clone())
                .collect();
        }
        let scope_exists = match &self.scope {
            Scope::Collection(id) => saved
                .definitions
                .collections
                .iter()
                .any(|c| c.id == *id && !c.archived),
            _ => true,
        };
        if !scope_exists {
            self.scope = Scope::All;
        }
        if self
            .filter_status
            .as_ref()
            .is_some_and(|key| saved.definitions.status(key).is_none())
        {
            self.filter_status = None;
        }
        self.source = Some((root, saved));
        self.recompute();
    }

    /// Columns in status definition order over the visible games, so the board
    /// follows the sidebar scope, search, and filters like the other views.
    pub fn board_columns(&self) -> Vec<BoardColumn> {
        let column = |key: Option<&str>| {
            let mut games: Vec<usize> = self
                .visible
                .iter()
                .copied()
                .filter(|&i| {
                    let status = self.games[i].status.as_str();
                    key.map_or_else(
                        || !self.statuses.iter().any(|s| s.key == status),
                        |key| key == status,
                    )
                })
                .collect();
            if !self.display.board_manual {
                // `visible` is already in the Sort menu order.
                return games;
            }
            // Stable: equal ranks and unranked games keep the Sort menu order.
            games.sort_by(|&a, &b| {
                match (self.games[a].board_rank(), self.games[b].board_rank()) {
                    (Some(a), Some(b)) => a.cmp(b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
            games
        };
        let mut columns: Vec<_> = self
            .statuses
            .iter()
            .map(|status| BoardColumn {
                key: Some(status.key.clone()),
                label: status.label.clone(),
                games: column(Some(&status.key)),
            })
            .collect();
        let other = column(None);
        if !other.is_empty() {
            columns.push(BoardColumn {
                key: None,
                label: "Other statuses".into(),
                games: other,
            });
        }
        columns
    }

    pub fn set_show_hidden_games(&mut self, show: bool) {
        self.show_hidden_games = show;
        if !show && self.scope == Scope::Hidden {
            // Not `set_scope`: a settings change must not leave Home.
            self.scope = Scope::All;
            self.recompute();
        }
    }

    pub fn apply_personal_record(&mut self, record: gamesync_desktop::records::GameRevision) {
        self.apply_personal_records(vec![record]);
    }

    pub fn apply_personal_records(
        &mut self,
        records: Vec<gamesync_desktop::records::GameRevision>,
    ) {
        for record in records {
            if let Some(game) = self.games.iter_mut().find(|game| game.id == record.game_id) {
                let personal = &record.game.personal;
                game.favorite = personal.favorite;
                game.status = personal.status.clone();
                game.status_label = self
                    .statuses
                    .iter()
                    .find(|s| s.key == personal.status)
                    .map(|s| s.label.clone())
                    .unwrap_or_else(|| personal.status.clone());
                game.rating = personal.rating;
                game.tags = personal.tags.clone();
                game.collections = self
                    .source
                    .as_ref()
                    .map(|(_, m)| {
                        m.definitions
                            .collections
                            .iter()
                            .filter(|c| !c.archived && personal.collections.contains(&c.id))
                            .map(|c| c.name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                game.record = Some(record);
            }
        }
        self.search_keys = self
            .games
            .iter()
            .map(|g| format!("{} {}", g.title, g.tags.join(" ")).to_lowercase())
            .collect();
        self.smart = smart_groups(&self.games);
        self.recompute();
    }

    /// Best on rules or enrichment changed: recount the sidebar bands and
    /// refilter, since a Best on scope can now match other games.
    pub fn refresh_best_on(&mut self) {
        self.smart = smart_groups(&self.games);
        self.recompute();
    }

    pub fn set_scope(&mut self, scope: Scope) {
        self.home = false;
        self.play_now = false;
        self.scope = scope;
        self.recompute();
    }

    /// App IDs that need prices: every wishlisted game, including removed ones.
    pub fn wishlist_app_ids(&self) -> Vec<u32> {
        self.games
            .iter()
            .filter_map(|game| {
                let record = game.record.as_ref()?;
                record
                    .game
                    .wishlisted()
                    .then(|| record.game.steam.as_ref().map(|s| s.app_id))?
            })
            .collect()
    }

    pub fn set_prices(&mut self, cache: PriceCache) {
        self.price_cache = Arc::new(cache);
        self.apply_quotes();
        self.recompute();
    }

    fn apply_quotes(&mut self) {
        for game in &mut self.games {
            game.price = game
                .record
                .as_ref()
                .filter(|record| record.game.wishlisted())
                .and_then(|record| record.game.steam.as_ref())
                .and_then(|steam| self.price_cache.quotes.get(&steam.app_id).cloned());
        }
    }

    pub fn show_home(&mut self) {
        self.home = true;
        self.play_now = false;
    }

    pub fn show_play_now(&mut self) {
        self.home = false;
        self.play_now = true;
    }

    /// Select a game from Home. Home ignores the library scope, search, and
    /// filters, so clear them; otherwise the selection could be filtered out.
    /// Home stays open behind the game card.
    pub fn select_from_home(&mut self, id: Uuid) {
        self.scope = Scope::All;
        self.query.clear();
        self.filter_status = None;
        self.filter_collection = None;
        self.filter_favorites = false;
        self.selected = Some(id);
        self.recompute();
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
        self.recompute();
    }

    pub fn count(&self, scope: &Scope) -> usize {
        if let Scope::Smart(rule) = scope {
            return self
                .smart
                .iter()
                .flat_map(|group| &group.values)
                .find(|(value, _)| value == rule)
                .map_or(0, |(_, count)| *count);
        }
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

    pub fn recompute(&mut self) {
        self.visible = Arc::new(
            self.games
                .iter()
                .enumerate()
                .filter_map(|(index, game)| {
                    (self.scope.contains(game)
                        && self.search_keys[index].contains(&self.query)
                        && (self.scope == Scope::Wishlist
                            || (self
                                .filter_status
                                .as_ref()
                                .is_none_or(|status| &game.status == status)
                                && self.filter_collection.is_none_or(|id| {
                                    game.record
                                        .as_ref()
                                        .is_some_and(|r| r.game.personal.collections.contains(&id))
                                })
                                && (!self.filter_favorites || game.favorite))))
                        .then_some(index)
                })
                .collect(),
        );
        let status_rank = |game: &Game| {
            self.statuses
                .iter()
                .position(|s| s.key == game.status)
                .unwrap_or(usize::MAX)
        };
        let collection_key = |game: &Game| game.collections.iter().map(|s| s.to_lowercase()).min();
        let wishlist = self.scope == Scope::Wishlist;
        Arc::make_mut(&mut self.visible).sort_by(|&a, &b| {
            let (a, b) = (&self.games[a], &self.games[b]);
            if wishlist {
                // `None` sorts last in every wishlist order.
                let last = |value: Option<u64>| value.unwrap_or(u64::MAX);
                let order = match self.wishlist_sort {
                    WishlistSort::Priority => last(wishlist_entry(a).map(|w| w.priority.into()))
                        .cmp(&last(wishlist_entry(b).map(|w| w.priority.into()))),
                    WishlistSort::Price => last(a.price.as_ref().and_then(Quote::cents))
                        .cmp(&last(b.price.as_ref().and_then(Quote::cents))),
                    WishlistSort::Discount => b
                        .price
                        .as_ref()
                        .map_or(0, Quote::discount)
                        .cmp(&a.price.as_ref().map_or(0, Quote::discount)),
                    WishlistSort::Added => wishlist_entry(b)
                        .map(|w| w.added)
                        .cmp(&wishlist_entry(a).map(|w| w.added)),
                    WishlistSort::Name => std::cmp::Ordering::Equal,
                };
                return order
                    .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                    .then(a.id.cmp(&b.id));
            }
            let order = match self.display.sort {
                SortBy::Name => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortBy::Status => status_rank(a).cmp(&status_rank(b)),
                SortBy::Hours => a.playtime_minutes.cmp(&b.playtime_minutes),
                SortBy::Collection => collection_key(a).cmp(&collection_key(b)),
            };
            let order = if self.display.descending {
                order.reverse()
            } else {
                order
            };
            order
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                .then(a.id.cmp(&b.id))
        });
        self.groups.clear();
        match (wishlist, self.display.group) {
            // Games that left the Steam wishlist stay apart until the user archives them.
            (true, _) => {
                let removed = |i: usize| {
                    self.games[i].record.as_ref().is_some_and(|r| {
                        r.game
                            .steam
                            .as_ref()
                            .and_then(|s| s.wishlist)
                            .is_some_and(|w| w.removed)
                    })
                };
                let (gone, listed): (Vec<usize>, Vec<usize>) =
                    (0..self.visible.len()).partition(|&slot| removed(self.visible[slot]));
                if !gone.is_empty() {
                    if !listed.is_empty() {
                        self.groups.push(("On wishlist".into(), listed));
                    }
                    self.groups
                        .push(("Removed from Steam wishlist".into(), gone));
                }
            }
            (false, GroupBy::None) => {}
            (false, GroupBy::Status) => {
                for status in &self.statuses {
                    let slots: Vec<_> = self
                        .visible
                        .iter()
                        .enumerate()
                        .filter_map(|(slot, &i)| {
                            (self.games[i].status == status.key).then_some(slot)
                        })
                        .collect();
                    if !slots.is_empty() {
                        self.groups.push((status.label.clone(), slots));
                    }
                }
                let unknown: Vec<_> = self
                    .visible
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, &i)| {
                        (!self.statuses.iter().any(|s| s.key == self.games[i].status))
                            .then_some(slot)
                    })
                    .collect();
                if !unknown.is_empty() {
                    self.groups.push(("Other statuses".into(), unknown));
                }
            }
            (false, GroupBy::Collections) => {
                let mut groups = std::collections::BTreeMap::<String, Vec<usize>>::new();
                let mut uncollected = Vec::new();
                for (slot, &i) in self.visible.iter().enumerate() {
                    let game = &self.games[i];
                    if game.collections.is_empty() {
                        uncollected.push(slot);
                    }
                    for name in &game.collections {
                        groups.entry(name.clone()).or_default().push(slot);
                    }
                }
                self.groups.extend(groups);
                if !uncollected.is_empty() {
                    self.groups.push(("No collection".into(), uncollected));
                }
            }
        }
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
    fn display_sort_filters_groups_and_refresh_compose() {
        let mut lib = library();
        lib.display.sort = SortBy::Hours;
        lib.display.descending = true;
        lib.display.group = GroupBy::Status;
        lib.recompute();
        assert!(lib
            .visible
            .windows(2)
            .all(|w| lib.games[w[0]].playtime_minutes >= lib.games[w[1]].playtime_minutes));
        assert_eq!(
            lib.groups
                .iter()
                .map(|(_, slots)| slots.len())
                .sum::<usize>(),
            lib.visible.len()
        );
        lib.select_slot(0);
        let selected = lib.selected;
        lib.display.sort = SortBy::Name;
        lib.recompute();
        assert_eq!(lib.selected, selected);
        lib.filter_favorites = true;
        lib.filter_status = Some("playing".into());
        lib.recompute();
        assert!(lib
            .visible
            .iter()
            .all(|&i| lib.games[i].favorite && lib.games[i].status == "playing"));
        lib.replace(library(), true);
        assert!(lib.filter_favorites);
        assert_eq!(lib.display.group, GroupBy::Status);
        lib.replace(library(), false);
        assert!(!lib.filter_favorites);
        assert_eq!(lib.display.group, GroupBy::Status);
        assert!(!lib.groups.is_empty());
    }

    #[test]
    fn status_and_collection_sort_use_definition_order_and_first_collection() {
        let mut lib = library();
        lib.display.sort = SortBy::Status;
        lib.recompute();
        let ranks: Vec<_> = lib
            .visible
            .iter()
            .map(|&i| {
                lib.statuses
                    .iter()
                    .position(|s| s.key == lib.games[i].status)
                    .unwrap()
            })
            .collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]));
        lib.games[0].collections = vec!["Zulu".into(), "Alpha".into()];
        lib.games[1].collections = vec!["Beta".into()];
        lib.display.sort = SortBy::Collection;
        lib.recompute();
        assert_eq!(&lib.visible[10..], &[0, 1]);
        lib.display.descending = true;
        lib.recompute();
        assert_eq!(&lib.visible[..2], &[1, 0]);
    }

    #[test]
    fn collection_groups_repeat_members_without_duplicating_results() {
        let mut lib = library();
        lib.games[0].collections = vec!["Cozy".into(), "Weekend".into()];
        lib.display.group = GroupBy::Collections;
        lib.recompute();
        assert_eq!(lib.visible.len(), 12);
        assert_eq!(
            lib.groups.iter().find(|(n, _)| n == "Cozy").unwrap().1,
            vec![0]
        );
        assert_eq!(
            lib.groups.iter().find(|(n, _)| n == "Weekend").unwrap().1,
            vec![0]
        );
        assert_eq!(lib.groups.last().unwrap().0, "No collection");
        lib.set_query("no-match-123");
        assert!(lib.groups.is_empty());
    }

    #[test]
    fn filters_compose_and_clear_hidden_selection() {
        let mut lib = library();
        lib.set_query("hAdEs");
        assert_eq!(lib.visible.len(), 1);
        lib.select_slot(0);
        assert_eq!(lib.selected, Some(Uuid::from_u128(1145360)));
        lib.filter_status = Some("completed".into());
        lib.recompute();
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
        lib.filter_status = Some("weekend".into());
        lib.recompute();
        assert_eq!(lib.visible.len(), 1);
        assert_eq!(lib.board_columns().last().unwrap().label, "For the weekend");
    }
    fn record(
        game: &Game,
        status: &str,
        rank: Option<&str>,
    ) -> gamesync_desktop::records::GameRevision {
        use gamesync_desktop::records::{GameData, GameRevision, SCHEMA_VERSION};
        let mut data = GameData::new(game.title.clone());
        data.personal.status = status.into();
        data.personal.board_rank = rank.map(Into::into);
        GameRevision {
            schema_version: SCHEMA_VERSION,
            game_id: game.id,
            revision_id: Uuid::new_v4(),
            parents: vec![],
            deleted: false,
            game: data,
            extra: Default::default(),
        }
    }

    #[test]
    fn board_columns_follow_statuses_ranks_and_sort_order() {
        let mut lib = library();
        let (a, b, c, d) = (
            lib.games[0].clone(),
            lib.games[1].clone(),
            lib.games[2].clone(),
            lib.games[3].clone(),
        );
        let mut hidden = record(&d, "playing", Some("a"));
        hidden.game.personal.hidden = true;
        lib.apply_personal_records(vec![
            record(&a, "playing", Some("m")),
            record(&b, "playing", Some("c")),
            // An invalid rank from another writer counts as no rank.
            record(&c, "retired", Some("BAD")),
            hidden,
        ]);
        let columns = lib.board_columns();
        let keys: Vec<_> = columns.iter().map(|c| c.key.clone()).collect();
        let mut expected: Vec<_> = lib.statuses.iter().map(|s| Some(s.key.clone())).collect();
        expected.push(None);
        assert_eq!(keys, expected);
        let playing = columns
            .iter()
            .find(|c| c.key.as_deref() == Some("playing"))
            .unwrap();
        let ids: Vec<_> = playing.games.iter().map(|&i| lib.games[i].id).collect();
        assert_eq!(&ids[..2], &[b.id, a.id]);
        assert!(!ids.contains(&d.id));
        // Unranked games keep the Sort menu order after ranked games.
        let rest: Vec<_> = playing.games[2..]
            .iter()
            .map(|&i| lib.games[i].title.to_lowercase())
            .collect();
        assert!(rest.windows(2).all(|w| w[0] <= w[1]));
        let other = columns.last().unwrap();
        assert_eq!(other.label, "Other statuses");
        assert_eq!(other.games.len(), 1);
        assert_eq!(
            columns.iter().map(|c| c.games.len()).sum::<usize>(),
            lib.visible.len()
        );
    }

    #[test]
    fn a_sorted_board_ignores_manual_positions() {
        let mut lib = library();
        let (a, b, c) = (
            lib.games[0].clone(),
            lib.games[1].clone(),
            lib.games[2].clone(),
        );
        lib.apply_personal_records(vec![
            record(&a, "playing", Some("c")),
            record(&b, "playing", Some("m")),
            record(&c, "playing", None),
        ]);
        for (game, minutes) in [(&a, 10), (&b, 300), (&c, 5_000)] {
            let game = lib.games.iter_mut().find(|g| g.id == game.id).unwrap();
            game.playtime_minutes = minutes;
        }
        lib.display.sort = SortBy::Hours;
        lib.display.descending = true;
        let playing = |lib: &Library| -> Vec<Uuid> {
            let column = lib
                .board_columns()
                .into_iter()
                .find(|c| c.key.as_deref() == Some("playing"))
                .unwrap();
            column
                .games
                .iter()
                .map(|&i| lib.games[i].id)
                .filter(|id| [a.id, b.id, c.id].contains(id))
                .collect()
        };
        // Manual order: ranked games first, then the Sort menu order.
        lib.recompute();
        assert_eq!(playing(&lib), [a.id, b.id, c.id]);
        // Sorted: most hours first; the ranks stay saved.
        lib.display.board_manual = false;
        lib.recompute();
        assert_eq!(playing(&lib), [c.id, b.id, a.id]);
        assert!(lib.games.iter().any(|g| g.board_rank() == Some("c")));
    }

    #[test]
    fn saved_definitions_update_statuses_labels_and_scope_together() {
        use gamesync_desktop::library::{LibraryDefinitions, LibraryRevision};
        let mut lib = library();
        let game = lib.games[0].clone();
        lib.apply_personal_record(record(&game, "playing", None));
        let mut definitions = LibraryDefinitions::new("Test");
        let key = gamesync_desktop::board::add_status(&mut definitions, "On hold").unwrap();
        gamesync_desktop::board::rename_status(&mut definitions, "playing", "Now playing").unwrap();
        gamesync_desktop::board::move_status(&mut definitions, &key, false);
        let saved = LibraryRevision {
            schema_version: 1,
            library_id: Uuid::new_v4(),
            revision_id: Uuid::new_v4(),
            parents: vec![],
            definitions,
            extra: Default::default(),
        };
        lib.set_scope(Scope::Collection(Uuid::new_v4()));
        lib.filter_status = Some("gone".into());
        lib.apply_definitions(PathBuf::from("/library"), saved.clone());
        assert_eq!(lib.statuses, saved.definitions.statuses);
        assert_eq!(lib.games[0].status_label, "Now playing");
        assert_eq!(lib.scope, Scope::All);
        assert!(lib.filter_status.is_none());
        let labels: Vec<_> = lib.board_columns().into_iter().map(|c| c.label).collect();
        let sidebar: Vec<_> = lib.statuses.iter().map(|s| s.label.clone()).collect();
        assert_eq!(labels, sidebar);
        assert_eq!(labels[labels.len() - 2], "On hold");
    }

    #[test]
    fn wishlist_games_stay_out_of_the_library_and_removed_ones_group_apart() {
        use gamesync_desktop::records::{SteamData, WishlistEntry};
        let mut lib = library();
        let total = lib.count(&Scope::All);
        let wish = |game: &Game, removed: bool| {
            let mut record = record(game, &game.status, None);
            record.game.personal.favorite = true;
            record.game.personal.tags = vec!["Wanted".into()];
            record.game.steam = Some(SteamData {
                app_id: 1,
                description: None,
                playtime_minutes: 0,
                owned: false,
                last_played: None,
                platform_minutes: Default::default(),
                wishlist: Some(WishlistEntry {
                    priority: 0,
                    added: 0,
                    removed,
                }),
                metadata: None,
                extra: Default::default(),
            });
            record
        };
        let records = vec![wish(&lib.games[0], false), wish(&lib.games[1], true)];
        lib.apply_personal_records(records);
        assert_eq!(lib.count(&Scope::All), total - 2);
        assert_eq!(lib.count(&Scope::Wishlist), 2);
        assert!(!Scope::Favorites.contains(&lib.games[0]));
        // Smart groups and Home use All games, so wishlist tags do not appear.
        assert!(lib
            .smart
            .iter()
            .flat_map(|g| &g.values)
            .all(|(rule, _)| *rule != SmartRule::MyTag("Wanted".into())));
        lib.set_scope(Scope::Wishlist);
        let labels: Vec<_> = lib
            .groups
            .iter()
            .map(|(l, s)| (l.as_str(), s.len()))
            .collect();
        assert_eq!(
            labels,
            [("On wishlist", 1), ("Removed from Steam wishlist", 1)]
        );
    }

    #[test]
    fn wishlist_sorts_by_price_and_discount_and_ignores_library_display() {
        use gamesync_desktop::{
            prices::{Price, PriceCache, Quote},
            records::{SteamData, WishlistEntry},
        };
        let mut lib = library();
        let games: Vec<_> = lib.games[..3].to_vec();
        let records = games
            .iter()
            .enumerate()
            .map(|(i, game)| {
                let mut record = record(game, &game.status, None);
                record.game.steam = Some(SteamData {
                    app_id: i as u32 + 1,
                    description: None,
                    playtime_minutes: 0,
                    owned: false,
                    last_played: None,
                    platform_minutes: Default::default(),
                    wishlist: Some(WishlistEntry {
                        priority: [2, 0, 1][i],
                        added: [10, 30, 20][i],
                        removed: false,
                    }),
                    metadata: None,
                    extra: Default::default(),
                });
                record
            })
            .collect();
        lib.apply_personal_records(records);
        let price = |cents, discount| {
            Quote::Price(Price {
                final_cents: cents,
                original_cents: None,
                discount_pct: discount,
                formatted_final: format!("${cents}"),
                formatted_original: None,
                sale_ends: None,
            })
        };
        assert_eq!(lib.wishlist_app_ids().len(), 3);
        lib.set_prices(PriceCache {
            country: "US".into(),
            fetched_at: 0,
            quotes: [
                (1, price(500, 50)),
                (2, Quote::Unavailable),
                (3, price(100, 0)),
            ]
            .into(),
        });
        lib.set_scope(Scope::Wishlist);
        let order = |lib: &Library| -> Vec<usize> {
            lib.visible
                .iter()
                .map(|&i| games.iter().position(|g| g.id == lib.games[i].id).unwrap())
                .collect()
        };
        assert_eq!(order(&lib), [1, 2, 0]);
        lib.wishlist_sort = WishlistSort::Price;
        lib.recompute();
        assert_eq!(order(&lib), [2, 0, 1]);
        lib.wishlist_sort = WishlistSort::Added;
        lib.recompute();
        assert_eq!(order(&lib), [1, 2, 0]);
        lib.filter_favorites = true;
        lib.display.group = GroupBy::Status;
        lib.recompute();
        assert_eq!(order(&lib), [1, 2, 0]);
        assert!(lib.groups.is_empty());
    }

    #[test]
    fn home_opens_first_and_selecting_from_home_clears_filters() {
        let mut lib = library();
        assert!(lib.home);
        lib.set_scope(Scope::Favorites);
        assert!(!lib.home);
        lib.set_query("zzz-no-match");
        lib.filter_favorites = true;
        lib.show_home();
        // Home selection must survive the old search and filters.
        let id = lib.games[0].id;
        lib.select_from_home(id);
        assert!(lib.home);
        assert_eq!(lib.scope, Scope::All);
        assert_eq!(lib.selected, Some(id));
        assert_eq!(lib.visible.len(), lib.count(&Scope::All));
        // A settings change does not leave Home.
        lib.set_show_hidden_games(false);
        assert!(lib.home);
    }

    #[test]
    fn smart_groups_count_visible_games_and_match_the_scope() {
        let mut lib = library();
        let games: Vec<_> = lib.games[..3].to_vec();
        let records = games
            .iter()
            .enumerate()
            .map(|(i, game)| {
                let mut record = record(game, &game.status, None);
                record.game.personal.tags = vec!["Weekend".into()];
                record.game.personal.rating = Some([10, 8, 10][i]);
                record.game.personal.hidden = i == 2;
                record
            })
            .collect();
        lib.apply_personal_records(records);
        let group = |kind| {
            lib.smart
                .iter()
                .find(|g| g.kind == kind)
                .unwrap()
                .values
                .clone()
        };
        // The hidden game is not counted.
        assert_eq!(
            group(SmartKind::MyTag),
            [(SmartRule::MyTag("Weekend".into()), 2)]
        );
        assert_eq!(
            group(SmartKind::Rating),
            [
                (SmartRule::Rating(RatingBand::Five), 1),
                (SmartRule::Rating(RatingBand::Four), 1)
            ]
        );
        // Games without Steam data have no time played value.
        assert!(group(SmartKind::Playtime).is_empty());

        let scope = Scope::Smart(SmartRule::MyTag("Weekend".into()));
        lib.set_scope(scope.clone());
        assert_eq!(lib.visible.len(), 2);
        assert_eq!(lib.count(&scope), 2);
        assert_eq!(lib.scope_label(&scope), "Weekend");
    }

    #[test]
    fn hidden_games_leave_normal_scopes_and_can_be_restored() {
        use gamesync_desktop::records::{GameData, GameRevision, SCHEMA_VERSION};
        let mut lib = library();
        let total = lib.count(&Scope::All);
        let game = lib.games[0].clone();
        let collection = Uuid::new_v4();
        let mut data = GameData::new(game.title.clone());
        data.personal.status = game.status.clone();
        data.personal.favorite = true;
        data.personal.collections.push(collection);
        data.personal.hidden = true;
        let mut record = GameRevision {
            schema_version: SCHEMA_VERSION,
            game_id: game.id,
            revision_id: Uuid::new_v4(),
            parents: vec![],
            deleted: false,
            game: data,
            extra: Default::default(),
        };
        lib.selected = Some(game.id);
        lib.apply_personal_record(record.clone());
        assert_eq!(lib.count(&Scope::All), total - 1);
        assert_eq!(lib.count(&Scope::Hidden), 1);
        assert_eq!(lib.count(&Scope::Collection(collection)), 0);
        assert!(!Scope::Favorites.contains(&lib.games[0]));
        assert!(lib.selected.is_none());
        lib.set_show_hidden_games(true);
        lib.set_scope(Scope::Hidden);
        assert_eq!(lib.visible.len(), 1);
        lib.set_query("no-match");
        assert!(lib.visible.is_empty());
        lib.set_query("");
        lib.set_show_hidden_games(false);
        assert_eq!(lib.scope, Scope::All);
        record.game.personal.hidden = false;
        lib.apply_personal_record(record);
        assert_eq!(lib.count(&Scope::All), total);
        assert_eq!(lib.count(&Scope::Hidden), 0);
    }

    #[test]
    fn section_view_keys_are_stable_and_distinct() {
        let collection = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        assert_eq!(Scope::All.view_key(), "all");
        assert_eq!(Scope::Favorites.view_key(), "favorites");
        assert_eq!(Scope::Wishlist.view_key(), "wishlist");
        assert_eq!(
            Scope::Collection(collection).view_key(),
            "collection:12345678-1234-5678-1234-567812345678"
        );
        assert_eq!(
            Scope::Smart(SmartRule::MyTag("Weekend".into())).view_key(),
            r#"smart:{"kind":"my_tag","value":"Weekend"}"#
        );
    }
}
