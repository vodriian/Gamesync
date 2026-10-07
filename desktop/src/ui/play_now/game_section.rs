//! Recommendation summary within the shared game card, not a separate page.
use super::*;
use gpui::{div, px, AnyElement};
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
            .map_or_else(
                || "Choose this game when its energy and session length fit your plans.".to_owned(),
                |pick| pick.reason.clone(),
            );
        let energy = profile.values.energy();
        let duration = match (profile.values.minimum_minutes, profile.values.ideal_minutes) {
            (Some(min), Some(ideal)) if min != ideal => format!("{min}–{ideal} min"),
            (Some(min), _) => format!("{min} min"),
            _ => "Session unknown".into(),
        };
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
                h_flex()
                    .flex_wrap()
                    .gap_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        h_flex()
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
                            .gap_1()
                            .child(Icon::new(crate::assets::SidebarIcon::Playtime).size(px(16.)))
                            .child(duration),
                    )
                    .when(profile.estimated(), |row| row.child("Estimated")),
            )
            .when(self.feedback_game == Some(id), |col| {
                col.child(self.feedback(cx))
            })
            .into_any_element()
    }
}
