//! Recommendation summary within the shared game card, not a separate page.
use super::*;
use gpui::{div, px, AnyElement, SharedString};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _,
    Sizable as _,
};
impl PlayNowView {
    pub fn game_section(
        &self,
        id: Uuid,
        editor_busy: bool,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(game) = self.game(id, cx) else {
            return div().into_any_element();
        };
        let record = game.record.as_ref().unwrap();
        let profile = resolve(&record.game);
        let personal = &record.game.personal.play_now;
        let disabled = editor_busy || self.busy();
        let summary = self
            .selection
            .ranked
            .iter()
            .find(|pick| pick.id == id)
            .filter(|_| self.library.read(cx).play_now)
            .map(|pick| pick.reason.clone())
            // Outside a hand, the AI's reason describes the game better than a generic line.
            .or_else(|| {
                gamesync_desktop::recommendations::analysis::current_analysis(&record.game)
                    .and_then(|a| a.reason.clone())
            })
            .unwrap_or_else(|| {
                "Choose this game when its energy and session length fit your plans.".to_owned()
            });
        let energy = profile.values.energy();
        let analyzed =
            gamesync_desktop::recommendations::analysis::current_analysis(&record.game).is_some();
        let activities = analyzed
            .then(|| profile.values.activities.clone())
            .flatten()
            .unwrap_or_default();
        let duration = match (profile.values.minimum_minutes, profile.values.ideal_minutes) {
            (Some(min), Some(ideal)) if min != ideal => format!("{min}–{ideal} min"),
            (Some(min), _) => format!("{min} min"),
            _ => "Session unknown".into(),
        };
        let job = self.analysis.read(cx);
        let ai_ready = job.ready(cx);
        let analysis_running = job.running();
        let action = |key, icon: &'static str, label: &'static str| {
            Button::new(key)
                .small()
                .ghost()
                .w(px(32.))
                .h(px(32.))
                .icon(PlayIcon(icon))
                .tooltip(label)
                .disabled(disabled)
        };
        v_flex()
            .gap_2()
            .py_3()
            .border_t_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Play now"),
                    )
                    .when(ai_ready, |row| {
                        let analysis = self.analysis.clone();
                        row.child(
                            action(
                                "profile-analyze",
                                "ai-beautify",
                                if analyzed {
                                    "Refresh AI suggestion"
                                } else {
                                    "Get AI suggestion"
                                },
                            )
                            .loading(analysis_running)
                            .disabled(disabled || analysis_running)
                            .on_click(move |_, _, cx| {
                                analysis.update(cx, |job, cx| job.analyze_one(id, cx))
                            }),
                        )
                    })
                    .child(
                        action("profile-edit", "customize", "Edit game profile").on_click(
                            cx.listener(move |this, _, w, cx| this.edit_profile(id, w, cx)),
                        ),
                    )
                    .child(
                        action(
                            "profile-save",
                            "clock-fading",
                            if personal.saved {
                                "Remove saved pick"
                            } else {
                                "Save for later"
                            },
                        )
                        .selected(personal.saved)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(game) = this.game(id, cx) {
                                let saved = game
                                    .record
                                    .as_ref()
                                    .is_some_and(|r| r.game.personal.play_now.saved);
                                this.save(id, Change::Saved(!saved), cx);
                            }
                        })),
                    )
                    .child(
                        Button::new("profile-exclude")
                            .small()
                            .ghost()
                            .w(px(32.))
                            .h(px(32.))
                            .icon(IconName::EyeOff)
                            .selected(personal.excluded)
                            .disabled(disabled)
                            .tooltip(if personal.excluded {
                                "Allow in Play now"
                            } else {
                                "Exclude from Play now"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(game) = this.game(id, cx) {
                                    let excluded = game
                                        .record
                                        .as_ref()
                                        .is_some_and(|r| r.game.personal.play_now.excluded);
                                    this.save(id, Change::Excluded(!excluded), cx);
                                }
                            })),
                    ),
            )
            .child(div().text_sm().child(if personal.excluded {
                "Excluded from recommendations.".to_owned()
            } else {
                summary
            }))
            .child(
                // Facts on the left, activity icons at the right end. Items do
                // not shrink: a shrunk item let its text overflow into the gap.
                h_flex()
                    .gap_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .flex_wrap()
                            .gap_x_3()
                            .gap_y_1()
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .gap_1()
                                    .child(
                                        Icon::new(PlayIcon(
                                            energy.map_or("battery-medium-01", energy_icon),
                                        ))
                                        .size(px(16.)),
                                    )
                                    .child(energy.map_or("Energy unknown".into(), |level| {
                                        format!("{} energy", level.label())
                                    })),
                            )
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .gap_1()
                                    .child(
                                        Icon::new(crate::assets::SidebarIcon::Playtime)
                                            .size(px(16.)),
                                    )
                                    .child(duration),
                            ),
                    )
                    .when(!activities.is_empty(), |row| {
                        row.child(h_flex().flex_shrink_0().gap_2().children(
                            activities.into_iter().map(|activity| {
                                div()
                                    .id(SharedString::from(format!("section-{activity:?}")))
                                    .child(
                                        Icon::new(PlayIcon(activity_icon(activity))).size(px(16.)),
                                    )
                                    .tooltip(move |window, cx| {
                                        gpui_component::tooltip::Tooltip::new(activity.label())
                                            .build(window, cx)
                                    })
                            }),
                        ))
                    }),
            )
            .when(self.feedback_game == Some(id), |col| {
                col.child(self.feedback(cx))
            })
            .into_any_element()
    }
}
