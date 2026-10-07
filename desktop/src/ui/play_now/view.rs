use super::*;
use gpui::{div, image_cache, img, px, relative, AnyElement, ImageSource, ObjectFit, SharedString};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _,
    Sizable as _, StyledExt as _,
};

fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Button {
    Button::new(id.into())
        .label(label)
        .small()
        .h(px(40.))
        .px(px(16.))
        .rounded(px(7.))
}
fn note(text: impl Into<SharedString>, cx: &gpui::App) -> gpui::Div {
    div()
        .text_size(px(12.))
        .line_height(relative(1.5))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
fn heading(number: &str, title: &str, cx: &gpui::App) -> gpui::Div {
    h_flex()
        .gap_3()
        .mb_3()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(number.to_owned()),
        )
        .child(div().text_sm().font_semibold().child(title.to_owned()))
}
impl PlayNowView {
    pub fn toolbar(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let saved = self
            .library
            .read(cx)
            .games
            .iter()
            .filter(|g| {
                Self::visible_game(g)
                    && g.record
                        .as_ref()
                        .is_some_and(|r| r.game.personal.play_now.saved)
            })
            .count();
        h_flex()
            .gap_2()
            .when(self.library.read(cx).demo, |row| {
                row.child(note("Demo library", cx))
            })
            .child(
                Button::new("play-saved")
                    .ghost()
                    .label(format!("Saved {saved}"))
                    .icon(PlayIcon("bookmark-02"))
                    .disabled(self.busy() || self.active_play.is_some())
                    .on_click(cx.listener(|this, _, w, cx| this.go(Screen::Saved, w, cx))),
            )
            .child(
                Button::new("play-recent")
                    .ghost()
                    .label("Recent")
                    .icon(PlayIcon("transaction-history"))
                    .disabled(self.busy() || self.active_play.is_some())
                    .on_click(cx.listener(|this, _, w, cx| this.go(Screen::Recent, w, cx))),
            )
            .into_any_element()
    }
    pub(super) fn setup(&self, modal: bool, cx: &mut gpui::Context<Self>) -> AnyElement {
        let form = v_flex()
            .w_full()
            .max_w(px(530.))
            .gap(px(20.))
            .child(
                v_flex()
                    .child(heading("01", "How much time do you have?", cx))
                    .child(
                        self.selection_group("time", cx)
                            .p_1()
                            .gap_1()
                            .rounded(px(8.))
                            .bg(cx.theme().secondary)
                            .children(controls::TIMES.into_iter().map(|minutes| {
                                let label = rec::Context {
                                    minutes,
                                    ..Default::default()
                                }
                                .time_label();
                                let control = button(
                                    format!("time-{minutes:?}"),
                                    if minutes.is_none() {
                                        String::new()
                                    } else {
                                        label.clone()
                                    },
                                )
                                .px(px(8.))
                                .tooltip(label);
                                self.choice(
                                    "time",
                                    format!("time-{minutes:?}"),
                                    true,
                                    control
                                        .when(minutes.is_none(), |b| {
                                            b.icon(PlayIcon("infinity-01"))
                                        })
                                        .on_click(cx.listener(move |this, event, _, cx| {
                                            this.context.minutes = minutes;
                                            this.change_context(Some(event), cx);
                                        })),
                                    cx,
                                )
                            })),
                    ),
            )
            .child(
                v_flex()
                    .child(heading("02", "How much energy do you have?", cx))
                    .child(
                        self.selection_group("energy", cx)
                            .p_1()
                            .gap_1()
                            .rounded(px(8.))
                            .bg(cx.theme().secondary)
                            .children(Effort::ALL.map(|energy| {
                                self.choice(
                                    "energy",
                                    format!("energy-{energy:?}"),
                                    true,
                                    button(format!("energy-{energy:?}"), energy.label())
                                        .icon(PlayIcon(energy_icon(energy)))
                                        .on_click(cx.listener(move |this, event, _, cx| {
                                            this.context.energy = energy;
                                            this.change_context(Some(event), cx);
                                        })),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        div().mt_3().child(
                            h_flex()
                                .p_4()
                                .gap_4()
                                .rounded(px(9.))
                                .bg(cx.theme().secondary)
                                .child(Icon::new(PlayIcon("brain-02")).size(px(44.)))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .gap_1()
                                        .child(div().font_semibold().text_sm().child("Brain dead"))
                                        .child(note("Less thinking, less remembering.", cx)),
                                )
                                .child(self.brain_switch(cx)),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .child(heading("03", "What do you feel like doing?", cx))
                    .child(
                        self.selection_group("activity", cx)
                            .flex_wrap()
                            .gap(px(7.))
                            .child(self.choice(
                                "activity",
                                "activity-any".into(),
                                false,
                                button("activity-any", "Anything").on_click(cx.listener(
                                    |this, event, _, cx| {
                                        this.context.activity = None;
                                        this.change_context(Some(event), cx);
                                    },
                                )),
                                cx,
                            ))
                            .children(Activity::ALL.map(|activity| {
                                self.choice(
                                    "activity",
                                    format!("activity-{activity:?}"),
                                    false,
                                    button(format!("activity-{activity:?}"), activity.label())
                                        .icon(PlayIcon(activity_icon(activity)))
                                        .on_click(cx.listener(move |this, event, _, cx| {
                                            this.context.activity = Some(activity);
                                            this.change_context(Some(event), cx);
                                        })),
                                    cx,
                                )
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        Button::new("play-options")
                            .ghost()
                            .label("More options")
                            .icon(if self.options {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.options = !this.options;
                                cx.notify();
                            })),
                    )
                    .when(self.options, |column| column.child(self.options_view(cx))),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        button("deal", "Deal me 3")
                            .primary()
                            .w_full()
                            .icon(PlayIcon("cards-02"))
                            .disabled(self.saving)
                            .on_click(cx.listener(|this, _, w, cx| this.deal(false, w, cx))),
                    )
                    .child(note("Up to three choices. One optional reshuffle.", cx)),
            );
        v_flex()
            .gap(px(24.))
            .when(!modal, |col| {
                col.child(
                    div()
                        .text_3xl()
                        .font_bold()
                        .child("Let's choose a game for you"),
                )
            })
            .child(form)
            .into_any_element()
    }
    fn options_view(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let devices = self
            .selection_group("device", cx)
            .gap_2()
            .child(note("Playing on", cx))
            .children([Device::Pc, Device::SteamDeck].map(|device| {
                self.choice(
                    "device",
                    format!("device-{device:?}"),
                    false,
                    button(format!("device-{device:?}"), device.label()).on_click(cx.listener(
                        move |this, event, _, cx| {
                            this.context.device = device;
                            this.change_context(Some(event), cx);
                        },
                    )),
                    cx,
                )
            }));
        let scopes = self
            .selection_group("scope", cx)
            .gap_2()
            .flex_wrap()
            .children([Scope::Any, Scope::Unplayed, Scope::Playing].map(|scope| {
                self.choice(
                    "scope",
                    format!("scope-{scope:?}"),
                    false,
                    button(format!("scope-{scope:?}"), scope.label()).on_click(cx.listener(
                        move |this, event, _, cx| {
                            this.context.scope = scope;
                            this.change_context(Some(event), cx);
                        },
                    )),
                    cx,
                )
            }));
        v_flex().gap_4().child(devices).child(scopes)
            .child(Checkbox::new("installed-only").label("Installed on this setup only")
                .checked(self.context.installed_only)
                .on_click(cx.listener(|this, checked, _, cx| {
                    this.context.installed_only = *checked;
                    this.change_context(None, cx);
                })))
            .child(Checkbox::new("favorites-only").label("Favorites only")
                .checked(self.context.favorites_only)
                .on_click(cx.listener(|this, checked, _, cx| {
                    this.context.favorites_only = *checked;
                    this.change_context(None, cx);
                })))
            .child(note(if self.library.read(cx).demo {
                "Installation is unknown in this demo."
            } else {
                "Installation checks apply to this computer. Remote setup installs stay unknown."
            }, cx)).into_any_element()
    }
    fn context_strip(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let reshuffle = self.session.reshuffles > 0
            && self.selection.ranked.iter().any(|p| {
                !self.session.shown.contains(&p.id) && !self.session.dismissed.contains(&p.id)
            });
        let values = [
            self.context.time_label(),
            format!("{} energy", self.context.energy.label()),
            self.context
                .activity
                .map_or("Anything", Activity::label)
                .into(),
            self.context.device.label().into(),
        ];
        h_flex()
            .gap_3()
            .pb_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .flex_1()
                    .flex_wrap()
                    .gap_2()
                    .children(values.into_iter().map(|value| {
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .rounded(px(5.))
                            .bg(cx.theme().secondary)
                            .child(value)
                    }))
                    .child(
                        Button::new("edit-context")
                            .ghost()
                            .icon(PlayIcon("customize"))
                            .tooltip("Edit preferences")
                            .h(px(40.))
                            .w(px(40.))
                            .disabled(self.busy() || self.active_play.is_some())
                            .on_click(cx.listener(|this, _, w, cx| this.edit_context(w, cx))),
                    )
                    .when(self.context.brain_dead, |row| {
                        row.child(note("Brain dead", cx))
                    })
                    .when(self.context.favorites_only, |row| {
                        row.child(note("Favorites", cx))
                    })
                    .when(self.context.installed_only, |row| {
                        row.child(note("Installed", cx))
                    })
                    .when(self.context.scope != Scope::Any, |row| {
                        row.child(note(self.context.scope.label(), cx))
                    }),
            )
            .child(
                button("reshuffle", "Reshuffle")
                    .outline()
                    .icon(PlayIcon("refresh-04"))
                    .tooltip(if self.session.reshuffles == 0 {
                        "Reshuffle used"
                    } else {
                        "Deal a different hand"
                    })
                    .disabled(!reshuffle || self.ranking || self.busy())
                    .on_click(cx.listener(|this, _, w, cx| this.deal(true, w, cx))),
            )
            .into_any_element()
    }
    fn cover(&self, game: &Game, width: f32, height: f32, cx: &gpui::App) -> AnyElement {
        let source = game
            .cover_path
            .clone()
            .map(ImageSource::from)
            .unwrap_or_else(|| game.cover.clone().into());
        let fallback_title = game.title.clone();
        div()
            .w(px(width))
            .h(px(height))
            .p(px(5.))
            .rounded(crate::theme::interface_radius(cx, px(11.)))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .shadow_md()
            .child(
                div()
                    .size_full()
                    .rounded(crate::theme::interface_radius(cx, px(6.)))
                    .overflow_hidden()
                    .child(
                        img(source)
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .with_fallback(move || {
                                div()
                                    .size_full()
                                    .p_4()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(fallback_title.clone())
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }
    fn card(
        &self,
        game: Game,
        index: usize,
        width: f32,
        pick: &Pick,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let id = game.id;
        let personal = &game.record.as_ref().unwrap().game.personal.play_now;
        let dismissed = self.session.dismissed.contains(&id);
        let role = if index == 0 {
            "Closest fit"
        } else {
            "Another good fit"
        };
        let p = &pick.profile.values;
        let duration = match (p.minimum_minutes, p.ideal_minutes) {
            (Some(min), Some(ideal)) => format!("{min}–{ideal} min"),
            (Some(min), _) => format!("{min}+ min"),
            _ => "Session unknown".into(),
        };
        let installation = self.installed(&game);
        v_flex()
            .w(px(width))
            .flex_shrink_0()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .px_1()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(px(3.))
                            .child(format!("{:02}", index + 1)),
                    )
                    .child(note(if dismissed { "Set aside" } else { role }, cx)),
            )
            .map(|column| {
                if dismissed {
                    column.child(
                        v_flex()
                            .h(px(width * 1.32))
                            .p_5()
                            .gap_4()
                            .justify_center()
                            .rounded(px(10.))
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(div().font_semibold().child(game.title.clone()))
                            .child(note("Not for this session.", cx))
                            .child(button(format!("undo-{id}"), "Undo").on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.session.dismissed.remove(&id);
                                    cx.notify();
                                },
                            ))),
                    )
                } else {
                    column
                        .child(
                            Button::new(SharedString::from(format!("cover-{id}")))
                                .ghost()
                                .p_0()
                                .w(px(width))
                                .h(px(width * 1.32))
                                .child(self.cover(&game, width, width * 1.32, cx))
                                .tooltip(format!("Details for {}", game.title))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.open_game(id, cx)),
                                ),
                        )
                        .child(div().font_semibold().text_lg().child(game.title.clone()))
                        .child(note(
                            format!(
                                "{duration} · {}",
                                match installation {
                                    Some(true) => "Installed",
                                    Some(false) => "Not installed",
                                    None => "Install unknown",
                                }
                            ),
                            cx,
                        ))
                        .when(pick.profile.estimated(), |col| {
                            col.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Estimated"),
                            )
                        })
                        .child(div().min_h(px(52.)).child(note(pick.reason.clone(), cx)))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    button(format!("choose-{id}"), "Play")
                                        .primary()
                                        .flex_1()
                                        .disabled(self.saving || self.launching)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.start_play(id, cx)
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("skip-{id}")))
                                        .outline()
                                        .h(px(40.))
                                        .w(px(40.))
                                        .icon(PlayIcon("fast-forward"))
                                        .tooltip("Skip")
                                        .disabled(self.busy() || self.active_play.is_some())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.session.dismissed.insert(id);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("save-{id}")))
                                        .outline()
                                        .h(px(40.))
                                        .w(px(40.))
                                        .icon(PlayIcon("clock-fading"))
                                        .selected(personal.saved)
                                        .disabled(self.saving)
                                        .tooltip(if personal.saved {
                                            "Remove saved pick"
                                        } else {
                                            "Save for later"
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Some(game) = this.game(id, cx) {
                                                let saved = game.record.as_ref().is_some_and(|r| {
                                                    r.game.personal.play_now.saved
                                                });
                                                this.save(id, Change::Saved(!saved), cx);
                                            }
                                        })),
                                ),
                        )
                }
            })
            .into_any_element()
    }
    fn hand(&self, width: f32, cx: &mut gpui::Context<Self>) -> AnyElement {
        let cards: Vec<_> = self
            .session
            .hand
            .iter()
            .filter_map(|id| {
                self.game(*id, cx)
                    .filter(|g| {
                        !g.record
                            .as_ref()
                            .is_some_and(|r| r.game.personal.play_now.excluded)
                    })
                    .and_then(|g| {
                        self.selection
                            .ranked
                            .iter()
                            .find(|p| p.id == *id)
                            .map(|p| (g, p))
                    })
            })
            .collect();
        if cards.is_empty() {
            return self.empty(cx);
        }
        v_flex()
            .gap_6()
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_3xl()
                            .font_bold()
                            .child("Your hand for right now."),
                    )
                    .child(note(
                        format!(
                            "{} {} to choose from. Start with the one that catches your eye.",
                            cards.len(),
                            if cards.len() == 1 { "game" } else { "games" }
                        ),
                        cx,
                    )),
            )
            .child(self.context_strip(cx))
            .child(
                h_flex()
                    .justify_center()
                    .items_start()
                    .gap(px(24.))
                    .children(
                        cards
                            .into_iter()
                            .enumerate()
                            .map(|(i, (game, pick))| self.card(game, i, width, pick, cx)),
                    ),
            )
            .into_any_element()
    }
    fn empty(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let empty = self.library.read(cx).games.is_empty();
        v_flex()
            .gap_6()
            .child(self.context_strip(cx))
            .child(
                v_flex()
                    .py(px(50.))
                    .gap_5()
                    .items_center()
                    .child(Icon::new(PlayIcon("cards-02")).size(px(44.)))
                    .child(div().text_2xl().font_semibold().child(if empty {
                        "Your library is empty."
                    } else {
                        "No games fit this hand yet."
                    }))
                    .child(note(
                        if empty {
                            "Add your games with Steam sync to get started."
                        } else {
                            "Adjust your preferences or review incomplete game profiles."
                        },
                        cx,
                    ))
                    .child(
                        h_flex().gap_2().flex_wrap().children(
                            self.selection
                                .rejected
                                .iter()
                                .filter(|(r, _)| {
                                    !matches!(r, Rejection::Hidden | Rejection::Unavailable)
                                })
                                .map(|(r, count)| {
                                    div()
                                        .px_3()
                                        .py_2()
                                        .rounded(px(6.))
                                        .bg(cx.theme().secondary)
                                        .text_sm()
                                        .child(format!("{}: {count}", r.label()))
                                }),
                        ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .flex_wrap()
                            .child(
                                button("adjust-context", "Edit preferences")
                                    .primary()
                                    .on_click(
                                        cx.listener(|this, _, w, cx| this.go(Screen::Setup, w, cx)),
                                    ),
                            )
                            .when(self.context.activity.is_some(), |row| {
                                row.child(button("relax-activity", "Try any activity").on_click(
                                    cx.listener(|this, _, w, cx| {
                                        this.context.activity = None;
                                        this.change_context(None, cx);
                                        this.deal(false, w, cx);
                                    }),
                                ))
                            })
                            .when(!self.selection.incomplete.is_empty(), |row| {
                                row.child(button("review-profiles", "Review profiles").on_click(
                                    cx.listener(|this, _, w, cx| this.go(Screen::Profiles, w, cx)),
                                ))
                            }),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn feedback(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        div()
            .when(!self.message.is_empty(), |col| {
                col.child(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded(px(7.))
                        .bg(cx.theme().secondary)
                        .child(note(self.message.clone(), cx))
                        .when(self.pending_save.is_some() && !self.saving, |col| {
                            col.child(button("retry-save", "Retry save").on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let Some((id, change)) = this.pending_save.clone() {
                                        this.save(id, change, cx);
                                    }
                                },
                            )))
                            .child(
                                button("discard-save", "Discard change").ghost().on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.pending_save = None;
                                        this.message.clear();
                                        cx.notify();
                                    }),
                                ),
                            )
                        })
                        .when(self.undo_exclusion.is_some(), |col| {
                            col.child(
                                button("undo-exclusion", "Undo")
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(id) = this.undo_exclusion {
                                            this.save(id, Change::Excluded(false), cx);
                                        }
                                    })),
                            )
                        }),
                )
            })
            .into_any_element()
    }
    fn list(&self, screen: Screen, cx: &mut gpui::Context<Self>) -> AnyElement {
        let mut games: Vec<_> = self
            .library
            .read(cx)
            .games
            .iter()
            .filter(|g| Self::visible_game(g))
            .filter(|g| {
                let p = &g.record.as_ref().unwrap().game.personal.play_now;
                match screen {
                    Screen::Saved => p.saved || p.excluded,
                    Screen::Recent => !p.recent.is_empty(),
                    _ => self.selection.incomplete.contains(&g.id),
                }
            })
            .cloned()
            .collect();
        if screen == Screen::Recent {
            games.sort_by_key(|g| {
                std::cmp::Reverse(
                    g.record
                        .as_ref()
                        .and_then(|r| r.game.personal.play_now.recent.first().map(|c| c.at))
                        .unwrap_or(0),
                )
            });
        }
        let total = games.len();
        v_flex()
            .gap_4()
            .child(div().text_2xl().font_semibold().child(match screen {
                Screen::Saved => "Saved and excluded",
                Screen::Recent => "Recent",
                _ => "Profiles to review",
            }))
            .when(games.is_empty(), |col| {
                col.child(note("No games here yet.", cx))
            })
            .children(games.into_iter().take(self.list_limit).map(|g| {
                let id = g.id;
                let p = &g.record.as_ref().unwrap().game.personal.play_now;
                let label = if p.excluded {
                    "Not interested".to_owned()
                } else if screen == Screen::Recent {
                    if let Some((choice, end)) = p
                        .recent
                        .first()
                        .and_then(|choice| choice.finished_at.map(|end| (choice, end)))
                    {
                        format!(
                            "Session finished · {} min",
                            ((end - choice.at).max(0) / 60).max(1)
                        )
                    } else {
                        match p.recent.first().map(|choice| choice.launch) {
                            Some(LaunchOutcome::Accepted) => "Launch requested · play not verified",
                            Some(LaunchOutcome::Failed) => "Launch failed · retry from the game",
                            _ => "Chosen · no launch requested",
                        }
                        .to_owned()
                    }
                } else if p.saved {
                    "Saved for later".to_owned()
                } else {
                    "Incomplete profile".to_owned()
                };
                h_flex()
                    .py_3()
                    .gap_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .flex_1()
                            .child(div().font_semibold().child(g.title.clone()))
                            .child(note(label, cx)),
                    )
                    .child(
                        button(format!("list-{id}"), "Review")
                            .outline()
                            .on_click(cx.listener(move |this, _, _, cx| this.open_game(id, cx))),
                    )
            }))
            .when(total > self.list_limit, |col| {
                col.child(
                    button("more-picks", "Show more")
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.list_limit += 60;
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
    fn playing(&self, window: &Window, cx: &mut gpui::Context<Self>) -> AnyElement {
        let active = self.active_play.as_ref().unwrap();
        let width = ((f32::from(window.viewport_size().height) - 400.) / 1.32).clamp(115., 250.);
        let game = self.game(active.game_id, cx);
        v_flex()
            .w_full()
            .items_center()
            .gap_5()
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .rounded_full()
                    .bg(cx.theme().secondary)
                    .child(Icon::new(PlayIcon("hourglass")).size(px(16.)))
                    .child(div().text_sm().font_medium().child("Now playing")),
            )
            .child(game.as_ref().map_or_else(
                || {
                    div()
                        .w(px(width))
                        .h(px(width * 1.32))
                        .rounded(px(12.))
                        .bg(cx.theme().secondary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(PlayIcon("cards-02")).size(px(44.)))
                        .into_any_element()
                },
                |game| self.cover(game, width, width * 1.32, cx),
            ))
            .child(
                v_flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_3xl().font_semibold().child(active.title.clone()))
                    .child(
                        div()
                            .text_size(px(32.))
                            .font_family("monospace")
                            .child(active.timer(now())),
                    )
                    .child(note(
                        if self.library.read(cx).demo {
                            "Demo session · no game launched".to_owned()
                        } else {
                            format!("Session on {}", active.context.device.label())
                        },
                        cx,
                    )),
            )
            .when(self.launch_error, |col| {
                col.child(
                    button("retry-launch", "Retry launch")
                        .outline()
                        .disabled(self.busy())
                        .on_click(cx.listener(|this, _, _, cx| this.launch_active(cx))),
                )
            })
            .child(
                button("done-playing", "Done playing")
                    .primary()
                    .icon(IconName::Check)
                    .w(px(300.))
                    .h(px(48.))
                    .disabled(self.busy())
                    .on_click(cx.listener(|this, _, _, cx| this.done_playing(cx))),
            )
            .into_any_element()
    }
}
impl Render for PlayNowView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        if self.restore_focus {
            self.restore_focus = false;
            window.focus(&self.focus);
        }
        if self.needs_rank && !self.ranking && self.active_play.is_none() {
            self.refresh_rank(cx);
        }
        let width = (f32::from(window.viewport_size().width) - 255. - 96. - 48.)
            .div_euclid(3.)
            .min((f32::from(window.viewport_size().height) - 540.) / 1.32)
            .clamp(160., 260.);
        let body = if self.active_play.is_some() {
            self.playing(window, cx)
        } else if self.deal_pending.is_some() {
            v_flex()
                .gap_4()
                .child(note("Finding a small hand…", cx))
                .child(
                    button("cancel-deal", "Cancel").on_click(cx.listener(|this, _, w, cx| {
                        this.deal_pending = None;
                        this.go(Screen::Setup, w, cx);
                    })),
                )
                .into_any_element()
        } else {
            match self.screen {
                Screen::Setup => self.setup(false, cx),
                Screen::Hand => self.hand(width, cx),
                other => self.list(other, cx),
            }
        };
        div()
            .id("play-now-page")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" && !this.saving {
                    this.back(w, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                image_cache(self.cache.clone()).child(
                    v_flex()
                        .pt(px(92.))
                        .px(px(42.))
                        .pb(px(40.))
                        .gap_5()
                        .when(
                            self.active_play.is_none()
                                && !matches!(self.screen, Screen::Setup | Screen::Hand),
                            |col| {
                                col.child(
                                    Button::new("play-back")
                                        .w(px(88.))
                                        .ghost()
                                        .icon(IconName::ArrowLeft)
                                        .label("Back")
                                        .disabled(self.saving)
                                        .on_click(cx.listener(|this, _, w, cx| this.back(w, cx))),
                                )
                            },
                        )
                        .child(body)
                        .child(self.feedback(cx)),
                ),
            )
    }
}
