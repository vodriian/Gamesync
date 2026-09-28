//! Wishlist price refresh. Runs when the Wishlist scope opens; see `prices`.
use super::GameSyncApp;
use anyhow::Context as _;
use gamesync_desktop::{prices, steam::SteamClient};
use gpui::{prelude::*, Context};

impl GameSyncApp {
    /// Show cached prices at once; fetch new ones only when they are stale.
    /// A failed fetch keeps the saved prices for the same country.
    pub(super) fn refresh_prices(&mut self, cx: &mut Context<Self>) {
        if self.price_task.is_some() {
            return;
        }
        let library = self.library.read(cx);
        let Some((_, manifest)) = &library.source else {
            return;
        };
        let library_id = manifest.library_id;
        let ids = library.wishlist_app_ids();
        if ids.is_empty() {
            return;
        }
        self.price_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let country = crate::settings::load()?.price_country();
                    let folder = directories::ProjectDirs::from("app", "GameSync", "GameSync")
                        .context("Local cache directory is unavailable")?
                        .cache_dir()
                        .to_owned();
                    let path = prices::cache_path(&folder, library_id);
                    let mut cached = prices::load(&path);
                    if cached.country != country {
                        cached = prices::PriceCache::default();
                    }
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    if !cached.stale(now, &country, &ids) {
                        return Ok::<_, anyhow::Error>((cached, None));
                    }
                    let fetched = SteamClient::new()
                        .and_then(|client| prices::fetch(&client, &ids, &country, now))
                        .and_then(|fresh| prices::save(&path, &fresh).map(|()| fresh));
                    Ok(match fetched {
                        Ok(fresh) => (fresh, None),
                        Err(error) => (cached, Some(error.to_string())),
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.price_task = None;
                match result {
                    Ok((cache, warning)) => {
                        this.library.update(cx, |lib, cx| {
                            lib.set_prices(cache);
                            cx.notify();
                        });
                        if let Some(warning) = warning {
                            this.notice = format!(
                                "Could not refresh wishlist prices: {warning}. Saved prices stay."
                            );
                        }
                    }
                    Err(error) => {
                        this.notice = format!("Could not load wishlist prices: {error}");
                    }
                }
                cx.notify();
            });
        }));
    }
}
