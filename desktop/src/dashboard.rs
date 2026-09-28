//! Home dashboard data. Computed from loaded games only: no disk or network work.
//! Hidden games are excluded from every panel.

use crate::model::{Game, Scope};
use uuid::Uuid;

pub const RECENT_LIMIT: usize = 8;
pub const FAVORITE_LIMIT: usize = 8;
pub const GENRE_LIMIT: usize = 5;
pub const COLLECTION_COVERS: usize = 3;

/// Steam playtime totals in minutes. `other` is play that Steam does not
/// assign to a platform, for example offline play.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlatformTotals {
    pub windows: u64,
    pub mac: u64,
    pub linux: u64,
    pub deck: u64,
    pub other: u64,
}

impl PlatformTotals {
    pub fn rows(&self) -> [(&'static str, u64); 5] {
        [
            ("Windows", self.windows),
            ("macOS", self.mac),
            ("Linux", self.linux),
            ("Steam Deck", self.deck),
            ("Other", self.other),
        ]
    }

    pub fn total(&self) -> u64 {
        self.rows().iter().map(|(_, minutes)| minutes).sum()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollectionTile {
    pub id: Uuid,
    pub name: String,
    pub count: usize,
    /// Game indices for the tile covers, most played first.
    pub covers: Vec<usize>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Dashboard {
    /// Games with Steam data. Zero means there is nothing to sync yet.
    pub steam_games: usize,
    /// Game indices, most recently played first.
    pub recent: Vec<usize>,
    pub platforms: PlatformTotals,
    /// Game indices: favorites by rating, then playtime.
    pub favorites: Vec<usize>,
    /// Genre names and the number of games that count toward them.
    pub genres: Vec<(String, usize)>,
    pub collections: Vec<CollectionTile>,
}

impl Dashboard {
    /// `collections` are the active collection definitions, in sidebar order.
    pub fn build(games: &[Game], collections: &[(Uuid, String)]) -> Self {
        let shown: Vec<usize> = (0..games.len())
            .filter(|&i| Scope::All.contains(&games[i]))
            .collect();
        let steam = |i: usize| games[i].record.as_ref()?.game.steam.as_ref();

        let mut recent: Vec<(i64, usize)> = shown
            .iter()
            .filter_map(|&i| Some((steam(i)?.last_played?, i)))
            .collect();
        recent.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut platforms = PlatformTotals::default();
        for steam in shown.iter().filter_map(|&i| steam(i)) {
            let by = steam.platform_minutes;
            let assigned = by.windows + by.mac + by.linux + by.deck;
            platforms.windows += u64::from(by.windows);
            platforms.mac += u64::from(by.mac);
            platforms.linux += u64::from(by.linux);
            platforms.deck += u64::from(by.deck);
            platforms.other += u64::from(steam.playtime_minutes.saturating_sub(assigned));
        }

        let mut favorites: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|&i| games[i].favorite)
            .collect();
        favorites.sort_by(|&a, &b| {
            let (a, b) = (&games[a], &games[b]);
            b.rating
                .cmp(&a.rating)
                .then(b.playtime_minutes.cmp(&a.playtime_minutes))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });

        let collections = collections
            .iter()
            .map(|(id, name)| {
                let mut members: Vec<usize> = shown
                    .iter()
                    .copied()
                    .filter(|&i| {
                        games[i]
                            .record
                            .as_ref()
                            .is_some_and(|r| r.game.personal.collections.contains(id))
                    })
                    .collect();
                let count = members.len();
                members.sort_by(|&a, &b| {
                    games[b]
                        .playtime_minutes
                        .cmp(&games[a].playtime_minutes)
                        .then(a.cmp(&b))
                });
                members.truncate(COLLECTION_COVERS);
                CollectionTile {
                    id: *id,
                    name: name.clone(),
                    count,
                    covers: members,
                }
            })
            .collect();

        Self {
            steam_games: shown.iter().filter(|&&i| steam(i).is_some()).count(),
            recent: recent
                .into_iter()
                .take(RECENT_LIMIT)
                .map(|(_, i)| i)
                .collect(),
            platforms,
            favorites: favorites.into_iter().take(FAVORITE_LIMIT).collect(),
            genres: favorite_genres(games, &shown),
            collections,
        }
    }
}

/// Genres of games the user likes: favorites or 4 stars and up. Playtime
/// breaks ties. Without such games, genres of played games rank by playtime.
fn favorite_genres(games: &[Game], shown: &[usize]) -> Vec<(String, usize)> {
    let liked: Vec<usize> = shown
        .iter()
        .copied()
        .filter(|&i| games[i].favorite || games[i].rating.is_some_and(|r| r >= 8))
        .collect();
    let (pool, by_playtime) = if liked.is_empty() {
        let played = shown
            .iter()
            .copied()
            .filter(|&i| games[i].playtime_minutes > 0)
            .collect();
        (played, true)
    } else {
        (liked, false)
    };
    // Genre -> (games, minutes). Keep the first spelling seen.
    let mut totals = std::collections::HashMap::<String, (usize, u64)>::new();
    for i in pool {
        let Some(metadata) = games[i]
            .record
            .as_ref()
            .and_then(|r| r.game.steam.as_ref()?.metadata.as_ref())
        else {
            continue;
        };
        let mut seen = std::collections::HashSet::new();
        for genre in metadata.genres.iter().map(|g| g.trim()) {
            if genre.is_empty() || !seen.insert(genre) {
                continue;
            }
            let entry = totals.entry(genre.to_owned()).or_default();
            entry.0 += 1;
            entry.1 += u64::from(games[i].playtime_minutes);
        }
    }
    let mut genres: Vec<_> = totals.into_iter().collect();
    genres.sort_by(|(a, (a_games, a_minutes)), (b, (b_games, b_minutes))| {
        let order = if by_playtime {
            b_minutes.cmp(a_minutes).then(b_games.cmp(a_games))
        } else {
            b_games.cmp(a_games).then(b_minutes.cmp(a_minutes))
        };
        order.then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    genres
        .into_iter()
        .take(GENRE_LIMIT)
        .map(|(name, (count, _))| (name, count))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamesync_desktop::records::{
        GameData, GameRevision, PlatformMinutes, SteamData, SteamMetadata, SCHEMA_VERSION,
    };

    struct Spec {
        title: &'static str,
        favorite: bool,
        rating: Option<u8>,
        hidden: bool,
        minutes: Option<u32>,
        last_played: Option<i64>,
        genres: &'static [&'static str],
        collection: Option<Uuid>,
    }

    const BASE: Spec = Spec {
        title: "",
        favorite: false,
        rating: None,
        hidden: false,
        minutes: None,
        last_played: None,
        genres: &[],
        collection: None,
    };

    fn game(spec: Spec) -> Game {
        let mut data = GameData::new(spec.title);
        data.personal.favorite = spec.favorite;
        data.personal.rating = spec.rating;
        data.personal.hidden = spec.hidden;
        data.personal.collections.extend(spec.collection);
        data.steam = spec.minutes.map(|minutes| SteamData {
            app_id: 1,
            description: None,
            playtime_minutes: minutes,
            owned: true,
            last_played: spec.last_played,
            platform_minutes: PlatformMinutes {
                linux: minutes / 2,
                deck: minutes / 4,
                ..Default::default()
            },
            wishlist: None,
            metadata: Some(SteamMetadata {
                genres: spec.genres.iter().map(|g| g.to_string()).collect(),
                ..Default::default()
            }),
            extra: Default::default(),
        });
        let record = GameRevision {
            schema_version: SCHEMA_VERSION,
            game_id: Uuid::new_v4(),
            revision_id: Uuid::new_v4(),
            parents: vec![],
            deleted: false,
            game: data,
            extra: Default::default(),
        };
        Game {
            id: record.game_id,
            title: spec.title.into(),
            cover: String::new(),
            description: String::new(),
            status: "backlog".into(),
            status_label: "Backlog".into(),
            collections: vec![],
            cover_path: None,
            rating: spec.rating,
            tags: vec![],
            playtime_minutes: spec.minutes.unwrap_or(0),
            favorite: spec.favorite,
            record: Some(record),
        }
    }

    #[test]
    fn panels_order_games_and_exclude_hidden_ones() {
        let shelf = Uuid::new_v4();
        let games = vec![
            game(Spec {
                title: "Old",
                minutes: Some(100),
                last_played: Some(10),
                favorite: true,
                rating: Some(8),
                genres: &["Puzzle"],
                collection: Some(shelf),
                ..BASE
            }),
            game(Spec {
                title: "New",
                minutes: Some(40),
                last_played: Some(30),
                favorite: true,
                rating: Some(10),
                genres: &["Puzzle", "Action"],
                collection: Some(shelf),
                ..BASE
            }),
            game(Spec {
                title: "Secret",
                minutes: Some(500),
                last_played: Some(99),
                favorite: true,
                hidden: true,
                genres: &["Horror"],
                collection: Some(shelf),
                ..BASE
            }),
            game(Spec {
                title: "Manual",
                ..BASE
            }),
        ];
        let dash = Dashboard::build(&games, &[(shelf, "Shelf".into())]);
        assert_eq!(dash.steam_games, 2);
        assert_eq!(dash.recent, [1, 0]);
        assert_eq!(dash.favorites, [1, 0]);
        assert_eq!(
            dash.genres,
            [("Puzzle".to_string(), 2), ("Action".to_string(), 1)]
        );
        // 140 minutes: half Linux, a quarter Deck, the rest unassigned.
        assert_eq!(
            (
                dash.platforms.linux,
                dash.platforms.deck,
                dash.platforms.other
            ),
            (70, 35, 35)
        );
        assert_eq!(dash.platforms.total(), 140);
        assert_eq!(dash.collections[0].count, 2);
        assert_eq!(dash.collections[0].covers, [0, 1]);
    }

    #[test]
    fn genres_fall_back_to_playtime_without_liked_games() {
        let games = vec![
            game(Spec {
                title: "A",
                minutes: Some(10),
                genres: &["Puzzle"],
                ..BASE
            }),
            game(Spec {
                title: "B",
                minutes: Some(20),
                genres: &["Puzzle"],
                ..BASE
            }),
            game(Spec {
                title: "C",
                minutes: Some(900),
                genres: &["RPG"],
                ..BASE
            }),
            game(Spec {
                title: "D",
                minutes: Some(0),
                genres: &["Unplayed"],
                ..BASE
            }),
        ];
        let dash = Dashboard::build(&games, &[]);
        assert_eq!(
            dash.genres,
            [("RPG".to_string(), 1), ("Puzzle".to_string(), 2)]
        );
    }

    #[test]
    fn an_empty_library_has_empty_panels() {
        assert_eq!(Dashboard::build(&[], &[]), Dashboard::default());
    }
}
