//! Glaze's grouped settings layout, backed by native device services.
mod ai;
mod view;
use crate::model::Library;
use gamesync_desktop::{
    credentials::{replace_checked, CredentialStore, SteamCredential},
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
    Theme(String),
    Refresh,
    LastSync(Option<u64>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Appearance,
    Ai,
}
impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Look and feel",
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
    library_id: Option<Uuid>,
    ai: Entity<ai::AiSettings>,
    key_saved: bool,
    theme: String,
    section: Section,
    tested: Option<TestedKey>,
    clear_key: bool,
    reduce_motion: bool,
    busy: bool,
    cancel: Option<steam::Cancellation>,
    message: String,
    last_sync: Option<u64>,
}
impl EventEmitter<SettingsEvent> for SettingsView {}
impl SettingsView {
    pub fn new(
        library: Entity<Library>,
        theme: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        Self {
            library,
            profile: Self::masked_input("Steam profile link or SteamID64", window, cx),
            key: Self::masked_input("Enter a key to save or replace", window, cx),
            library_id: None,
            ai: cx.new(|cx| ai::AiSettings::new(window, cx)),
            key_saved: false,
            theme,
            section: Section::General,
            tested: None,
            clear_key: false,
            reduce_motion: super::motion::reduced(cx),
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
    pub fn show_general(&mut self, cx: &mut Context<Self>) {
        self.section = Section::General;
        cx.notify();
    }
    pub fn busy(&self) -> bool {
        self.busy
    }
    pub fn set_theme(&mut self, theme: String, cx: &mut Context<Self>) {
        self.theme = theme;
        cx.notify();
    }
    fn read_connection(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let last = settings::load()?.last_sync.get(&id.to_string()).copied();
                    let credential = SteamCredential::new(id)?.get();
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
                        SteamCredential::new(manifest.library_id)?
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
                match result {
                    Ok(test) => {
                        this.tested = Some(test);
                        this.message = "Test passed. Save key to keep this connection.".into();
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
                    replace_checked(&SteamCredential::new(manifest.library_id)?, &key, bind)
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
                .background_spawn(async move { SteamCredential::new(id)?.remove() })
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
                let key = SteamCredential::new(manifest.library_id)?
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
