//! Glaze's grouped settings layout, backed by native device services.
mod ai;
mod appearance;
mod best_on;
mod sync;
mod view;
use crate::model::Library;
use gamesync_desktop::{
    credentials::{replace_checked, CredentialStore, OsCredential},
    library::LibraryStore,
    settings,
    steam::{self, SteamClient},
};
use gpui::{prelude::*, Entity, EventEmitter, Window};
use gpui_component::input::{InputEvent, InputState};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;

pub enum SettingsEvent {
    Theme(gamesync_desktop::appearance::Appearance),
    OmarchyMode(bool, Option<gamesync_desktop::omarchy::OmarchyTheme>),
    Refresh,
    LastSync(Option<u64>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Sync,
    Appearance,
    BestOn,
    Ai,
}
impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Sync => "Sync",
            Self::Appearance => "Look and feel",
            Self::BestOn => "Best on",
            Self::Ai => "AI",
        }
    }
}
// Secrets stay in memory until the explicit Save key action succeeds.
struct TestedKey {
    key: String,
    input: String,
    profile: String,
    account: String,
}
impl TestedKey {
    fn matches(&self, input: &str, profile: &str) -> bool {
        self.input == input.trim() && self.profile == profile.trim()
    }
}
pub struct SettingsView {
    library: Entity<Library>,
    profile: Entity<InputState>,
    key: Entity<InputState>,
    /// Two-letter store country for wishlist prices; empty uses the profile.
    country: Entity<InputState>,
    library_id: Option<Uuid>,
    ai: Entity<ai::AiSettings>,
    best_on: Entity<best_on::BestOnSettings>,
    key_saved: bool,
    theme: gamesync_desktop::appearance::Appearance,
    omarchy_mode: bool,
    omarchy_available: bool,
    omarchy_theme_name: Option<String>,
    section: Section,
    tested: Option<TestedKey>,
    /// The Steam dialog's Save tests first, then saves only if the test passes.
    save_after_test: bool,
    clear_key: bool,
    reduce_motion: bool,
    saving_appearance_preference: bool,
    saving_hidden_preference: bool,
    busy: bool,
    cancel: Option<steam::Cancellation>,
    message: String,
    last_sync: Option<u64>,
    sync: Entity<crate::sync_runtime::SyncState>,
    device_name: Entity<InputState>,
    /// Stop was pressed once while changes wait to be written.
    stop_confirm: bool,
    sync_busy: bool,
    passphrase: Entity<InputState>,
    passphrase_confirm: Entity<InputState>,
    /// Replace the passphrase inputs at the next render, so no copy stays.
    clear_passphrase: bool,
}
impl EventEmitter<SettingsEvent> for SettingsView {}
impl SettingsView {
    pub fn new(
        library: Entity<Library>,
        sync: Entity<crate::sync_runtime::SyncState>,
        theme: gamesync_desktop::appearance::Appearance,
        omarchy_mode: bool,
        omarchy_theme_name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        cx.observe(&sync, |_, _, cx| cx.notify()).detach();
        let best_on_library = library.clone();
        let best_on_state = cx.global::<super::best_on_state::BestOnGlobal>().0.clone();
        let open_rules = best_on_state.update(cx, |state, _| std::mem::take(&mut state.show_rules));
        cx.observe(&best_on_state, |this: &mut Self, state, cx| {
            if state.read(cx).show_rules {
                state.update(cx, |state, _| state.show_rules = false);
                this.section = Section::BestOn;
                cx.notify();
            }
        })
        .detach();
        Self {
            device_name: Self::device_name_input(window, cx),
            sync,
            stop_confirm: false,
            sync_busy: false,
            passphrase: Self::masked_input("Sync passphrase", window, cx),
            passphrase_confirm: Self::masked_input("Repeat the passphrase", window, cx),
            clear_passphrase: false,
            library,
            profile: Self::masked_input("Steam profile link or SteamID64", window, cx),
            key: Self::masked_input("Enter a key to save or replace", window, cx),
            country: {
                let loaded = settings::load().ok();
                let saved = loaded
                    .as_ref()
                    .and_then(|s| s.store_country.clone())
                    .unwrap_or_default();
                let placeholder = loaded.and_then(|s| s.detected_country).map_or_else(
                    || format!("Default: {}", gamesync_desktop::prices::DEFAULT_COUNTRY),
                    |country| format!("Detected: {country}"),
                );
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(placeholder)
                        .default_value(saved)
                })
            },
            library_id: None,
            ai: cx.new(|cx| ai::AiSettings::new(window, cx)),
            best_on: cx.new(|cx| best_on::BestOnSettings::new(best_on_library, window, cx)),
            key_saved: false,
            theme,
            omarchy_mode,
            omarchy_available: gamesync_desktop::omarchy::is_available(),
            omarchy_theme_name,
            // "Edit rules in Settings" can open this window for the first time.
            section: if open_rules {
                Section::BestOn
            } else {
                Section::General
            },
            tested: None,
            save_after_test: false,
            clear_key: false,
            reduce_motion: super::motion::reduced(cx),
            saving_appearance_preference: false,
            saving_hidden_preference: false,
            busy: false,
            cancel: None,
            message: String::new(),
            last_sync: None,
        }
    }
    fn masked_input(
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(placeholder)
        });
        cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        input
    }
    /// Empty clears the choice, so the profile country applies again.
    fn save_country(&mut self, cx: &mut Context<Self>) {
        let code = self.country.read(cx).value().trim().to_uppercase();
        if !code.is_empty() && !steam::client::valid_country(&code) {
            self.message = "Enter a two-letter country code, for example UA, US, or DE.".into();
            cx.notify();
            return;
        }
        let choice = (!code.is_empty()).then_some(code);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { settings::update(|s| s.store_country = choice) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.message = match result {
                    Ok(()) => {
                        "Store country saved. Wishlist prices refresh when you open Wishlist."
                            .into()
                    }
                    Err(error) => format!("Could not save the store country: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub fn show_general(&mut self, cx: &mut Context<Self>) {
        self.section = Section::General;
        cx.notify();
    }
    pub fn busy(&self) -> bool {
        self.saving_appearance_preference
            || self.saving_hidden_preference
            || self.busy
            || self.sync_busy
    }
    /// Read the saved Steam key again, for example after sync saved one.
    pub fn reload_connection(&mut self, cx: &mut Context<Self>) {
        if let (Some(id), false) = (self.library_id, self.busy) {
            self.read_connection(id, cx);
        }
    }
    pub fn show_sync(&mut self, cx: &mut Context<Self>) {
        self.section = Section::Sync;
        cx.notify();
    }
    pub fn set_theme(
        &mut self,
        theme: gamesync_desktop::appearance::Appearance,
        cx: &mut Context<Self>,
    ) {
        self.theme = theme;
        cx.notify();
    }
    pub fn set_omarchy_theme(
        &mut self,
        theme: &gamesync_desktop::omarchy::OmarchyTheme,
        cx: &mut Context<Self>,
    ) {
        self.omarchy_available = true;
        self.omarchy_theme_name = Some(theme.name.clone());
        cx.notify();
    }
    fn read_connection(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let last = settings::load()?.last_sync.get(&id.to_string()).copied();
                    let credential = OsCredential::steam(id)?.get();
                    Ok::<_, anyhow::Error>((last, credential))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok((last, credential)) => {
                        this.last_sync = last;
                        cx.emit(SettingsEvent::LastSync(last));
                        match credential {
                            Ok(key) => this.key_saved = key.is_some(),
                            Err(error) => this.message = error.to_string(),
                        }
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn tested_matches(&self, cx: &gpui::App) -> bool {
        self.tested.as_ref().is_some_and(|test| {
            test.matches(&self.key.read(cx).value(), &self.profile.read(cx).value())
        })
    }
    /// Test, then save if the test passes. A test that cannot start clears the
    /// request, so a later manual test never saves by itself.
    fn test_and_save(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.save_after_test = true;
        self.test_key(cx);
        if !self.busy {
            self.save_after_test = false;
        }
    }
    fn test_key(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some((_, manifest)) = self.library.read(cx).source.clone() else {
            return;
        };
        let input = self.key.read(cx).value().trim().to_owned();
        let profile = self.profile.read(cx).value().trim().to_owned();
        self.tested = None;
        self.busy = true;
        self.message = "Testing key…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let key = if input.is_empty() {
                        OsCredential::steam(manifest.library_id)?
                            .get()?
                            .ok_or_else(|| anyhow::anyhow!("Enter a Steam API key."))?
                    } else {
                        input.clone()
                    };
                    let client = SteamClient::new()?;
                    let account = client.account(&key, &profile)?;
                    anyhow::ensure!(
                        manifest
                            .definitions
                            .steam_account
                            .as_ref()
                            .is_none_or(|old| old == &account),
                        "This library belongs to another Steam account."
                    );
                    client.owned(&key, &account)?;
                    Ok::<_, anyhow::Error>(TestedKey {
                        key,
                        input,
                        profile,
                        account,
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                let save = std::mem::take(&mut this.save_after_test);
                match result {
                    Ok(test) => {
                        this.tested = Some(test);
                        if save {
                            this.save_key(cx);
                        } else {
                            this.message = "Test passed. Save key to keep this connection.".into();
                        }
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save_key(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.tested_matches(cx) {
            return;
        }
        let Some((root, manifest)) = self.library.read(cx).source.clone() else {
            return;
        };
        let Some(test) = self.tested.as_ref() else {
            return;
        };
        let key = test.key.clone();
        let account = test.account.clone();
        self.busy = true;
        self.message = "Saving connection…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let bind = |_: &str| {
                        LibraryStore::open(root)?.bind_steam(manifest.revision_id, &account)?;
                        Ok(())
                    };
                    replace_checked(&OsCredential::steam(manifest.library_id)?, &key, bind)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.tested = None;
                        this.clear_key = true;
                        this.key_saved = true;
                        this.message = "Key saved. This device will reuse it next time.".into();
                        cx.emit(SettingsEvent::Refresh);
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn remove_key(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.key_saved {
            return;
        }
        let Some(id) = self.library_id else {
            return;
        };
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { OsCredential::steam(id)?.remove() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.tested = None;
                        this.clear_key = true;
                        this.key_saved = false;
                        this.message =
                            "Key removed. Your games and personal details are kept.".into();
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn sync(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some((root, manifest)) = self.library.read(cx).source.clone() else {
            return;
        };
        let Some(account) = manifest.definitions.steam_account else {
            self.message = "Test and save your key first.".into();
            cx.notify();
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        self.busy = true;
        self.message = "Starting sync…".into();
        // One shared progress value bounds memory even when the window is closed.
        let progress = Arc::new(std::sync::Mutex::new(String::new()));
        let display_progress = progress.clone();
        cx.spawn(async move |this, cx| {
            let task = cx.background_spawn(async move {
                let key = OsCredential::steam(manifest.library_id)?
                    .get()?
                    .ok_or_else(|| anyhow::anyhow!("Enter your Steam key in Settings"))?;
                let result = steam::sync(&root, &account, &key, cancel, |state| {
                    if let Ok(mut value) = progress.lock() {
                        *value = state.message;
                    }
                })?;
                let timestamp = if !result.cancelled {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs();
                    settings::update(|s| {
                        s.last_sync.insert(manifest.library_id.to_string(), now);
                    })?;
                    Some(now)
                } else {
                    None
                };
                Ok::<_, anyhow::Error>((result, timestamp))
            });
            futures::pin_mut!(task);
            let result = loop {
                let timer = cx
                    .background_executor()
                    .timer(std::time::Duration::from_millis(250));
                futures::pin_mut!(timer);
                match futures::future::select(task.as_mut(), timer).await {
                    futures::future::Either::Left((result, _)) => break result,
                    futures::future::Either::Right(_) => {
                        let text = display_progress
                            .lock()
                            .map(|v| v.clone())
                            .unwrap_or_default();
                        let _ = this.update(cx, |this, cx| {
                            if !text.is_empty() {
                                this.message = text;
                            }
                            cx.notify();
                        });
                    }
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.cancel = None;
                match result {
                    Ok((result, timestamp)) => {
                        this.message = result.message;
                        if timestamp.is_some() {
                            this.last_sync = timestamp;
                            cx.emit(SettingsEvent::LastSync(timestamp));
                        }
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.emit(SettingsEvent::Refresh);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::TestedKey;
    #[test]
    fn changed_key_or_profile_requires_another_test() {
        let test = TestedKey {
            key: "dummy".into(),
            input: "dummy".into(),
            profile: "profile".into(),
            account: "account".into(),
        };
        assert!(test.matches("dummy", "profile"));
        assert!(!test.matches("replacement", "profile"));
        assert!(!test.matches("dummy", "different-profile"));
    }
}
