//! Steam wire formats. Errors deliberately exclude request URLs and response bodies.
use anyhow::{bail, ensure, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;
use std::{io::Read, time::Duration};

#[derive(Clone, Deserialize)]
pub struct OwnedGame {
    pub appid: u32,
    pub name: String,
    #[serde(default)]
    pub playtime_forever: u32,
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
    pub fn cover(&self, id: u32) -> Result<Vec<u8>> {
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
            .ok();
        let assets = artwork
            .as_ref()
            .and_then(|v| v["response"]["store_items"].as_array())
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item["appid"].as_u64() == Some(u64::from(id)))
            })
            .map(|item| &item["assets"]);
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

/// Decode before recording success; a partial download must remain retryable.
pub fn validate_cover(bytes: &[u8]) -> Result<()> {
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
    Ok(())
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
