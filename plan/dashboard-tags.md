# Dashboard, Smart collections, and Wishlist

Branch: `dashboard-tags`, created from `main` after PR #1.

## Goal

Add four features on top of the desktop v1 library, in this order:

1. **Steam data:** last played, playtime by platform, and Steam tags.
2. **Smart collections:** a sidebar section with computed groups for genres,
   Steam tags, personal tags, rating, and time played.
3. **Dashboard:** a Home page with recent games, playtime by platform,
   favorite games, favorite genres, and collections.
4. **Wishlist:** sync the Steam wishlist and show current prices.

Each step is a separate, working increment. Each step reuses data from the
steps before it. The user chose this order on September 28.

## 1. Steam data

### Owned-games fields

`GetOwnedGames` already returns these fields. The sync ignores them today.

| Field | Store as | Use |
| --- | --- | --- |
| `rtime_last_played` | `SteamData.last_played` (Unix seconds, `None` when 0) | Recent games, Rediscover |
| `playtime_windows_forever`, `_mac_`, `_linux_`, `_deck_forever` | `SteamData.platform_minutes` | Playtime by platform |

- The owned-games import already updates playtime. It also updates these fields.
- A sync must not write a revision when no value changed. Check
  `update_steam` for this before adding more fields.
- Steam playtime is provider data. Do not change a status from it.
- Platform totals can be less than `playtime_forever`. Offline play is not
  assigned to a platform. The dashboard shows the difference as **Other**.

### Steam tags

- Source: `IStoreBrowseService/GetItems` with `include_tag_count`. The response
  gives `tagid` and `weight`, not names. `IStoreService/GetTagList` maps IDs to
  English names. Neither call needs a key. Checked live on September 28.
- Request up to 50 apps in one `GetItems` call. This stage is batched; the
  other enrichment stages stay per game.
- Keep the top 20 tags by weight, in weight order, as names in
  `SteamMetadata.tags`. Add a `tags_complete` stage flag.
- Existing libraries have no `tags_complete` flag, so the next sync fetches
  tags for all games once.
- Fetch the tag list once per sync, and only when some game needs tags. It is
  one small request, so the app keeps no tag cache. A tag ID without a name is
  skipped.
- A game that is missing from the `GetItems` result, or that has no successful
  result, keeps `tags_complete` false and retries on the next sync. A game
  with a successful result and no tags is complete with an empty list.
- Steam tags are separate from personal `tags`. Sync never writes personal tags.

### Deferred in this step

Deck compatibility, achievements, and a tag refresh action. `GetItems` also
returns `steam_deck_compat_category`. Add it when a feature uses it.

## 2. Smart collections

A **Smart collections** sidebar section below **Collections**. The app
computes the groups from current records. They store nothing and write nothing.

| Group | Values | Source |
| --- | --- | --- |
| Genres | Each genre, by game count | `SteamMetadata.genres` |
| Steam tags | Each tag, by game count | `SteamMetadata.tags` |
| My tags | Each personal tag, by game count | `PersonalData.tags` |
| Rating | 5 stars, 4 stars, 3 stars, 1–2 stars, Unrated | `PersonalData.rating` |
| Time played | Not played, Under 1 h, 1–10 h, 10–50 h, 50 h or more | `SteamData.playtime_minutes` |

- A group row expands to its values. Each value shows its game count.
  Selecting a value sets the library scope.
- Genres and tags show the 12 largest values and a **Show all** row.
- Rating uses whole stars rounded down from half-star units:
  10 is 5 stars, 8–9 is 4 stars, 6–7 is 3 stars, and 1–5 is 1–2 stars.
- Time played counts only Steam games. Manual games have no playtime, and the
  app does not guess it.
- Hidden games are excluded, as in other scopes. Values with no visible games
  are not shown.
- Cards, Grid, Table, and Board work with a smart scope. Dragging a game onto
  a smart value does nothing, because the membership is computed.
- The expanded or collapsed state of each group is device-local.

### Model

Add `Scope::Smart(SmartRule)`. `SmartRule` is a typed enum: `Genre(String)`,
`SteamTag(String)`, `MyTag(String)`, `Rating(RatingBand)`, and
`Playtime(PlaytimeBand)`. Put the matching and counting in a domain module
without GPUI, and test it there.

### Later: user smart collections

The built-in groups are the entry point for user-created smart collections.
A saved smart collection will be a named list of `SmartRule` values in library
definitions. Keep `SmartRule` serializable for that reason. Do not add saved
rules, a rule editor, or rule persistence in this branch.

## 3. Dashboard

A **Home** row at the top of the sidebar, above All games. Home is a page, not
a scope: the view switch, Filter menu, and search do not apply to it. The app
opens on Home.

### Panels in the first version

| Panel | Content | Action |
| --- | --- | --- |
| Continue playing | Up to 8 covers, newest `last_played` first | Open the game card |
| Playtime by platform | Hours for Windows, macOS, Linux, Steam Deck, and Other, as horizontal bars | None |
| Favorite games | Favorite games, by personal rating, then playtime | Open the game card |
| Favorite genres | Top 5 genres | Open the genre smart scope |
| Collections | Each collection with its count and up to 3 covers | Open the collection |

- Favorite genres count the games that are favorites or rated 4 stars or more.
  Playtime breaks ties. With no such games, the panel uses playtime only.
- Compute all panel data in a pure domain function over the loaded games. Test it
  with fixtures. The page does no network work.
- Empty states: no Steam account shows **Connect Steam**. Missing
  `last_played` before the next sync shows **Sync to see recent games**.
  An empty panel shows one short sentence and no placeholder art.
- Hidden games are excluded from all panels.

### Later panels

Rediscover (owned games not played for a long time), Random game, and
Recommendations. Recommendations use Choose from the [app plan](app-plan.md).
Do not add these in the first version.

## 4. Wishlist

### Source

- `IWishlistService/GetWishlist` with the SteamID64 returns the wishlist app
  IDs, priority, and date added. A call without a key returned an empty
  response for a test account. Check the fields and the visibility rules with
  the user's account before writing the parser.
- The wishlist response is the complete list. Treat an empty or invalid
  response as a failed stage, not as an empty wishlist.

### Records

- A wishlisted game is a library record with `steam.owned = false` and a
  `SteamData.wishlist` value (priority and date added).
- It uses the same identity as an owned import:
  `Uuid::new_v5(library_id, "steam:<app_id>")`. When the user buys the game,
  the same record becomes owned and loses its wishlist value. Personal notes and
  tags stay.
- Wishlist records use the normal enrichment stages: details, reviews, cover,
  and tags.
- A **Wishlist** sidebar row shows only wishlisted games. Wishlisted games
  are excluded from All games, Favorites, collections, smart collections,
  dashboard panels, and counts.

### Prices

- Source: `GetItems` with `include_all_purchase_options` and the user's
  country code. `best_purchase_option` gives `final_price_in_cents`,
  `original_price_in_cents`, `discount_pct`, and formatted prices.
  Checked live on September 28.
- Prices are device-local cache data, not library data. Store them outside the
  library folder with the fetch time. Prices change often; writing them into
  records would add a revision for every price change on every computer.
- Refresh prices when the cache is older than 12 hours, during sync or when
  the Wishlist scope opens. Batch 50 apps per request.
- Country: use `loccountrycode` from `ISteamUser/GetPlayerSummaries` when
  the profile exposes it. Let the user override it in Settings. Show the currency
  from the response.
- The Wishlist scope shows price, discount, and an **On sale** filter. It
  can sort by discount, price, priority, and date added.
- A game with no price shows **No price**. Examples are unreleased games and
  games not sold in the region.

### Deferred

Price history, lowest-price data, and sale notifications. Steam does not
provide history; it needs a third-party service such as IsThereAnyDeal.

## Open questions

- A game leaves the Steam wishlist and the user did not buy it. Proposed
  default: clear its wishlist value and keep the record if it has personal
  edits. Without personal edits, archive it with an explicit tombstone.
- Confirm that the Board should include the Wishlist scope, or hide the Board
  option there.

## Acceptance checks

- A sync fills `last_played`, platform playtime, and Steam tags. A second sync
  with no Steam changes writes no new revisions.
- Each smart value shows the same games in Cards, Grid, Table, and Board, and
  its count matches the visible results.
- Personal tags and Steam tags stay separate after sync and edits.
- Home opens on launch and works offline. Each panel item opens the right card
  or scope.
- Buying a wishlisted game keeps one record and its personal edits.
- A failed wishlist or price request keeps the previous wishlist and cached
  prices, and reports the failure.
- A 10,000-game fixture keeps the sidebar and Home responsive.
