//! Session state survives navigation and restart. Save it before requesting a
//! launch; clear it only after Done playing has been durably recorded.
use super::*;

impl PlayNowView {
    pub fn timer_label(&self) -> Option<String> {
        self.active_play.as_ref().map(|active| active.timer(now()))
    }

    pub(super) fn start_timer(&mut self, cx: &mut gpui::Context<Self>) {
        if self.active_play.is_none() || self.ticker.is_some() {
            return;
        }
        self.ticker = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(1))
                .await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }));
    }

    pub(super) fn start_play(&mut self, id: Uuid, cx: &mut gpui::Context<Self>) {
        if self.active_play.is_some() || self.busy() {
            return;
        }
        let Some(game) = self.game(id, cx) else {
            return;
        };
        let Some(record) = game.record.as_ref() else {
            return;
        };
        let Some((_, manifest)) = self.library.read(cx).source.as_ref() else {
            return;
        };
        let candidate = Candidate {
            record,
            status_eligible: self
                .library
                .read(cx)
                .statuses
                .iter()
                .any(|s| s.key == game.status && s.recommendation_eligible),
            installed: self.installed(&game),
            blocked: gamesync_desktop::suitability::assessment(&record.game).is_some_and(|a| {
                match self.context.device {
                    Device::Pc => a.pc.blocker.is_some(),
                    Device::SteamDeck => a.steam_deck.blocker.is_some(),
                }
            }),
        };
        if eligibility(&candidate, &self.context, &resolve(&record.game)).is_err() {
            self.message =
                "This game no longer fits. Edit your preferences or choose another game.".into();
            self.needs_rank = true;
            cx.notify();
            return;
        }
        let active = ActivePlay {
            id: Uuid::new_v4(),
            library_id: manifest.library_id,
            game_id: id,
            title: game.title,
            started_at: now(),
            context: self.context.clone(),
        };
        self.active_play = Some(active.clone());
        self.session_saving = true;
        self.deal_pending = None;
        self.launch_error = false;
        self.message = "Starting your session…".into();
        self.scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        self.start_timer(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::settings::update(|settings| settings.active_play = Some(active))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.session_saving = false;
                match result {
                    Ok(()) => this.launch_active(cx),
                    Err(error) => {
                        this.active_play = None;
                        this.ticker = None;
                        this.message =
                            format!("Could not start the session: {error:#}. Try Play again.");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn launch_active(&mut self, cx: &mut gpui::Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(active) = self.active_play.clone() else {
            return;
        };
        let Some(game) = self.game(active.game_id, cx) else {
            return;
        };
        let local_target =
            self.installations.on_deck == (active.context.device == Device::SteamDeck);
        let app_id = game
            .record
            .as_ref()
            .and_then(|r| r.game.steam.as_ref())
            .map(|s| s.app_id);
        self.launch_error = false;
        if self.library.read(cx).demo
            || !local_target
            || app_id.is_none()
            || self
                .installations
                .for_target(app_id.unwrap_or(0), active.context.device)
                != Some(true)
        {
            self.save(
                active.game_id,
                Change::Choice(active.choice(LaunchOutcome::NotRequested, None)),
                cx,
            );
            return;
        }
        self.launching = true;
        self.message = "Asking Steam to open the game…".into();
        cx.spawn(async move |this, cx| {
            let device = active.context.device;
            let result = cx
                .background_spawn(async move {
                    let app_id = app_id.unwrap_or(0);
                    anyhow::ensure!(
                        local_steam::scan().for_target(app_id, device) == Some(true),
                        "This game is no longer installed on this setup."
                    );
                    local_steam::request_launch(app_id)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.launching = false;
                this.launch_error = result.is_err();
                let outcome = if result.is_ok() {
                    LaunchOutcome::Accepted
                } else {
                    LaunchOutcome::Failed
                };
                this.save(
                    active.game_id,
                    Change::Choice(active.choice(outcome, None)),
                    cx,
                );
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn done_playing(&mut self, cx: &mut gpui::Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(active) = self.active_play.clone() else {
            return;
        };
        let same_library = self
            .library
            .read(cx)
            .source
            .as_ref()
            .is_some_and(|(_, manifest)| manifest.library_id == active.library_id);
        if !same_library || self.game(active.game_id, cx).is_none() {
            // A removed game or disconnected library must not trap the picker.
            self.finish_local(cx);
            return;
        }
        let previous = self
            .game(active.game_id, cx)
            .and_then(|game| game.record)
            .and_then(|record| {
                record
                    .game
                    .personal
                    .play_now
                    .recent
                    .into_iter()
                    .find(|c| c.session == active.id)
            });
        let outcome = previous
            .as_ref()
            .map_or(LaunchOutcome::NotRequested, |c| c.launch);
        let ended = previous.and_then(|c| c.finished_at).unwrap_or_else(now);
        self.save(
            active.game_id,
            Change::Choice(active.choice(outcome, Some(ended))),
            cx,
        );
    }

    pub(super) fn finish_local(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(active) = self.active_play.clone() else {
            return;
        };
        self.session_saving = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::settings::update(|settings| {
                        if settings
                            .active_play
                            .as_ref()
                            .is_some_and(|s| s.id == active.id)
                        {
                            settings.active_play = None;
                        }
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.session_saving = false;
                match result {
                    Ok(()) => {
                        this.active_play = None;
                        this.ticker = None;
                        this.session = Session::default();
                        this.screen = Screen::Setup;
                        this.message.clear();
                        this.restore_focus = true;
                    }
                    Err(error) => {
                        this.message = format!(
                            "Could not finish the session: {error:#}. Try Done playing again."
                        )
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
