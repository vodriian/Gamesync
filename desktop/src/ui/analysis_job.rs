//! One AI analysis job for the app, shared by Play now and Settings → AI so
//! both show the same progress and only one job runs. Requests run in small
//! batches on workers. Each validated result is saved by game ID as soon as
//! its batch returns, so cancel or a later failure keeps earlier results.
use crate::model::{Game, Library};
use gamesync_desktop::{
    ai::{self, Connection},
    recommendations::analysis,
    record_store::RecordStore,
    settings::{self, AiProvider},
};
use gpui::{div, prelude::*, px, Entity, Window};
use gpui_component::{
    button::Button, h_flex, v_flex, ActiveTheme as _, Selectable as _, WindowExt as _,
};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};
use uuid::Uuid;

pub struct AnalysisGlobal(pub Entity<AnalysisJob>);
impl gpui::Global for AnalysisGlobal {}

/// Which games a batch covers. Hidden games are outside every scope.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Play now candidates: eligible statuses, not excluded from Play now.
    PlayNow,
    /// Every library game except hidden ones (Settings → AI).
    Library,
}

/// A provider ready for analysis: added in Settings with a chosen model.
#[derive(Clone)]
struct Ready {
    id: &'static str,
    entry: AiProvider,
}
impl Ready {
    fn model(&self) -> &str {
        self.entry.model.as_deref().unwrap_or_default()
    }
    fn label(&self) -> String {
        let name = ai::provider(self.id).map_or(self.id, |p| p.name);
        format!("{name} ({})", self.model())
    }
}

struct Job {
    provider: Ready,
    /// Read once per job, so secure storage asks at most once.
    connection: Option<Arc<Connection>>,
    queue: VecDeque<Uuid>,
    total: usize,
    saved: usize,
    unanswered: usize,
    /// Skip the needs-analysis check: the user asked to analyze again.
    force: bool,
    cancel: bool,
}

pub struct AnalysisJob {
    library: Entity<Library>,
    providers: Vec<Ready>,
    choice: usize,
    job: Option<Job>,
    /// A stopped job's remaining games, for Retry.
    retry: Option<Job>,
    status: String,
}

fn ready_providers() -> Vec<Ready> {
    let saved = settings::load().map(|s| s.ai_providers).unwrap_or_default();
    ai::PROVIDERS
        .iter()
        .filter_map(|p| {
            let entry = saved.get(p.id)?.clone();
            entry.model.is_some().then_some(Ready { id: p.id, entry })
        })
        .collect()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// A library game that can be analyzed: present, owned, and not hidden.
fn analyzable(game: &Game) -> bool {
    crate::ui::play_now::PlayNowView::visible_game(game)
}

/// Whether a batch in `scope` sends this game. Hidden, removed, and unowned
/// games are never sent; neither is a game with a current analysis.
fn in_scope(
    scope: Scope,
    record: &gamesync_desktop::records::GameRevision,
    eligible_statuses: &BTreeSet<&str>,
) -> bool {
    let game = &record.game;
    let library_game = !record.deleted
        && !game.personal.hidden
        && game.steam.as_ref().is_none_or(|steam| steam.owned);
    let wanted = match scope {
        Scope::Library => true,
        Scope::PlayNow => {
            eligible_statuses.contains(game.personal.status.as_str())
                && !game.personal.play_now.excluded
        }
    };
    library_game && wanted && analysis::needs_analysis(game)
}

impl AnalysisJob {
    pub fn new(library: Entity<Library>) -> Self {
        Self {
            library,
            providers: Vec::new(),
            choice: 0,
            job: None,
            retry: None,
            status: String::new(),
        }
    }
    pub fn running(&self) -> bool {
        self.job.is_some()
    }
    pub fn cancelling(&self) -> bool {
        self.job.as_ref().is_some_and(|j| j.cancel)
    }
    pub fn can_retry(&self) -> bool {
        self.retry.is_some()
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    fn chosen(&self) -> Option<&Ready> {
        self.providers.get(self.choice)
    }

    /// The newest current analysis in this library, in Unix seconds.
    pub fn last_run(&self, cx: &gpui::App) -> Option<i64> {
        self.library
            .read(cx)
            .games
            .iter()
            .filter_map(|g| analysis::current_analysis(&g.record.as_ref()?.game))
            .map(|a| a.analyzed_at)
            .max()
    }

    /// Demo libraries hold bundled samples; they are never sent for analysis.
    pub fn available(&self, cx: &gpui::App) -> bool {
        !self.library.read(cx).demo
    }

    /// Games without a current analysis in `scope`, in library order.
    pub fn targets(&self, scope: Scope, cx: &gpui::App) -> Vec<Uuid> {
        let library = self.library.read(cx);
        let statuses: BTreeSet<_> = library
            .statuses
            .iter()
            .filter(|s| s.recommendation_eligible)
            .map(|s| s.key.as_str())
            .collect();
        library
            .games
            .iter()
            .filter(|g| {
                g.record
                    .as_ref()
                    .is_some_and(|r| in_scope(scope, r, &statuses))
            })
            .map(|g| g.id)
            .collect()
    }

    fn load_providers(&mut self) -> bool {
        self.providers = ready_providers();
        if self.choice >= self.providers.len() {
            self.choice = 0;
        }
        if self.providers.is_empty() {
            self.status = "Add an AI provider and choose a model in Settings → AI first.".into();
            return false;
        }
        true
    }

    /// Batch: confirm the count, provider, and what is sent before any request.
    pub fn confirm(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        self.retry = None;
        if !self.load_providers() {
            cx.notify();
            return;
        }
        let targets = self.targets(scope, cx);
        if targets.is_empty() {
            self.status = match scope {
                Scope::PlayNow => "Every eligible game has a current profile.",
                Scope::Library => "Every game has a current profile.",
            }
            .into();
            cx.notify();
            return;
        }
        let count = targets.len();
        let requests = count.div_ceil(analysis::BATCH_SIZE);
        let job = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let state = job.read(cx);
            let local = state
                .chosen()
                .and_then(|r| ai::provider(r.id))
                .is_some_and(|p| p.local());
            let choices = state
                .providers
                .iter()
                .enumerate()
                .map(|(index, ready)| {
                    let target = job.clone();
                    Button::new(("ai-provider", index))
                        .outline()
                        .label(ready.label())
                        .selected(index == state.choice)
                        .on_click(move |_, _, cx| {
                            target.update(cx, |job, cx| {
                                job.choice = index;
                                cx.notify();
                            })
                        })
                })
                .collect::<Vec<_>>();
            let start = job.clone();
            let games = if count == 1 { "game" } else { "games" };
            let calls = if requests == 1 { "request" } else { "requests" };
            let (title, summary) = match scope {
                Scope::PlayNow => (
                    "Analyze for Play now",
                    format!("Estimate energy, session length, stopping points, and activities for {count} {games} without a current profile. This uses {requests} {calls}."),
                ),
                Scope::Library => (
                    "Analyze games",
                    format!("Add Play now data to {count} {games} without a current profile. Hidden games are skipped. This uses {requests} {calls}."),
                ),
            };
            dialog
                .title(title)
                .w(px(500.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(summary)
                        .when(choices.len() > 1, |col| {
                            col.child(h_flex().flex_wrap().gap_2().children(choices))
                        })
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(if local {
                                    "GameSync sends each title, Steam description, tags, and genres to your local server. Notes, ratings, and history are not sent."
                                } else {
                                    "GameSync sends each title, Steam description, tags, and genres to the provider. Notes, ratings, and history are not sent. The provider can charge for these requests."
                                }),
                        ),
                )
                .confirm()
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default().ok_text("Analyze"),
                )
                .on_ok(move |_, _, cx| {
                    start.update(cx, |job, cx| {
                        // Recount at start: games may have changed while the dialog was open.
                        let targets = job.targets(scope, cx);
                        job.start(targets, false, cx);
                    });
                    true
                })
        });
    }

    /// One game, from its profile: the click is the explicit request.
    pub fn analyze_one(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        self.retry = None;
        if self.load_providers() {
            self.start(vec![id], true, cx);
        }
        cx.notify();
    }

    fn start(&mut self, ids: Vec<Uuid>, force: bool, cx: &mut Context<Self>) {
        let Some(provider) = self.chosen().cloned() else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        self.job = Some(Job {
            provider,
            connection: None,
            total: ids.len(),
            queue: ids.into(),
            saved: 0,
            unanswered: 0,
            force,
            cancel: false,
        });
        self.next_batch(cx);
    }

    pub fn retry(&mut self, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        if let Some(mut job) = self.retry.take() {
            job.cancel = false;
            self.job = Some(job);
            self.next_batch(cx);
        }
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &mut self.job {
            job.cancel = true;
            self.status = "Stopping after the current request…".into();
            cx.notify();
        }
    }

    fn progress(job: &Job) -> String {
        let done = job.total - job.queue.len();
        format!(
            "Analyzing {done} of {} with {}…",
            job.total,
            job.provider.label()
        )
    }

    fn finish(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        let Some(job) = self.job.take() else {
            return;
        };
        let mut status = match job.saved {
            1 => "1 profile saved.".to_owned(),
            n => format!("{n} profiles saved."),
        };
        if job.unanswered > 0 {
            status.push_str(&format!(
                " {} without a usable answer; they stay on local estimates.",
                job.unanswered
            ));
        }
        if let Some(error) = error {
            status = format!("Analysis stopped: {error}. {status}");
            if !job.queue.is_empty() {
                self.retry = Some(job);
            }
        } else if job.cancel && !job.queue.is_empty() {
            status = format!("Analysis cancelled. {status}");
        }
        self.status = status;
        cx.notify();
    }

    /// Take the next batch from the current records. Games that were hidden,
    /// removed, or analyzed elsewhere since the job started are skipped.
    fn next_batch(&mut self, cx: &mut Context<Self>) {
        let Some(job) = self.job.as_ref() else {
            return;
        };
        if job.cancel || job.queue.is_empty() {
            return self.finish(None, cx);
        }
        let library = self.library.read(cx);
        if let Some(issue) = library.write_issue.clone() {
            return self.finish(Some(format!("cannot save ({issue})")), cx);
        }
        let Some((root, _)) = library.source.clone() else {
            return self.finish(Some("the library is not open".into()), cx);
        };
        let Some(job) = self.job.as_mut() else {
            return;
        };
        let library = self.library.read(cx);
        let mut games = Vec::new();
        while games.len() < analysis::BATCH_SIZE {
            let Some(id) = job.queue.pop_front() else {
                break;
            };
            let current = library
                .games
                .iter()
                .find(|g| g.id == id)
                .filter(|g| analyzable(g))
                .and_then(|g| g.record.as_ref())
                .filter(|r| job.force || analysis::needs_analysis(&r.game));
            if let Some(record) = current {
                games.push((id, record.game.clone()));
            }
        }
        if games.is_empty() {
            return self.next_batch(cx);
        }
        let batch_ids: Vec<Uuid> = games.iter().map(|(id, _)| *id).collect();
        let provider = job.provider.clone();
        let connection = job.connection.clone();
        self.status = Self::progress(job);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let connection = match connection {
                        Some(c) => c,
                        None => Arc::new(Connection::saved(provider.id, &provider.entry)?),
                    };
                    let refs: Vec<_> = games.iter().map(|(id, g)| (*id, g)).collect();
                    let (user, batch) = analysis::request(&refs);
                    let text = connection.complete_json(
                        provider.model(),
                        analysis::SYSTEM,
                        &user,
                        analysis::SCHEMA_NAME,
                        &analysis::schema(),
                    )?;
                    let outcome =
                        analysis::parse(&text, &batch, provider.id, provider.model(), now())?;
                    let store = RecordStore::open(&root)?;
                    let mut saved = Vec::new();
                    let mut unsaved = outcome.missing.len();
                    for result in outcome.analyses {
                        // A game edited during the request keeps its local estimate.
                        match store.set_recommendation_analysis(result.game_id, result) {
                            Ok(record) => saved.push(record),
                            Err(_) => unsaved += 1,
                        }
                    }
                    anyhow::Ok((connection, saved, unsaved))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(job) = this.job.as_mut() else {
                    return;
                };
                match result {
                    Ok((connection, saved, unsaved)) => {
                        job.connection = Some(connection);
                        job.saved += saved.len();
                        job.unanswered += unsaved;
                        if !saved.is_empty() {
                            this.library.update(cx, |lib, cx| {
                                lib.apply_personal_records(saved);
                                cx.notify();
                            });
                        }
                        this.next_batch(cx);
                    }
                    Err(error) => {
                        // Put the failed batch back so Retry includes it.
                        for id in batch_ids.into_iter().rev() {
                            job.queue.push_front(id);
                        }
                        this.finish(Some(format!("{error:#}")), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamesync_desktop::records::{GameData, GameRevision};

    fn record(edit: impl FnOnce(&mut GameRevision)) -> GameRevision {
        let mut record = GameRevision {
            schema_version: 1,
            game_id: Uuid::from_u128(1),
            revision_id: Uuid::from_u128(2),
            parents: vec![],
            deleted: false,
            game: GameData::new("Game"),
            extra: Default::default(),
        };
        record.game.personal.status = "completed".into();
        edit(&mut record);
        record
    }

    #[test]
    fn library_scope_skips_hidden_but_not_finished_or_excluded_games() {
        let eligible = BTreeSet::from(["backlog"]);
        // Completed and excluded games are outside Play now but inside the library.
        let finished = record(|r| r.game.personal.play_now.excluded = true);
        assert!(in_scope(Scope::Library, &finished, &eligible));
        assert!(!in_scope(Scope::PlayNow, &finished, &eligible));
        let hidden = record(|r| r.game.personal.hidden = true);
        assert!(!in_scope(Scope::Library, &hidden, &eligible));
        let removed = record(|r| r.deleted = true);
        assert!(!in_scope(Scope::Library, &removed, &eligible));
        let backlog = record(|r| r.game.personal.status = "backlog".into());
        assert!(in_scope(Scope::PlayNow, &backlog, &eligible));
    }
}
