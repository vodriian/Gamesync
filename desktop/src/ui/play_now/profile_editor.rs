//! Game profile editor. Controls show the effective value: the user's own
//! value, else the AI suggestion, else a local estimate. Choosing a value
//! makes it the user's; Reset returns the field to the suggestion. Only the
//! user's values are saved, so a later analysis can still update the rest.
use super::*;
use gpui::{div, px, AnyElement, SharedString};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
};

pub(super) struct Draft {
    /// The user's own values only. None follows the suggestion.
    pub values: Profile,
    pub dirty: bool,
    minimum: Entity<InputState>,
    ideal: Entity<InputState>,
    setup: Entity<InputState>,
    /// Last placeholders set on the minute inputs, so render does not reset them.
    placeholders: [String; 3],
    /// Energy and session controls; collapsed because suggestions cover them.
    details_open: bool,
}

/// Where a shown value comes from, as a short tag.
fn source_tag(source: Option<&&str>) -> &'static str {
    match source.copied() {
        Some("Your value") => "Yours",
        Some("AI estimate") => "AI suggestion",
        Some("Local estimate") => "Estimate",
        _ => "Unknown",
    }
}

impl PlayNowView {
    pub(super) fn edit_profile(
        &mut self,
        id: Uuid,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some(game) = self.game(id, cx) else {
            return;
        };
        let values = game
            .record
            .as_ref()
            .unwrap()
            .game
            .personal
            .play_now
            .profile
            .clone();
        let input = |value: Option<u16>, cx: &mut gpui::Context<Self>, window: &mut Window| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value.map_or(String::new(), |v| v.to_string()))
            })
        };
        let minimum = input(values.minimum_minutes, cx, window);
        let ideal = input(values.ideal_minutes, cx, window);
        let setup = input(values.setup_minutes, cx, window);
        for entity in [&minimum, &ideal, &setup] {
            cx.subscribe(entity, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(draft) = &mut this.draft {
                        draft.dirty = true;
                    }
                    cx.notify();
                }
            })
            .detach();
        }
        self.draft = Some(Draft {
            values,
            dirty: false,
            minimum,
            ideal,
            setup,
            placeholders: Default::default(),
            details_open: false,
        });
        self.message.clear();
        self.open_modal(Modal::Profile(id), window, cx);
    }
    fn save_profile(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(Modal::Profile(id)) = self.modal else {
            return;
        };
        let Some(draft) = &self.draft else {
            return;
        };
        let parse = |input: &Entity<InputState>| -> Result<Option<u16>, String> {
            let text = input.read(cx).value();
            let text = text.trim();
            if text.is_empty() {
                Ok(None)
            } else {
                text.parse::<u16>().map(Some).map_err(|_| {
                    "Enter whole minutes, or leave the field blank to use the suggestion.".into()
                })
            }
        };
        let result = (|| {
            let mut values = draft.values.clone();
            values.minimum_minutes = parse(&draft.minimum)?;
            values.ideal_minutes = parse(&draft.ideal)?;
            values.setup_minutes = parse(&draft.setup)?;
            values.validate().map_err(|e| e.to_string())?;
            Ok::<_, String>(values)
        })();
        match result {
            Ok(values) => self.save(id, Change::Profile(values), cx),
            Err(error) => {
                self.message = error;
                cx.notify();
            }
        }
    }

    /// The profile as shown: the draft's own values over the game's AI and
    /// local values. `automatic` ignores the user's values, for placeholders.
    fn draft_profiles(&self, cx: &gpui::App) -> Option<(EffectiveProfile, EffectiveProfile)> {
        let Some(Modal::Profile(id)) = self.modal else {
            return None;
        };
        let draft = self.draft.as_ref()?;
        let mut game = self.game(id, cx)?.record?.game;
        game.personal.play_now.profile = draft.values.clone();
        let effective = resolve(&game);
        game.personal.play_now.profile = Profile::default();
        Some((effective, resolve(&game)))
    }

    /// Minute inputs show the suggestion as a placeholder until the user types.
    /// Called from the modal's render, which has the window this needs.
    pub(super) fn sync_profile_placeholders(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some((_, automatic)) = self.draft_profiles(cx) else {
            return;
        };
        let Some(draft) = &mut self.draft else {
            return;
        };
        let values = [
            ("minimum_minutes", automatic.values.minimum_minutes),
            ("ideal_minutes", automatic.values.ideal_minutes),
            ("setup_minutes", automatic.values.setup_minutes),
        ];
        let inputs = [
            draft.minimum.clone(),
            draft.ideal.clone(),
            draft.setup.clone(),
        ];
        for (index, ((field, value), input)) in values.into_iter().zip(inputs).enumerate() {
            let text = value.map_or_else(
                || "Unknown".to_owned(),
                |v| format!("{v} ({})", source_tag(automatic.sources.get(field))),
            );
            if draft.placeholders[index] != text {
                draft.placeholders[index] = text.clone();
                input.update(cx, |input, cx| input.set_placeholder(text, window, cx));
            }
        }
    }

    fn field_header(
        &self,
        key: &'static str,
        label: &'static str,
        source: &'static str,
        manual: bool,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Div {
        h_flex()
            .gap_2()
            .child(div().text_sm().child(label))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(source),
            )
            .when(manual, |row| {
                row.child(
                    Button::new(SharedString::from(format!("reset-{key}")))
                        .ghost()
                        .xsmall()
                        .label("Reset")
                        .tooltip("Use the suggestion again")
                        .disabled(self.saving)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(draft) = &mut this.draft {
                                let v = &mut draft.values;
                                match key {
                                    "mechanical" => v.mechanical = None,
                                    "cognitive" => v.cognitive = None,
                                    "narrative" => v.narrative = None,
                                    "onboarding" => v.onboarding = None,
                                    "stopping" => v.stopping = None,
                                    _ => v.activities = None,
                                }
                                draft.dirty = true;
                            }
                            cx.notify();
                        })),
                )
            })
    }

    pub(super) fn profile_view(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let Some(draft) = &self.draft else {
            return div().into_any_element();
        };
        let Some((effective, _)) = self.draft_profiles(cx) else {
            return div().into_any_element();
        };
        let shown = effective.values.clone();
        let sources = effective.sources.clone();
        let manual = draft.values.clone();
        let details_open = draft.details_open;
        let minimum = draft.minimum.clone();
        let ideal = draft.ideal.clone();
        let setup = draft.setup.clone();
        let summary = match self.modal {
            Some(Modal::Profile(id)) if !self.library.read(cx).demo => {
                Some(self.analysis_summary(id, cx))
            }
            _ => None,
        };

        let activities_header = self.field_header(
            "activities",
            "Activities",
            source_tag(sources.get("activities")),
            manual.activities.is_some(),
            cx,
        );
        let activities = h_flex()
            .flex_wrap()
            .gap_2()
            .children(Activity::ALL.map(|activity| {
                Button::new(SharedString::from(format!("profile-{activity:?}")))
                    .label(activity.label())
                    .icon(PlayIcon(activity_icon(activity)))
                    .h(px(40.))
                    .px(px(16.))
                    .outline()
                    .disabled(self.saving)
                    .selected(
                        shown
                            .activities
                            .as_ref()
                            .is_some_and(|a| a.contains(&activity)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let suggested = this
                            .draft_profiles(cx)
                            .and_then(|(effective, _)| effective.values.activities);
                        if let Some(draft) = &mut this.draft {
                            // The first choice starts from what is shown, not from nothing.
                            let values = draft
                                .values
                                .activities
                                .get_or_insert_with(|| suggested.unwrap_or_default());
                            if values.contains(&activity) {
                                values.retain(|a| *a != activity);
                            } else {
                                values.push(activity);
                            }
                            draft.dirty = true;
                        }
                        cx.notify();
                    }))
            }));

        let rows = [
            (
                "mechanical",
                "Mechanical effort",
                shown.mechanical,
                manual.mechanical.is_some(),
                0,
            ),
            (
                "cognitive",
                "Thinking",
                shown.cognitive,
                manual.cognitive.is_some(),
                1,
            ),
            (
                "narrative",
                "Story to remember",
                shown.narrative,
                manual.narrative.is_some(),
                2,
            ),
            (
                "onboarding",
                "Onboarding",
                shown.onboarding,
                manual.onboarding.is_some(),
                3,
            ),
        ];
        let effort_rows: Vec<_> = rows
            .into_iter()
            .map(|(key, label, current, is_manual, index)| {
                let header =
                    self.field_header(key, label, source_tag(sources.get(key)), is_manual, cx);
                let controls = Effort::ALL.into_iter().map(|effort| {
                    Button::new(SharedString::from(format!("effort-{index}-{effort:?}")))
                        .label(effort.label())
                        .outline()
                        .selected(current == Some(effort))
                        .disabled(self.saving)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(draft) = &mut this.draft {
                                let v = &mut draft.values;
                                match index {
                                    0 => v.mechanical = Some(effort),
                                    1 => v.cognitive = Some(effort),
                                    2 => v.narrative = Some(effort),
                                    _ => v.onboarding = Some(effort),
                                }
                                draft.dirty = true;
                            }
                            cx.notify();
                        }))
                });
                v_flex()
                    .gap_2()
                    .child(header)
                    .child(h_flex().flex_wrap().gap_2().children(controls))
            })
            .collect();
        let muted = cx.theme().muted_foreground;
        let duration = h_flex().gap_4().children(
            [
                ("Minimum session", &minimum),
                ("Ideal session", &ideal),
                ("Startup", &setup),
            ]
            .into_iter()
            .map(|(label, input)| {
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(div().text_sm().child(format!("{label} (min)")))
                    .child(Input::new(input).disabled(self.saving))
            }),
        );
        let stopping_header = self.field_header(
            "stopping",
            "Stopping point",
            source_tag(sources.get("stopping")),
            manual.stopping.is_some(),
            cx,
        );
        let stopping = h_flex()
            .flex_wrap()
            .gap_2()
            .children(Stopping::ALL.into_iter().map(|stopping| {
                Button::new(SharedString::from(format!("stopping-{stopping:?}")))
                    .label(stopping.label())
                    .outline()
                    .selected(shown.stopping == Some(stopping))
                    .disabled(self.saving)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(draft) = &mut this.draft {
                            draft.values.stopping = Some(stopping);
                            draft.dirty = true;
                        }
                        cx.notify();
                    }))
            }));

        // One line that says what the collapsed controls hold.
        let session = match (shown.minimum_minutes, shown.ideal_minutes) {
            (Some(min), Some(ideal)) if min != ideal => format!("{min}–{ideal} min"),
            (Some(min), _) => format!("{min} min"),
            _ => "session unknown".into(),
        };
        let digest = [
            shown.energy().map_or("Energy unknown".to_owned(), |e| {
                format!("{} energy", e.label())
            }),
            session,
            shown
                .stopping
                .map_or("stopping unknown".to_owned(), |s| s.label().to_lowercase()),
        ]
        .join(" · ");
        let details_toggle = h_flex()
            .gap_2()
            .child(
                Button::new("profile-details")
                    .ghost()
                    .label("Energy and session")
                    .icon(if details_open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(draft) = &mut this.draft {
                            draft.details_open = !draft.details_open;
                        }
                        cx.notify();
                    })),
            )
            .when(!details_open, |row| {
                row.child(div().text_sm().text_color(muted).child(digest))
            });

        v_flex()
            .max_w(px(670.))
            .gap_5()
            .children(summary)
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Suggestions fill in automatically. Choose a value to make it yours."),
            )
            .child(v_flex().gap_2().child(activities_header).child(activities))
            .child(
                v_flex()
                    .gap_4()
                    .child(details_toggle)
                    .when(details_open, |col| {
                        col.children(effort_rows)
                            .child(duration)
                            .child(v_flex().gap_2().child(stopping_header).child(stopping))
                    }),
            )
            .child(self.feedback(cx))
            .into_any_element()
    }
    pub(super) fn profile_actions(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        h_flex()
            .gap_3()
            .child(
                Button::new("save-profile")
                    .primary()
                    .label("Save profile")
                    .disabled(self.saving)
                    .on_click(cx.listener(|this, _, _, cx| this.save_profile(cx))),
            )
            .child(
                Button::new("discard-profile")
                    .label("Discard")
                    .disabled(self.saving)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.draft = None;
                        this.pending_save = None;
                        this.modal = None;
                        this.message.clear();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
