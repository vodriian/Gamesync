//! Steam wire formats. Errors deliberately exclude request URLs and response bodies.
use anyhow::{bail, ensure, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, io::Read, time::Duration};

#[derive(Clone, Default, Deserialize)]
pub struct OwnedGame {
    pub appid: u32,
    pub name: String,
    #[serde(default)]
    pub playtime_forever: u32,
    /// Unix seconds; Steam sends 0 for a game that was never played.
    #[serde(default)]
    pub rtime_last_played: i64,
    #[serde(default)]
    pub playtime_windows_forever: u32,
    #[serde(default)]
    pub playtime_mac_forever: u32,
    #[serde(default)]
    pub playtime_linux_forever: u32,
    #[serde(default)]
    pub playtime_deck_forever: u32,
}

impl OwnedGame {
    pub fn last_played(&self) -> Option<i64> {
        (self.rtime_last_played > 0).then_some(self.rtime_last_played)
    }

    pub fn platform_minutes(&self) -> crate::records::PlatformMinutes {
        crate::records::PlatformMinutes {
            windows: self.playtime_windows_forever,
            mac: self.playtime_mac_forever,
            linux: self.playtime_linux_forever,
            deck: self.playtime_deck_forever,
        }
    }
}
/// One Steam wishlist entry. `priority` is the user's order; lower is first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WishlistItem {
    pub appid: u32,
    pub priority: u32,
    /// Unix seconds.
    pub date_added: i64,
}

pub struct SteamClient {
    client: Client,
}
impl SteamClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(20))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
    fn bytes(&self, url: &str, query: &[(&str, &str)], limit: u64) -> Result<Vec<u8>> {
        let response = self.client.get(url).query(query).send().map_err(|_| {
            anyhow::anyhow!("Steam request failed. Check your connection and retry.")
        })?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "Steam returned HTTP {}. Check the key, visibility, or retry later.",
                status.as_u16()
            );
        }
        let mut bytes = Vec::new();
        response
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow::anyhow!("Steam response was interrupted"))?;
        ensure!(bytes.len() as u64 <= limit, "Steam response is too large");
        Ok(bytes)
    }
    fn json(&self, url: &str, query: &[(&str, &str)]) -> Result<Value> {
        serde_json::from_slice(&self.bytes(url, query, 8 * 1024 * 1024)?)
            .map_err(|_| anyhow::anyhow!("Steam returned an invalid response"))
    }
    pub fn account(&self, key: &str, input: &str) -> Result<String> {
        let input = input.trim().trim_end_matches('/');
        if input.len() == 17 && input.bytes().all(|b| b.is_ascii_digit()) {
            return Ok(input.into());
        }
        if let Some(id) = input
            .strip_prefix("https://steamcommunity.com/profiles/")
            .or_else(|| input.strip_prefix("http://steamcommunity.com/profiles/"))
        {
            ensure!(
                id.len() == 17 && id.bytes().all(|b| b.is_ascii_digit()),
                "Enter a Steam profile link or SteamID64"
            );
            return Ok(id.into());
        }
        let vanity = input
            .strip_prefix("https://steamcommunity.com/id/")
            .or_else(|| input.strip_prefix("http://steamcommunity.com/id/"))
            .unwrap_or(input);
        ensure!(
            !vanity.is_empty()
                && vanity.len() <= 100
                && vanity
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "Enter a Steam profile link or SteamID64"
        );
        let data = self.json(
            "https://api.steampowered.com/ISteamUser/ResolveVanityURL/v1/",
            &[("key", key), ("vanityurl", vanity)],
        )?;
        let id = data["response"]["steamid"]
            .as_str()
            .context("Steam profile was not found")?;
        ensure!(
            id.len() == 17 && id.bytes().all(|b| b.is_ascii_digit()),
            "Steam returned an invalid account"
        );
        Ok(id.into())
    }
    pub fn owned(&self, key: &str, account: &str) -> Result<Vec<OwnedGame>> {
        let data = self.json(
            "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/",
            &[
                ("key", key),
                ("steamid", account),
                ("include_appinfo", "true"),
                ("include_played_free_games", "true"),
            ],
        )?;
        parse_owned(data)
    }
    pub fn details(&self, id: u32) -> Result<Value> {
        let data = self.json(
            "https://store.steampowered.com/api/appdetails",
            &[("appids", &id.to_string()), ("l", "english")],
        )?;
        let result = &data[id.to_string()];
        ensure!(
            result["success"] == true && result["data"].is_object(),
            "Store details are unavailable"
        );
        Ok(result["data"].clone())
    }
    pub fn reviews(&self, id: u32) -> Result<Value> {
        let data = self.json(
            &format!("https://store.steampowered.com/appreviews/{id}"),
            &[
                ("json", "1"),
                ("language", "all"),
                ("purchase_type", "all"),
                ("num_per_page", "0"),
            ],
        )?;
        ensure!(
            data["success"] == 1 && data["query_summary"].is_object(),
            "Reviews are unavailable"
        );
        Ok(data["query_summary"].clone())
    }
    /// Valve's Steam Deck compatibility report. This store endpoint is not a
    /// documented API; `parse_setup` rejects any shape it does not expect.
    pub fn deck_report(&self, id: u32) -> Result<Value> {
        self.json(
            "https://store.steampowered.com/saleaction/ajaxgetdeckappcompatibilityreport",
            &[("nAppID", &id.to_string()), ("l", "english")],
        )
    }
    /// Store categories only, for controller support.
    pub fn store_categories(&self, id: u32) -> Result<Value> {
        let data = self.json(
            "https://store.steampowered.com/api/appdetails",
            &[
                ("appids", &id.to_string()),
                ("l", "english"),
                ("filters", "categories"),
            ],
        )?;
        let result = &data[id.to_string()];
        ensure!(
            result["success"] == true,
            "Store categories are unavailable"
        );
        // A game with no categories returns an empty list instead of an object.
        Ok(result["data"].clone())
    }
    /// Tag IDs per app, highest weight first. The call takes at most
    /// `TAG_BATCH` apps. An app that is missing from the result must be retried.
    pub fn store_tags(&self, ids: &[u32]) -> Result<BTreeMap<u32, Vec<u32>>> {
        ensure!(ids.len() <= TAG_BATCH, "Too many apps in one tag request");
        let request = serde_json::json!({
            "ids": ids.iter().map(|id| serde_json::json!({"appid": id})).collect::<Vec<_>>(),
            "context": {"language": "english", "country_code": "US"},
            "data_request": {"include_tag_count": TAG_LIMIT}
        });
        parse_store_tags(&self.json(
            "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/",
            &[("input_json", &request.to_string())],
        )?)
    }
    /// A public wishlist needs no key. Steam answers a private wishlist and an
    /// empty one the same way, so both are errors: removals need a real list.
    pub fn wishlist(&self, account: &str) -> Result<Vec<WishlistItem>> {
        parse_wishlist(&self.json(
            "https://api.steampowered.com/IWishlistService/GetWishlist/v1/",
            &[("steamid", account)],
        )?)
    }
    /// Store names for up to `TAG_BATCH` apps. Missing apps are omitted.
    pub fn store_names(&self, ids: &[u32]) -> Result<BTreeMap<u32, String>> {
        ensure!(ids.len() <= TAG_BATCH, "Too many apps in one name request");
        let request = serde_json::json!({
            "ids": ids.iter().map(|id| serde_json::json!({"appid": id})).collect::<Vec<_>>(),
            "context": {"language": "english", "country_code": "US"}
        });
        parse_store_names(&self.json(
            "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/",
            &[("input_json", &request.to_string())],
        )?)
    }
    /// Prices for up to `TAG_BATCH` apps in one store country.
    pub fn quotes(
        &self,
        ids: &[u32],
        country: &str,
    ) -> Result<BTreeMap<u32, crate::prices::Quote>> {
        ensure!(ids.len() <= TAG_BATCH, "Too many apps in one price request");
        ensure!(valid_country(country), "Store country must be two letters");
        let request = serde_json::json!({
            "ids": ids.iter().map(|id| serde_json::json!({"appid": id})).collect::<Vec<_>>(),
            "context": {"language": "english", "country_code": country},
            "data_request": {"include_all_purchase_options": true}
        });
        crate::prices::parse_quotes(&self.json(
            "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/",
            &[("input_json", &request.to_string())],
        )?)
    }
    /// The profile country, when the profile shows it. Used for store prices.
    pub fn country(&self, key: &str, account: &str) -> Result<Option<String>> {
        let data = self.json(
            "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/",
            &[("key", key), ("steamids", account)],
        )?;
        Ok(data["response"]["players"][0]["loccountrycode"]
            .as_str()
            .filter(|code| valid_country(code))
            .map(str::to_owned))
    }
    /// English names for store tag IDs. `GetItems` returns only IDs.
    pub fn tag_names(&self) -> Result<BTreeMap<u32, String>> {
        parse_tag_names(&self.json(
            "https://api.steampowered.com/IStoreService/GetTagList/v1/",
            &[("language", "english")],
        )?)
    }
    /// Store artwork paths for one app, or None when Steam does not list them.
    fn store_assets(&self, id: u32) -> Option<Value> {
        let request = serde_json::json!({
            "ids": [{"appid": id}],
            "context": {"language": "english", "country_code": "US"},
            "data_request": {"include_assets": true}
        });
        let artwork = self
            .json(
                "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/",
                &[("input_json", &request.to_string())],
            )
            .ok()?;
        artwork["response"]["store_items"]
            .as_array()?
            .iter()
            .find(|item| item["appid"].as_u64() == Some(u64::from(id)))
            .map(|item| item["assets"].clone())
    }
    pub fn cover(&self, id: u32) -> Result<Vec<u8>> {
        let assets = self.store_assets(id);
        let assets = assets.as_ref();
        let mut urls = Vec::new();
        for field in ["library_capsule", "library_capsule_2x"] {
            if let Some(url) = assets
                .and_then(|a| a[field].as_str())
                .and_then(|path| artwork_url(id, path))
            {
                urls.push(url);
            }
        }
        urls.push(format!(
            "https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_600x900.jpg"
        ));
        if let Some(url) = assets
            .and_then(|a| a["header"].as_str())
            .and_then(|path| artwork_url(id, path))
        {
            urls.push(url);
        }
        for url in urls {
            if let Ok(bytes) = self.bytes(&url, &[], 8 * 1024 * 1024) {
                if validate_cover(&bytes).is_ok() {
                    return Ok(bytes);
                }
            }
        }
        // Older store responses may expose a header even when artwork metadata is absent.
        if let Ok(details) = self.details(id) {
            if let Some(url) = details["header_image"]
                .as_str()
                .filter(|url| trusted_header(id, url))
            {
                if let Ok(bytes) = self.bytes(url, &[], 8 * 1024 * 1024) {
                    if validate_cover(&bytes).is_ok() {
                        return Ok(bytes);
                    }
                }
            }
        }
        bail!("Steam has no usable cover right now. Your previous cover is kept. Try again later.")
    }
    /// The landscape store header (920×430, then 460×215) for the inside
    /// cover. It carries the game logo, so the app shows it uncropped.
    pub fn banner(&self, id: u32) -> Result<Vec<u8>> {
        let assets = self.store_assets(id);
        let mut urls: Vec<String> = ["header_2x", "header"]
            .into_iter()
            .filter_map(|field| artwork_url(id, assets.as_ref()?[field].as_str()?))
            .collect();
        // Older store responses may expose a header even when artwork metadata is absent.
        if urls.is_empty() {
            if let Ok(details) = self.details(id) {
                urls.extend(
                    details["header_image"]
                        .as_str()
                        .filter(|url| trusted_header(id, url))
                        .map(str::to_owned),
                );
            }
        }
        for url in urls {
            if let Ok(bytes) = self.bytes(&url, &[], 8 * 1024 * 1024) {
                if validate_banner(&bytes).is_ok() {
                    return Ok(bytes);
                }
            }
        }
        bail!("Steam has no landscape artwork right now. Try again later.")
    }
}
pub fn parse_owned(data: Value) -> Result<Vec<OwnedGame>> {
    let response = &data["response"];
    let count = response["game_count"].as_u64().context(
        "Steam did not expose this library. Check Game details visibility and your key.",
    )?;
    if count == 0 && response.get("games").is_none() {
        return Ok(Vec::new());
    }
    let games: Vec<OwnedGame> = serde_json::from_value(response["games"].clone())
        .context("Owned games response is incomplete")?;
    ensure!(
        games.len() as u64 == count,
        "Owned games response is incomplete"
    );
    let mut ids = std::collections::BTreeSet::new();
    ensure!(
        games
            .iter()
            .all(|g| g.appid > 0 && !g.name.trim().is_empty() && ids.insert(g.appid)),
        "Owned games response has invalid or duplicate games"
    );
    Ok(games)
}

/// Apps per `GetItems` tag request. Keeps the query string well below URL limits.
pub const TAG_BATCH: usize = 50;
/// Tags kept per game.
pub const TAG_LIMIT: usize = 20;

pub fn parse_store_tags(data: &Value) -> Result<BTreeMap<u32, Vec<u32>>> {
    let items = data["response"]["store_items"]
        .as_array()
        .context("Store tags response is incomplete")?;
    let mut tags = BTreeMap::new();
    for item in items {
        let Some(id) = item["appid"].as_u64().and_then(|id| u32::try_from(id).ok()) else {
            continue;
        };
        // Unknown or removed apps come back without a successful result. Omit
        // them so the stage stays incomplete and retries on the next sync.
        if item["success"].as_u64() != Some(1) {
            continue;
        }
        let mut weighted = item["tags"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|tag| {
                let id = u32::try_from(tag["tagid"].as_u64()?).ok()?;
                Some((tag["weight"].as_u64().unwrap_or(0), id))
            })
            .collect::<Vec<_>>();
        weighted.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        weighted.truncate(TAG_LIMIT);
        tags.insert(id, weighted.into_iter().map(|(_, id)| id).collect());
    }
    Ok(tags)
}

/// ISO 3166-1 alpha-2 in upper case, as Steam expects.
/// Steam Deck report plus store categories. `now` is Unix seconds.
pub fn parse_setup(
    report: &Value,
    categories: &Value,
    now: i64,
) -> Result<crate::suitability::SetupEvidence> {
    use crate::suitability::{
        ControllerSupport, DeckNote, DeckNoteKind, DeckRating, SetupEvidence,
    };
    ensure!(report["success"] == 1, "Steam Deck report is unavailable");
    let results = &report["results"];
    let deck = match results["resolved_category"].as_u64() {
        Some(3) => DeckRating::Verified,
        Some(2) => DeckRating::Playable,
        Some(1) => DeckRating::Unsupported,
        _ => DeckRating::Unknown,
    };
    let deck_notes = results["resolved_items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let kind = match item["display_type"].as_u64()? {
                4 => DeckNoteKind::Pass,
                3 => DeckNoteKind::Warning,
                2 => DeckNoteKind::Blocker,
                1 => DeckNoteKind::Info,
                _ => return None,
            };
            let token = item["loc_token"].as_str()?;
            let token = token.rsplit("_TestResult_").next()?;
            let valid = !token.is_empty()
                && token.len() <= 120
                && token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_');
            valid.then(|| DeckNote {
                kind,
                text: sentence(token),
            })
        })
        .take(12)
        .collect();
    let ids: Vec<u64> = categories["categories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|category| category["id"].as_u64())
        .collect();
    let controller = if ids.contains(&28) {
        ControllerSupport::Full
    } else if ids.contains(&18) {
        ControllerSupport::Partial
    } else {
        ControllerSupport::None
    };
    Ok(SetupEvidence {
        deck,
        deck_notes,
        controller,
        workshop: Some(ids.contains(&30)),
        checked_at: now,
    })
}

/// "InterfaceTextIsNotLegible" becomes "Interface text is not legible".
/// Valve's test token as a sentence. Common blockers get fixed wording;
/// other tokens are split into words, keeping acronyms such as VR whole.
fn sentence(token: &str) -> String {
    match token {
        "SteamOSDoesNotSupport" => "SteamOS does not support this game".into(),
        "UnsupportedAntiCheatConfiguration" | "UnsupportedAntiCheat_Other" => {
            "Its anti-cheat does not support Steam Deck".into()
        }
        _ => match token.strip_prefix("SteamOSDoesNotSupport_") {
            Some(rest) => format!("SteamOS does not support {}", words(rest)),
            None => {
                let text = words(token);
                let mut chars = text.chars();
                chars.next().map_or(text.clone(), |first| {
                    first.to_uppercase().chain(chars).collect()
                })
            }
        },
    }
}

/// "InterfaceTextIsNotLegible" becomes "interface text is not legible";
/// a run of capitals stays one word ("VR", "SteamOS" reads "steam OS").
fn words(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        let previous = index.checked_sub(1).map(|i| chars[i]);
        let next = chars.get(index + 1);
        let starts_word = c.is_ascii_uppercase()
            && previous.is_some_and(|p| {
                p.is_ascii_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()))
            });
        if starts_word && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
        .into_iter()
        .map(|word| {
            if word.len() > 1 && word.chars().all(|c| c.is_ascii_uppercase()) {
                word
            } else {
                word.to_ascii_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn valid_country(code: &str) -> bool {
    code.len() == 2 && code.bytes().all(|b| b.is_ascii_uppercase())
}

pub fn parse_wishlist(data: &Value) -> Result<Vec<WishlistItem>> {
    let items = data["response"]["items"]
        .as_array()
        .filter(|items| !items.is_empty())
        .context("Steam did not return a wishlist. Make it public in Steam privacy settings, or add a game to it.")?;
    let mut seen = std::collections::BTreeSet::new();
    items
        .iter()
        .map(|item| {
            let appid = item["appid"]
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .filter(|id| *id > 0)
                .context("Wishlist has an invalid game")?;
            ensure!(seen.insert(appid), "Wishlist has a duplicate game");
            Ok(WishlistItem {
                appid,
                priority: item["priority"]
                    .as_u64()
                    .and_then(|p| u32::try_from(p).ok())
                    .unwrap_or(u32::MAX),
                date_added: item["date_added"].as_i64().unwrap_or(0),
            })
        })
        .collect()
}

pub fn parse_store_names(data: &Value) -> Result<BTreeMap<u32, String>> {
    let items = data["response"]["store_items"]
        .as_array()
        .context("Store names response is incomplete")?;
    Ok(items
        .iter()
        .filter(|item| item["success"].as_u64() == Some(1))
        .filter_map(|item| {
            let id = u32::try_from(item["appid"].as_u64()?).ok()?;
            let name = item["name"].as_str()?.trim();
            (!name.is_empty()).then(|| (id, name.to_owned()))
        })
        .collect())
}

pub fn parse_tag_names(data: &Value) -> Result<BTreeMap<u32, String>> {
    let names = data["response"]["tags"]
        .as_array()
        .context("Steam tag list is incomplete")?
        .iter()
        .filter_map(|tag| {
            let id = u32::try_from(tag["tagid"].as_u64()?).ok()?;
            let name = tag["name"].as_str()?.trim();
            (!name.is_empty()).then(|| (id, name.to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    ensure!(!names.is_empty(), "Steam tag list is empty");
    Ok(names)
}

/// Decode before recording success; a partial download must remain retryable.
pub fn validate_cover(bytes: &[u8]) -> Result<()> {
    artwork_size(bytes).map(|_| ())
}

/// A banner must be clearly wider than tall; Steam headers are about 2.14:1.
pub fn validate_banner(bytes: &[u8]) -> Result<()> {
    let (width, height) = artwork_size(bytes)?;
    ensure!(
        width * 2 >= height * 3,
        "Landscape artwork is not wide enough"
    );
    Ok(())
}

/// Decoded size of a usable JPEG, after the size and placeholder checks.
fn artwork_size(bytes: &[u8]) -> Result<(u32, u32)> {
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Jpeg);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .context("Cover is invalid or too large")?
        .to_rgb8();
    let (width, height) = image.dimensions();
    ensure!(width >= 32 && height >= 32, "Cover is too small");
    // Steam can return HTTP 200 with a flat gray JPEG and a one-pixel border.
    // Ignore the border, then reject only near-uniform interiors.
    let mut low = [255u8; 3];
    let mut high = [0u8; 3];
    for y in height / 20..height - height / 20 {
        for x in width / 20..width - width / 20 {
            let pixel = image.get_pixel(x, y);
            for channel in 0..3 {
                low[channel] = low[channel].min(pixel[channel]);
                high[channel] = high[channel].max(pixel[channel]);
            }
        }
    }
    ensure!(
        (0..3).any(|i| high[i].saturating_sub(low[i]) > 4),
        "Steam returned a blank placeholder"
    );
    Ok((width, height))
}

fn artwork_url(id: u32, path: &str) -> Option<String> {
    (!path.is_empty()
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b)))
    .then(|| {
        format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{id}/{path}")
    })
}
fn trusted_header(id: u32, value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host == "steamstatic.com" || host.ends_with(".steamstatic.com"))
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && (url
                .path()
                .starts_with(&format!("/store_item_assets/steam/apps/{id}/"))
                || url.path().starts_with(&format!("/steam/apps/{id}/")))
    })
}

#[cfg(test)]
mod cover_tests {
    use super::*;

    #[test]
    fn setup_evidence_parses_deck_report_and_controller_categories() {
        use crate::suitability::{ControllerSupport, DeckNoteKind, DeckRating};
        let report = serde_json::json!({"success": 1, "results": {"resolved_category": 1,
            "resolved_items": [
                {"display_type": 2, "loc_token": "#SteamDeckVerified_TestResult_UnsupportedAntiCheat_Other"},
                {"display_type": 3, "loc_token": "#SteamDeckVerified_TestResult_InterfaceTextIsNotLegible"},
                {"display_type": 9, "loc_token": "#SteamDeckVerified_TestResult_Ignored"},
                {"display_type": 1, "loc_token": "<script>"}]}});
        let categories = serde_json::json!({"categories": [{"id": 2}, {"id": 18}]});
        let evidence = super::parse_setup(&report, &categories, 7).unwrap();
        assert_eq!(evidence.deck, DeckRating::Unsupported);
        assert_eq!(evidence.controller, ControllerSupport::Partial);
        assert_eq!(evidence.checked_at, 7);
        let notes: Vec<_> = evidence
            .deck_notes
            .iter()
            .map(|note| (note.kind, note.text.as_str()))
            .collect();
        assert_eq!(
            notes,
            [
                (
                    DeckNoteKind::Blocker,
                    "Its anti-cheat does not support Steam Deck"
                ),
                (DeckNoteKind::Warning, "Interface text is not legible"),
            ]
        );
        // Acronyms stay whole, and SteamOS blockers name what is missing.
        assert_eq!(
            super::sentence("SteamOSDoesNotSupport_VR"),
            "SteamOS does not support VR"
        );
        assert_eq!(
            super::sentence("SteamOSDoesNotSupport"),
            "SteamOS does not support this game"
        );
        assert_eq!(
            super::sentence("DefaultControllerConfigFullyFunctional"),
            "Default controller config fully functional"
        );
        // An unrated game is Unknown; a failed report is an error to retry.
        let unrated = serde_json::json!({"success": 1, "results": {"resolved_category": null}});
        let none = serde_json::json!([]);
        assert_eq!(
            super::parse_setup(&unrated, &none, 0).unwrap().deck,
            DeckRating::Unknown
        );
        assert!(super::parse_setup(&serde_json::json!({"success": 2}), &none, 0).is_err());
    }

    #[test]
    fn gray_placeholder_is_rejected_but_artwork_is_accepted() {
        let image = image::RgbImage::from_pixel(300, 450, image::Rgb([75, 75, 75]));
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode_image(&image)
            .unwrap();
        assert!(validate_cover(&bytes)
            .unwrap_err()
            .to_string()
            .contains("placeholder"));
        validate_cover(include_bytes!("../../fixtures/covers/620.jpg")).unwrap();
    }

    #[test]
    fn artwork_sources_stay_on_steam_and_match_the_game() {
        assert!(artwork_url(42, "abc/library_capsule.jpg").is_some());
        for path in [
            "../cover.jpg",
            "https://example.com/cover.jpg",
            "/cover.jpg",
        ] {
            assert!(artwork_url(42, path).is_none());
        }
        assert!(trusted_header(
            42,
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/42/abc/header.jpg"
        ));
        assert!(!trusted_header(
            42,
            "https://example.com/steam/apps/42/header.jpg"
        ));
        assert!(!trusted_header(
            42,
            "https://cdn.akamai.steamstatic.com/steam/apps/43/header.jpg"
        ));
    }
}
