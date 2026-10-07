//! Analyze for Play now. Requests run in small batches on workers. Each
//! validated result is saved by game ID as soon as its batch returns, so
//! cancel or a later failure keeps earlier results. Dealing never waits for this.
use super::*;
use gamesync_desktop::{
    ai::{self, Connection},
    recommendations::analysis,
    record_store::RecordStore,
    settings::{self, AiProvider},
};
use gpui::{div, px, AnyElement, SharedString};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
    WindowExt as _,
};
use std::{collections::VecDeque, sync::Arc};

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

#[derive(Default)]
pub(super) struct Analyzer {
    providers: Vec<Ready>,
    choice: usize,
    job: Option<Job>,
    /// A stopped job's remaining games, for Retry.
    retry: Option<Job>,
    status: String,
}
impl Analyzer {
    pub fn running(&self) -> bool {
        self.job.is_some()
    }
    fn chosen(&self) -> Option<&Ready> {
        self.providers.get(self.choice)
    }
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

impl PlayNowView {
    /// Eligible games without a current analysis, in library order. Hidden,
    /// excluded, and ineligible-status games are never sent.
    fn analysis_targets(&self, cx: &gpui::App) -> Vec<Uuid> {
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
            .filter(|g| Self::visible_game(g))
            .filter_map(|g| Some((g.id, &g.record.as_ref()?.game)))
            .filter(|(_, game)| {
                statuses.contains(game.personal.status.as_str())
                    && !game.personal.play_now.excluded
                    && analysis::needs_analysis(game)
            })
            .map(|(id, _)| id)
            .collect()
    }

    fn load_providers(&mut self) -> bool {
        self.analyzer.providers = ready_providers();
        if self.analyzer.choice >= self.analyzer.providers.len() {
            self.analyzer.choice = 0;
        }
        if self.analyzer.providers.is_empty() {
            self.analyzer.status =
                "Add an AI provider and choose a model in Settings → AI first.".into();
            return false;
        }
        true
    }

    /// Batch scope: confirm the count, provider, and what is sent before any request.
    pub(super) fn confirm_analysis(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.analyzer.running() {
            return;
        }
        self.analyzer.retry = None;
        if !self.load_providers() {
            cx.notify();
            return;
        }
        let targets = self.analysis_targets(cx);
        if targets.is_empty() {
            self.analyzer.status = "Every eligible game has a current profile.".into();
            cx.notify();
            return;
        }
        let count = targets.len();
        let requests = count.div_ceil(analysis::BATCH_SIZE);
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let state = view.read(cx);
            let chosen = state.analyzer.chosen().cloned();
            let local = chosen
                .as_ref()
                .and_then(|r| ai::provider(r.id))
                .is_some_and(|p| p.local());
            let choices = state
                .analyzer
                .providers
                .iter()
                .enumerate()
                .map(|(index, ready)| {
                    let target = view.clone();
                    Button::new(("ai-provider", index))
                        .outline()
                        .label(ready.label())
                        .selected(index == state.analyzer.choice)
                        .on_click(move |_, _, cx| {
                            target.update(cx, |view, cx| {
                                view.analyzer.choice = index;
                                cx.notify();
                            })
                        })
                })
                .collect::<Vec<_>>();
            let start = view.clone();
            let games = if count == 1 { "game" } else { "games" };
            let calls = if requests == 1 { "request" } else { "requests" };
            dialog
                .title("Analyze for Play now")
                .w(px(500.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(format!(
                            "Estimate energy, session length, stopping points, and activities for {count} {games} without a current profile. This uses {requests} {calls}."
                        ))
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
                    start.update(cx, |view, cx| {
                        let targets = view.analysis_targets(cx);
                        view.start_analysis(targets, false, cx);
                    });
                    true
                })
        });
    }

    /// One game, from its profile: the click is the explicit request.
    pub(super) fn analyze_one(&mut self, id: Uuid, cx: &mut gpui::Context<Self>) {
        if self.analyzer.running() || self.game(id, cx).is_none() {
            return;
        }
        self.analyzer.retry = None;
        if self.load_providers() {
            self.start_analysis(vec![id], true, cx);
        }
        cx.notify();
    }

    fn start_analysis(&mut self, ids: Vec<Uuid>, force: bool, cx: &mut gpui::Context<Self>) {
        let Some(provider) = self.analyzer.chosen().cloned() else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        self.analyzer.job = Some(Job {
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

    pub(super) fn retry_analysis(&mut self, cx: &mut gpui::Context<Self>) {
        if self.analyzer.running() {
            return;
        }
        if let Some(mut job) = self.analyzer.retry.take() {
            job.cancel = false;
            self.analyzer.job = Some(job);
            self.next_batch(cx);
        }
    }

    pub(super) fn cancel_analysis(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(job) = &mut self.analyzer.job {
            job.cancel = true;
            self.analyzer.status = "Stopping after the current request…".into();
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

    fn finish_analysis(&mut self, error: Option<String>, cx: &mut gpui::Context<Self>) {
        let Some(job) = self.analyzer.job.take() else {
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
                self.analyzer.retry = Some(job);
            }
        } else if job.cancel && !job.queue.is_empty() {
            status = format!("Analysis cancelled. {status}");
        }
        self.analyzer.status = status;
        cx.notify();
    }

    /// Take the next batch from the current records. Games that were hidden,
    /// removed, or analyzed elsewhere since the job started are skipped.
    fn next_batch(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(job) = self.analyzer.job.as_ref() else {
            return;
        };
        if job.cancel || job.queue.is_empty() {
            return self.finish_analysis(None, cx);
        }
        let library = self.library.read(cx);
        if let Some(issue) = library.write_issue.clone() {
            return self.finish_analysis(Some(format!("cannot save ({issue})")), cx);
        }
        let Some((root, _)) = library.source.clone() else {
            return self.finish_analysis(Some("the library is not open".into()), cx);
        };
        let Some(job) = self.analyzer.job.as_mut() else {
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
                .filter(|g| Self::visible_game(g))
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
        self.analyzer.status = Self::progress(job);
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
                let Some(job) = this.analyzer.job.as_mut() else {
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
                        this.finish_analysis(Some(format!("{error:#}")), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Toolbar control: Analyze, or progress with Cancel, or Retry after a failure.
    pub(super) fn analysis_control(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let status = (!self.analyzer.status.is_empty()).then(|| {
            div()
                .max_w(px(360.))
                .truncate()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(self.analyzer.status.clone()))
        });
        let button = if self.analyzer.running() {
            Button::new("analysis-cancel")
                .ghost()
                .label("Cancel")
                .disabled(self.analyzer.job.as_ref().is_some_and(|j| j.cancel))
                .on_click(cx.listener(|this, _, _, cx| this.cancel_analysis(cx)))
        } else if self.analyzer.retry.is_some() {
            Button::new("analysis-retry")
                .ghost()
                .label("Retry")
                .icon(PlayIcon("ai-beautify"))
                .on_click(cx.listener(|this, _, _, cx| this.retry_analysis(cx)))
        } else {
            Button::new("analysis-start")
                .ghost()
                .label("Analyze")
                .icon(PlayIcon("ai-beautify"))
                .tooltip("Estimate missing game profiles with AI")
                .disabled(self.active_play.is_some())
                .on_click(cx.listener(|this, _, w, cx| this.confirm_analysis(w, cx)))
        };
        h_flex()
            .gap_2()
            .children(status)
            .child(button)
            .into_any_element()
    }

    /// Profile modal: the cached analysis for this game and a one-game request.
    pub(super) fn analysis_summary(&self, id: Uuid, cx: &mut gpui::Context<Self>) -> AnyElement {
        let game = self.game(id, cx);
        let record = game.as_ref().and_then(|g| g.record.as_ref());
        let current = record.and_then(|r| analysis::current_analysis(&r.game));
        let stale =
            record.is_some_and(|r| r.game.recommendation_analysis.is_some()) && current.is_none();
        let muted = cx.theme().muted_foreground;
        let source = current.map(|a| {
            let name = ai::provider(&a.provider).map_or(a.provider.as_str(), |p| p.name);
            format!(
                "AI estimate from {name} ({}), confidence {}%.",
                a.model, a.confidence
            )
        });
        let running = self.analyzer.running();
        v_flex()
            .gap_2()
            .p_3()
            .rounded(px(7.))
            .bg(cx.theme().secondary)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_sm().child(source.unwrap_or_else(|| {
                        if stale {
                            "The AI estimate is out of date because the game details changed."
                                .into()
                        } else {
                            "No AI estimate yet.".into()
                        }
                    })))
                    .child(
                        Button::new("analysis-one")
                            .small()
                            .outline()
                            .icon(PlayIcon("ai-beautify"))
                            .label(if current.is_some() {
                                "Analyze again"
                            } else {
                                "Analyze"
                            })
                            .disabled(running || self.saving)
                            .on_click(cx.listener(move |this, _, _, cx| this.analyze_one(id, cx))),
                    ),
            )
            .children(
                current
                    .and_then(|a| a.reason.clone())
                    .map(|reason| div().text_sm().child(SharedString::from(reason))),
            )
            .child(div().text_xs().text_color(muted).child(if running {
                SharedString::from(self.analyzer.status.clone())
            } else if self.analyzer.status.is_empty() {
                "Analyze sends the title, Steam description, tags, and genres to your AI provider."
                    .into()
            } else {
                SharedString::from(self.analyzer.status.clone())
            }))
            .into_any_element()
    }
}
