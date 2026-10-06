//! App-wide Best on state: the user's rules and optional ProtonDB enrichment.
//! Each change replaces the assessment context and recalculates the library;
//! assessments are never saved. Settings and the detail panel observe this.

use crate::model::Library;
use gamesync_desktop::{
    protondb::{self, ProtonData},
    suitability::{self, Rules},
};
use gpui::{AppContext as _, Context, Entity, Task};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

/// The app's single Best on state, for views created deep in the tree.
pub struct BestOnGlobal(pub Entity<BestOnState>);
impl gpui::Global for BestOnGlobal {}

#[derive(Clone, Debug, PartialEq)]
pub enum Enrichment {
    Idle,
    /// Bytes downloaded, and the total when known.
    Downloading(u64, Option<u64>),
    Failed(String),
}

pub struct BestOnState {
    library: Entity<Library>,
    pub rules: Rules,
    /// ProtonDB enrichment is on for this device.
    pub protondb: bool,
    pub data: Option<ProtonData>,
    pub enrichment: Enrichment,
    /// Set by "Edit rules in Settings"; the Settings view opens Best on and clears it.
    pub show_rules: bool,
    cancel: Arc<AtomicBool>,
    task: Option<Task<()>>,
    save: Option<Task<()>>,
    pub message: String,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

impl BestOnState {
    pub fn new(library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        let settings = crate::settings::load().unwrap_or_default();
        // An unreadable cache counts as no data; the next update replaces it.
        let data = settings
            .protondb
            .then(|| protondb::load().ok().flatten())
            .flatten();
        let mut state = Self {
            library,
            rules: settings.best_on_rules,
            protondb: settings.protondb,
            data,
            enrichment: Enrichment::Idle,
            show_rules: false,
            cancel: Arc::new(AtomicBool::new(false)),
            task: None,
            save: None,
            message: String::new(),
        };
        state.apply(cx);
        // A monthly check, only when enrichment is on.
        if state.protondb && state.data.as_ref().is_none_or(|data| data.due(now())) {
            state.update_now(cx);
        }
        state
    }

    /// Replace the assessment context and recalculate the library.
    pub fn apply(&mut self, cx: &mut Context<Self>) {
        suitability::set_context(suitability::Context {
            rules: self.rules.clone(),
            proton: self
                .data
                .as_ref()
                .filter(|_| self.protondb)
                .map(|data| Arc::new(data.apps.clone())),
        });
        self.library.update(cx, |library, cx| {
            library.refresh_best_on();
            cx.notify();
        });
        cx.notify();
    }

    /// Save and apply new rules. Saves are chained so the newest rules win.
    pub fn set_rules(&mut self, rules: Rules, cx: &mut Context<Self>) {
        if rules == self.rules {
            return;
        }
        self.rules = rules.clone();
        self.apply(cx);
        let previous = self.save.take();
        self.save = Some(cx.spawn(async move |this, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = cx
                .background_spawn(async move {
                    crate::settings::update(|settings| settings.best_on_rules = rules)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.message = match result {
                    Ok(()) => String::new(),
                    Err(error) => format!("Could not save Best on rules: {error}"),
                };
                cx.notify();
            });
        }));
    }

    /// Rules that data sync wrote from another device.
    pub fn reload_rules(&mut self, rules: Rules, cx: &mut Context<Self>) {
        if rules != self.rules {
            self.rules = rules;
            self.apply(cx);
        }
    }

    /// Turning enrichment on downloads data; turning it off deletes it.
    pub fn set_protondb(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if enabled == self.protondb {
            return;
        }
        self.protondb = enabled;
        self.message.clear();
        if enabled {
            self.update_now(cx);
        } else {
            self.cancel.store(true, Ordering::Relaxed);
            self.task = None;
            self.data = None;
            self.enrichment = Enrichment::Idle;
            self.apply(cx);
        }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::settings::update(|settings| settings.protondb = enabled)?;
                    if !enabled {
                        protondb::remove()?;
                    }
                    anyhow::Ok(())
                })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.message = format!("Could not save the ProtonDB setting: {error:#}");
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Download the newest export when it is new. The previous data stays in
    /// use until the new data is saved.
    pub fn update_now(&mut self, cx: &mut Context<Self>) {
        if matches!(self.enrichment, Enrichment::Downloading(..)) {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        let read = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(u64::MAX));
        let done = Arc::new(AtomicBool::new(false));
        self.enrichment = Enrichment::Downloading(0, None);
        cx.notify();
        let current = self.data.clone();
        let work = {
            let (read, total, done, cancel) =
                (read.clone(), total.clone(), done.clone(), cancel.clone());
            cx.background_spawn(async move {
                let mut progress = |bytes: u64, size: Option<u64>| {
                    read.store(bytes, Ordering::Relaxed);
                    total.store(size.unwrap_or(u64::MAX), Ordering::Relaxed);
                };
                let result = protondb::update(current.as_ref(), &cancel, &mut progress, now());
                done.store(true, Ordering::Relaxed);
                result
            })
        };
        self.task = Some(cx.spawn(async move |this, cx| {
            // Show progress while the download runs.
            while !done.load(Ordering::Relaxed) {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let bytes = read.load(Ordering::Relaxed);
                let size = Some(total.load(Ordering::Relaxed)).filter(|size| *size != u64::MAX);
                let Ok(()) = this.update(cx, |this, cx| {
                    if matches!(this.enrichment, Enrichment::Downloading(..)) {
                        this.enrichment = Enrichment::Downloading(bytes, size);
                        cx.notify();
                    }
                }) else {
                    return;
                };
            }
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                // Turned off or cancelled while downloading: drop the result.
                if !this.protondb || cancel.load(Ordering::Relaxed) {
                    if this.protondb {
                        this.enrichment = Enrichment::Idle;
                        cx.notify();
                    }
                    return;
                }
                match result {
                    Ok(data) => {
                        this.data = Some(data);
                        this.enrichment = Enrichment::Idle;
                        this.apply(cx);
                    }
                    Err(error) => {
                        log::warn!("ProtonDB update failed: {error:#}");
                        this.enrichment = Enrichment::Failed(format!("{error:#}"));
                        cx.notify();
                    }
                }
            });
        }));
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        self.enrichment = Enrichment::Idle;
        cx.notify();
    }
}
