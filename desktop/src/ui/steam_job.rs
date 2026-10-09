//! One Steam sync job shared by the sidebar and Settings. Network and disk work
//! stay on a worker; closing Settings does not stop progress or cancellation.
use crate::model::Library;
use gamesync_desktop::{
    credentials::{CredentialStore, OsCredential},
    settings, steam,
};
use gpui::{prelude::*, Entity, EventEmitter, Global};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct SteamGlobal(pub Entity<SteamJob>);
impl Global for SteamGlobal {}
pub struct Finished;

pub struct SteamJob {
    library: Entity<Library>,
    cancel: Option<steam::Cancellation>,
    pub message: String,
    pub last_sync: Option<u64>,
}
impl EventEmitter<Finished> for SteamJob {}
impl SteamJob {
    pub fn new(library: Entity<Library>) -> Self {
        Self {
            library,
            cancel: None,
            message: String::new(),
            last_sync: None,
        }
    }
    pub fn running(&self) -> bool {
        self.cancel.is_some()
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
            self.message = "Cancelling after the current request…".into();
            cx.notify();
        }
    }
    pub fn start(&mut self, cx: &mut Context<Self>) {
        if self.running() || self.library.read(cx).demo {
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
                this.cancel = None;
                match result {
                    Ok((result, timestamp)) => {
                        this.message = result.message;
                        if timestamp.is_some() {
                            this.last_sync = timestamp;
                        }
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.emit(Finished);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
