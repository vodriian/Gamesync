use super::*;
use gpui::{div, px, AnyElement, SharedString};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, ActiveTheme as _, Disableable as _, Selectable as _,
};

pub(super) struct Draft {
    pub values: Profile,
    pub dirty: bool,
    minimum: Entity<InputState>,
    ideal: Entity<InputState>,
    setup: Entity<InputState>,
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
                    .placeholder("Automatic")
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
                    "Enter whole minutes, or leave the field blank for Automatic.".into()
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
    pub(super) fn profile_view(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let Some(draft) = &self.draft else {
            return div().into_any_element();
        };
        let rows = [
            ("Mechanical effort", draft.values.mechanical, 0),
            ("Thinking", draft.values.cognitive, 1),
            ("Story to remember", draft.values.narrative, 2),
            ("Onboarding", draft.values.onboarding, 3),
        ];
        let effort_rows = rows.into_iter().map(|(label, current, index)| {
            let controls = [
                None,
                Some(Effort::Low),
                Some(Effort::Medium),
                Some(Effort::High),
            ]
            .into_iter()
            .map(|effort| {
                Button::new(SharedString::from(format!("effort-{index}-{effort:?}")))
                    .label(effort.map_or("Automatic", Effort::label))
                    .outline()
                    .selected(current == effort)
                    .disabled(self.saving)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(draft) = &mut this.draft {
                            match index {
                                0 => draft.values.mechanical = effort,
                                1 => draft.values.cognitive = effort,
                                2 => draft.values.narrative = effort,
                                _ => draft.values.onboarding = effort,
                            }
                            draft.dirty = true;
                        }
                        cx.notify();
                    }))
            });
            v_flex()
                .gap_2()
                .child(div().text_sm().child(label))
                .child(h_flex().flex_wrap().gap_2().children(controls))
        });
        let duration = h_flex().gap_4().children(
            [
                ("Minimum session", &draft.minimum),
                ("Ideal session", &draft.ideal),
                ("Startup", &draft.setup),
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
        let stopping = h_flex().flex_wrap().gap_2().children(
            [
                None,
                Some(Stopping::Flexible),
                Some(Stopping::Checkpoints),
                Some(Stopping::LongSession),
            ]
            .into_iter()
            .map(|stopping| {
                Button::new(SharedString::from(format!("stopping-{stopping:?}")))
                    .label(stopping.map_or("Automatic", Stopping::label))
                    .outline()
                    .selected(draft.values.stopping == stopping)
                    .disabled(self.saving)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(draft) = &mut this.draft {
                            draft.values.stopping = stopping;
                            draft.dirty = true;
                        }
                        cx.notify();
                    }))
            }),
        );
        let automatic = Button::new("activities-auto")
            .label("Automatic")
            .outline()
            .selected(draft.values.activities.is_none())
            .disabled(self.saving)
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(draft) = &mut this.draft {
                    draft.values.activities = None;
                    draft.dirty = true;
                }
                cx.notify();
            }));
        let activities = h_flex()
            .flex_wrap()
            .gap_2()
            .child(automatic)
            .children(Activity::ALL.map(|activity| {
                Button::new(SharedString::from(format!("profile-{activity:?}")))
                    .label(activity.label())
                    .icon(PlayIcon(activity_icon(activity)))
                    .h(px(40.))
                    .px(px(16.))
                    .outline()
                    .disabled(self.saving)
                    .selected(
                        draft
                            .values
                            .activities
                            .as_ref()
                            .is_some_and(|a| a.contains(&activity)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(draft) = &mut this.draft {
                            let values = draft.values.activities.get_or_insert_with(Vec::new);
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
        v_flex().max_w(px(670.)).gap_5()
            .child(div().text_sm().text_color(cx.theme().muted_foreground)
                .child("Your corrections take precedence. Automatic uses cached analysis or local estimates, where available."))
            .children(effort_rows).child(duration)
            .child(v_flex().gap_2().child("Stopping point").child(stopping))
            .child(v_flex().gap_2().child("Activities (choose all that apply)").child(activities))
            .child(self.feedback(cx)).into_any_element()
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
