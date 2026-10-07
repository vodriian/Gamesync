//! Native Play now state. Ranking and guarded record writes run on workers;
//! the view retains a draft or retry action when a write fails.
mod analyze;
mod controls;
mod game_section;
mod modal;
mod playing;
mod profile_editor;
mod view;

use super::thumb_cache::LruImageCache;
use crate::{
    assets::PlayIcon,
    model::{Game, Library},
};
use gamesync_desktop::{
    library::LibraryStore,
    recommendations::{self as rec, *},
};
use gpui::{prelude::*, Entity, FocusHandle, Task, Window};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Setup,
    Hand,
    Saved,
    Recent,
    Profiles,
}
#[derive(Clone, Copy, PartialEq)]
enum Modal {
    Context,
    Profile(Uuid),
}

pub struct OpenGame(pub Uuid);
impl gpui::EventEmitter<OpenGame> for PlayNowView {}

#[derive(Clone, PartialEq)]
enum Change {
    Saved(bool),
    Excluded(bool),
    Profile(Profile),
    Choice(Choice),
}

pub struct PlayNowView {
    library: Entity<Library>,
    cache: Entity<LruImageCache>,
    focus: FocusHandle,
    scroll: gpui::ScrollHandle,
    restore_focus: bool,
    list_limit: usize,
    screen: Screen,
    modal: Option<Modal>,
    context_before_edit: Option<rec::Context>,
    context: rec::Context,
    controls: controls::ControlMotion,
    session: Session,
    options: bool,
    selection: Selection,
    installations: local_steam::Installations,
    needs_rank: bool,
    ranking: bool,
    epoch: u64,
    rank_task: Option<Task<()>>,
    deal_pending: Option<bool>,
    saving: bool,
    launching: bool,
    pending_save: Option<(Uuid, Change)>,
    undo_exclusion: Option<Uuid>,
    message: String,
    feedback_game: Option<Uuid>,
    launch_error: bool,
    active_play: Option<ActivePlay>,
    session_saving: bool,
    ticker: Option<Task<()>>,
    draft: Option<profile_editor::Draft>,
    /// The app's one AI analysis job, shared with Settings → AI.
    analysis: Entity<crate::ui::analysis_job::AnalysisJob>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
fn activity_icon(a: Activity) -> &'static str {
    match a {
        Activity::Shoot => "gun",
        Activity::Fly => "airplane-02",
        Activity::Drive => "car-04",
        Activity::Explore => "maps-search",
        Activity::Horse => "horse-saddle",
        Activity::Cards => "spades",
        Activity::Multiplayer => "user-group-03",
        Activity::Runs => "infinity-square",
        Activity::Weird => "alien-02",
    }
}
fn energy_icon(e: Effort) -> &'static str {
    match e {
        Effort::Low => "battery-low",
        Effort::Medium => "battery-medium-01",
        Effort::High => "battery-full",
    }
}

impl PlayNowView {
    pub fn new(
        library: Entity<Library>,
        cache: Entity<LruImageCache>,
        active_play: Option<ActivePlay>,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        cx.observe(&library, |this, _, cx| {
            this.needs_rank = true;
            this.epoch += 1;
            cx.notify();
        })
        .detach();
        let demo = library.read(cx).demo;
        if !demo {
            cx.spawn(async move |this, cx| {
                let installations = cx.background_spawn(async { local_steam::scan() }).await;
                let _ = this.update(cx, |this, cx| {
                    this.installations = installations;
                    this.needs_rank = true;
                    this.epoch += 1;
                    cx.notify();
                });
            })
            .detach();
        }
        let analysis = cx
            .global::<crate::ui::analysis_job::AnalysisGlobal>()
            .0
            .clone();
        cx.observe(&analysis, |_, _, cx| cx.notify()).detach();
        let context = active_play
            .as_ref()
            .map_or_else(rec::Context::default, |active| active.context.clone());
        let mut view = Self {
            library,
            cache,
            focus: cx.focus_handle(),
            scroll: gpui::ScrollHandle::new(),
            restore_focus: true,
            list_limit: 60,
            screen: Screen::Setup,
            modal: None,
            context_before_edit: None,
            controls: controls::ControlMotion::new(&context),
            context,
            session: Session::default(),
            options: false,
            selection: Selection::default(),
            installations: local_steam::Installations::default(),
            needs_rank: true,
            ranking: false,
            epoch: 0,
            rank_task: None,
            deal_pending: None,
            saving: false,
            launching: false,
            pending_save: None,
            undo_exclusion: None,
            message: String::new(),
            feedback_game: None,
            launch_error: false,
            active_play,
            session_saving: false,
            ticker: None,
            draft: None,
            analysis,
        };
        view.start_timer(cx);
        view
    }
    pub fn busy(&self) -> bool {
        self.saving
            || self.session_saving
            || self.launching
            || self.pending_save.is_some()
            || self.draft.as_ref().is_some_and(|d| d.dirty)
    }
    pub(crate) fn visible_game(game: &Game) -> bool {
        game.record.as_ref().is_some_and(|record| {
            !record.deleted
                && !record.game.personal.hidden
                && record.game.steam.as_ref().is_none_or(|steam| steam.owned)
        })
    }
    fn game(&self, id: Uuid, cx: &gpui::App) -> Option<Game> {
        self.library
            .read(cx)
            .games
            .iter()
            .find(|game| game.id == id)
            .filter(|game| Self::visible_game(game))
            .cloned()
    }
    fn installed(&self, game: &Game) -> Option<bool> {
        let id = game.record.as_ref()?.game.steam.as_ref()?.app_id;
        self.installations.for_target(id, self.context.device)
    }
    fn change_context(&mut self, event: Option<&gpui::ClickEvent>, cx: &mut gpui::Context<Self>) {
        self.controls
            .retarget(&self.context, event.is_some_and(|e| !e.is_keyboard()), cx);
        self.needs_rank = true;
        self.epoch += 1;
        self.message.clear();
        cx.notify();
    }
    fn go(&mut self, screen: Screen, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.active_play.is_some() {
            return;
        }
        if self.saving || self.launching || self.pending_save.is_some() {
            self.message = if self.pending_save.is_some() && !self.saving {
                "Retry the saved change or discard it before leaving."
            } else {
                "Wait for the current action to finish."
            }
            .into();
            cx.notify();
            return;
        }
        self.list_limit = 60;
        self.scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.screen = screen;
        self.message.clear();
        window.focus(&self.focus);
        cx.notify();
    }
    pub fn restore_focus(&mut self, cx: &mut gpui::Context<Self>) {
        self.restore_focus = true;
        cx.notify();
    }
    fn open_game(&mut self, id: Uuid, cx: &mut gpui::Context<Self>) {
        if !self.busy() && self.game(id, cx).is_some() {
            self.message.clear();
            cx.emit(OpenGame(id));
        }
    }
    fn back(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.screen == Screen::Setup || self.screen == Screen::Hand {
            return;
        }
        if self.draft.as_ref().is_some_and(|d| d.dirty) {
            self.message = "Save the profile or discard your changes first.".into();
            cx.notify();
            return;
        }
        self.go(
            if self.session.hand.is_empty() {
                Screen::Setup
            } else {
                Screen::Hand
            },
            window,
            cx,
        );
    }
    fn refresh_rank(&mut self, cx: &mut gpui::Context<Self>) {
        self.needs_rank = false;
        self.ranking = true;
        let records: Vec<_> = self
            .library
            .read(cx)
            .games
            .iter()
            .filter_map(|g| g.record.clone())
            .collect();
        let statuses: BTreeSet<_> = self
            .library
            .read(cx)
            .statuses
            .iter()
            .filter(|s| s.recommendation_eligible)
            .map(|s| s.key.clone())
            .collect();
        let installed = self.installations.clone();
        let context = self.context.clone();
        let epoch = self.epoch;
        let suitability = gamesync_desktop::suitability::context();
        self.rank_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let candidates: Vec<_> = records
                        .iter()
                        .map(|record| {
                            let assessment = gamesync_desktop::suitability::assessment_in(
                                &record.game,
                                &suitability,
                            );
                            Candidate {
                                record,
                                status_eligible: statuses.contains(&record.game.personal.status),
                                installed: record
                                    .game
                                    .steam
                                    .as_ref()
                                    .and_then(|s| installed.for_target(s.app_id, context.device)),
                                blocked: assessment.is_some_and(|a| match context.device {
                                    Device::Pc => a.pc.blocker.is_some(),
                                    Device::SteamDeck => a.steam_deck.blocker.is_some(),
                                }),
                            }
                        })
                        .collect();
                    rank(&candidates, &context, now())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.ranking = false;
                if this.epoch != epoch {
                    this.needs_rank = true;
                    cx.notify();
                    return;
                }
                this.selection = result;
                if let Some(reshuffle) = this.deal_pending.take() {
                    this.session
                        .deal(this.context.clone(), &this.selection.ranked, reshuffle);
                    this.scroll
                        .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
                    this.screen = Screen::Hand;
                }
                cx.notify();
            });
        }));
    }
    fn deal(&mut self, reshuffle: bool, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.active_play.is_some() || self.busy() {
            return;
        }
        self.modal = None;
        self.context_before_edit = None;
        if self.needs_rank || self.ranking {
            self.deal_pending = Some(reshuffle);
            if !self.ranking {
                self.refresh_rank(cx);
            }
        } else {
            self.session
                .deal(self.context.clone(), &self.selection.ranked, reshuffle);
            self.screen = Screen::Hand;
        }
        self.scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.message.clear();
        window.focus(&self.focus);
        cx.notify();
    }
    fn save(&mut self, id: Uuid, change: Change, cx: &mut gpui::Context<Self>) {
        if self.saving {
            return;
        }
        if self
            .pending_save
            .as_ref()
            .is_some_and(|(pending_id, pending)| *pending_id != id || *pending != change)
        {
            self.message =
                "Retry the previous change or discard it before making another change.".into();
            cx.notify();
            return;
        }
        // Retain intent even when the reader already knows storage is unavailable.
        self.pending_save = Some((id, change.clone()));
        self.feedback_game = Some(id);
        let library = self.library.read(cx);
        if let Some(issue) = &library.write_issue {
            self.message = format!("Cannot save: {issue}");
            cx.notify();
            return;
        }
        let Some((root, manifest)) = library.source.clone() else {
            self.message = "Wait for the library to finish opening.".into();
            cx.notify();
            return;
        };
        let Some(base) = self.game(id, cx).and_then(|g| g.record) else {
            self.message = "This game is no longer available.".into();
            cx.notify();
            return;
        };
        let mut personal = base.game.personal.clone();
        match &change {
            Change::Saved(value) => personal.play_now.saved = *value,
            Change::Excluded(value) => personal.play_now.excluded = *value,
            Change::Profile(value) => personal.play_now.profile = value.clone(),
            Change::Choice(choice) => personal.play_now.record_choice(choice.clone()),
        }
        self.saving = true;
        self.message = "Saving…".into();
        cx.spawn(async move |this, cx| {
            let save_root = root.clone();
            let result = cx.background_spawn(async move {
                LibraryStore::open(save_root)?.edit_game_personal(
                    &manifest, id, base.revision_id, personal,
                )
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(record) => {
                        let finished = matches!(&change, Change::Choice(choice) if choice.finished_at.is_some());
                        this.pending_save = None;
                        if this.library.read(cx).source.as_ref().is_some_and(|(p, _)| *p == root) {
                            this.library.update(cx, |lib, cx| {
                                lib.apply_personal_record(record);
                                cx.notify();
                            });
                        }
                        this.message = match change {
                            Change::Excluded(true) => {
                                this.undo_exclusion = Some(id);
                                "Removed from recommendations. Your library is unchanged."
                            }
                            Change::Excluded(false) => {
                                this.undo_exclusion = None;
                                "Allowed in recommendations again."
                            }
                            Change::Saved(true) => "Saved for later.",
                            Change::Saved(false) => "Removed from saved picks.",
                            // The modal closes on save; a note would linger in game details.
                            Change::Profile(_) => {
                                this.draft = None;
                                this.modal = None;
                                this.feedback_game = None;
                                ""
                            }
                            Change::Choice(choice) => match choice.launch {
                                LaunchOutcome::Failed => "Launch failed. Your choice was saved; you can retry.",
                                LaunchOutcome::Accepted => "",
                                LaunchOutcome::NotRequested => "",
                            },
                        }.into();
                        if finished {
                            this.finish_local(cx);
                        }
                    }
                    Err(error) => {
                        this.message = format!("Save failed: {error:#}. Your change is kept. Refresh the library and retry.");
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
