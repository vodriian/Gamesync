//! Wishlist prices. Prices are device-local cache data, never library data:
//! they change often, and writing them into records would add a revision for
//! every price change on every computer.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

/// Refresh prices older than this.
pub const MAX_AGE_SECS: u64 = 12 * 60 * 60;
/// Used when no country is known or set.
pub const DEFAULT_COUNTRY: &str = "US";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    pub final_cents: u64,
    /// Present only during a discount.
    pub original_cents: Option<u64>,
    pub discount_pct: u8,
    /// Steam's text with the currency, for example "$59.99" or "59,99€".
    pub formatted_final: String,
    pub formatted_original: Option<String>,
    /// Unix seconds when the current discount ends. Older caches lack it.
    #[serde(default)]
    pub sale_ends: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Quote {
    Price(Price),
    Free,
    /// Not sold in this country, not released, or no purchase option.
    Unavailable,
}

impl Quote {
    pub fn label(&self) -> String {
        match self {
            Self::Price(price) if price.discount_pct > 0 => {
                format!("−{}% {}", price.discount_pct, price.formatted_final)
            }
            Self::Price(price) => price.formatted_final.clone(),
            Self::Free => "Free".into(),
            Self::Unavailable => "No price".into(),
        }
    }

    /// "Sale ends today", "… tomorrow", or "… in N days". Relative wording
    /// needs no date formatting and reads well on a small card.
    pub fn sale_label(&self, now: i64) -> Option<String> {
        let Self::Price(price) = self else {
            return None;
        };
        let ends = price.sale_ends.filter(|_| price.discount_pct > 0)?;
        let days = (ends - now).div_euclid(86_400);
        Some(match days {
            ..0 => "Sale ended".into(),
            0 => "Sale ends today".into(),
            1 => "Sale ends tomorrow".into(),
            n => format!("Sale ends in {n} days"),
        })
    }

    pub fn discount(&self) -> u8 {
        match self {
            Self::Price(price) => price.discount_pct,
            _ => 0,
        }
    }

    /// Sort key: free first, unavailable last.
    pub fn cents(&self) -> Option<u64> {
        match self {
            Self::Price(price) => Some(price.final_cents),
            Self::Free => Some(0),
            Self::Unavailable => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PriceCache {
    pub country: String,
    /// Unix seconds of the last complete fetch.
    pub fetched_at: u64,
    pub quotes: BTreeMap<u32, Quote>,
}

impl PriceCache {
    /// Stale when too old, for another country, or missing a wishlist game.
    pub fn stale(&self, now: u64, country: &str, ids: &[u32]) -> bool {
        self.country != country
            || now.saturating_sub(self.fetched_at) > MAX_AGE_SECS
            || ids.iter().any(|id| !self.quotes.contains_key(id))
    }
}

/// Steam cent values are JSON strings; accept numbers too.
fn cents(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

/// Quotes from a `GetItems` response with `include_all_purchase_options`.
/// Apps without a successful result are omitted and stay unknown.
pub fn parse_quotes(data: &Value) -> Result<BTreeMap<u32, Quote>> {
    let items = data["response"]["store_items"]
        .as_array()
        .context("Store prices response is incomplete")?;
    Ok(items
        .iter()
        .filter(|item| item["success"].as_u64() == Some(1))
        .filter_map(|item| {
            let id = u32::try_from(item["appid"].as_u64()?).ok()?;
            let option = &item["best_purchase_option"];
            let quote = match cents(&option["final_price_in_cents"]) {
                Some(final_cents) => Quote::Price(Price {
                    final_cents,
                    original_cents: cents(&option["original_price_in_cents"]),
                    discount_pct: option["discount_pct"]
                        .as_u64()
                        .and_then(|d| u8::try_from(d).ok())
                        .filter(|d| *d <= 100)
                        .unwrap_or(0),
                    formatted_final: option["formatted_final_price"].as_str().map_or_else(
                        || format!("{:.2}", final_cents as f64 / 100.),
                        str::to_owned,
                    ),
                    formatted_original: option["formatted_original_price"]
                        .as_str()
                        .map(str::to_owned),
                    // With several discounts, the first to end sets the date.
                    sale_ends: option["active_discounts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|d| d["discount_end_date"].as_i64())
                        .min(),
                }),
                None if item["is_free"].as_bool() == Some(true) => Quote::Free,
                None => Quote::Unavailable,
            };
            Some((id, quote))
        })
        .collect())
}

/// One cache file per library in the device cache folder.
pub fn cache_path(cache_dir: &Path, library_id: uuid::Uuid) -> PathBuf {
    cache_dir.join(format!("prices-{library_id}.json"))
}

/// A missing or unreadable cache is treated as empty; it is rebuilt from Steam.
pub fn load(path: &Path) -> PriceCache {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, cache: &PriceCache) -> Result<()> {
    let parent = path.parent().context("Price cache has no folder")?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec(cache)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Fetch every quote, 50 apps per request. Any failed request fails the whole
/// fetch, so the caller keeps the previous cache.
pub fn fetch(
    client: &crate::steam::SteamClient,
    ids: &[u32],
    country: &str,
    now: u64,
) -> Result<PriceCache> {
    let mut quotes = BTreeMap::new();
    for batch in ids.chunks(crate::steam::client::TAG_BATCH) {
        let mut found = client.quotes(batch, country)?;
        // An app Steam does not return has no price in this country.
        for id in batch {
            quotes.insert(*id, found.remove(id).unwrap_or(Quote::Unavailable));
        }
    }
    Ok(PriceCache {
        country: country.into(),
        fetched_at: now,
        quotes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quotes_read_string_cents_discounts_free_and_missing_apps() {
        let quotes = parse_quotes(&json!({"response":{"store_items":[
            {"appid":1,"success":1,"best_purchase_option":{"final_price_in_cents":"5999","formatted_final_price":"59,99€"}},
            {"appid":2,"success":1,"best_purchase_option":{"final_price_in_cents":"1499","original_price_in_cents":"2999","discount_pct":50,"formatted_final_price":"$14.99","formatted_original_price":"$29.99","active_discounts":[{"discount_end_date":1000000},{"discount_end_date":900000}]}},
            {"appid":3,"success":1,"is_free":true},
            {"appid":4,"success":1},
            {"appid":0,"success":15}
        ]}}))
        .unwrap();
        assert_eq!(quotes[&1].label(), "59,99€");
        assert_eq!(quotes[&2].label(), "−50% $14.99");
        assert_eq!(quotes[&2].discount(), 50);
        // The earliest end wins; the label counts whole days.
        assert_eq!(
            quotes[&2].sale_label(900_000 - 86_400 * 3 - 10).as_deref(),
            Some("Sale ends in 3 days")
        );
        assert_eq!(
            quotes[&2].sale_label(900_000 - 10).as_deref(),
            Some("Sale ends today")
        );
        assert_eq!(
            quotes[&2].sale_label(900_001).as_deref(),
            Some("Sale ended")
        );
        assert_eq!(quotes[&1].sale_label(0), None);
        assert_eq!(quotes[&3], Quote::Free);
        assert_eq!(quotes[&4], Quote::Unavailable);
        assert_eq!(quotes.len(), 4);
    }

    #[test]
    fn cache_is_stale_by_age_country_or_new_games() {
        let cache = PriceCache {
            country: "US".into(),
            fetched_at: 1_000,
            quotes: BTreeMap::from([(1, Quote::Free)]),
        };
        assert!(!cache.stale(1_000 + MAX_AGE_SECS, "US", &[1]));
        assert!(cache.stale(1_001 + MAX_AGE_SECS, "US", &[1]));
        assert!(cache.stale(1_000, "DE", &[1]));
        assert!(cache.stale(1_000, "US", &[1, 2]));
    }

    #[test]
    fn cache_round_trips_and_a_broken_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = cache_path(dir.path(), uuid::Uuid::nil());
        assert_eq!(load(&path), PriceCache::default());
        let cache = PriceCache {
            country: "DE".into(),
            fetched_at: 5,
            quotes: BTreeMap::from([(7, Quote::Unavailable)]),
        };
        save(&path, &cache).unwrap();
        assert_eq!(load(&path), cache);
        std::fs::write(&path, b"{broken").unwrap();
        assert_eq!(load(&path), PriceCache::default());
    }
}
