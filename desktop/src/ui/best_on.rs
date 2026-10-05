//! Best on assessment on the open book's right page. The result and confidence
//! always show; fits, reasons, blockers, and the preference start collapsed.
//! Real games use Steam evidence; the Best on demo uses saved fixture values.
//! The detail panel saves a preference through the editor.

use crate::{assets::SetupIcon, model::Library};
use gamesync_desktop::suitability::{Assessment, BestOn, SetupFit, SetupPreference};
use gpui::{div, prelude::*, px, relative, App, Entity, EventEmitter, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _, StyledExt as _,
};
use uuid::Uuid;

pub(super) struct BestOnPanel {
    library: Entity<Library>,
    pub game_id: Uuid,
    expanded: bool,
}

/// The user chose a setup. None returns to Automatic.
pub(super) struct ChoosePreference(pub Option<SetupPreference>);
impl EventEmitter<ChoosePreference> for BestOnPanel {}

impl BestOnPanel {
    pub fn new(library: Entity<Library>, game_id: Uuid, cx: &mut Context<Self>) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        Self {
            library,
            game_id,
            expanded: false,
        }
    }
}

impl Render for BestOnPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let record = self
            .library
            .read(cx)
            .games
            .iter()
            .find(|game| game.id == self.game_id)
            .and_then(|game| game.record.clone());
        let Some(record) = record else {
            return div().into_any_element();
        };
        let Some(assessment) = gamesync_desktop::suitability::assessment(&record.game) else {
            return div().into_any_element();
        };
        let demo = record.game.suitability.is_some();
        // Where the values come from, and how old Steam's answer is.
        let source = if demo {
            "Demo".to_owned()
        } else {
            record
                .game
                .steam
                .as_ref()
                .and_then(|steam| steam.metadata.as_ref()?.setup.as_ref())
                .map_or("Steam".into(), |evidence| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |elapsed| elapsed.as_secs() as i64);
                    match (now - evidence.checked_at) / 86_400 {
                        days if days < 1 => "Steam · checked today".into(),
                        1 => "Steam · checked yesterday".into(),
                        days => format!("Steam · checked {days} days ago"),
                    }
                })
        };
        let preference = record.game.personal.setup_preference;
        let recommendation = assessment.recommendation();
        let muted = cx.theme().muted_foreground;
        let confidence = assessment
            .confidence
            .filter(|value| *value <= 100)
            .map_or("Unknown".into(), |value| format!("{value}/100"));

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(div().text_xs().text_color(muted).child("Best on"))
                    .child(div().text_xs().text_color(muted).child(source)),
            )
            .child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(recommendation.label()),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(div().text_xs().text_color(muted).child("Confidence"))
                            .child(div().text_sm().font_semibold().child(confidence)),
                    ),
            )
            .child(
                h_flex().child(
                    Button::new("best-on-details")
                        .ghost()
                        .xsmall()
                        .label(if self.expanded {
                            "Fewer details"
                        } else {
                            "More details"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.expanded = !this.expanded;
                            cx.notify();
                        })),
                ),
            )
            .when(self.expanded, |panel| {
                panel.child(details(&assessment, preference, demo, cx))
            })
            .into_any_element()
    }
}

/// Fit for each setup, then the personal preference.
fn details(
    assessment: &Assessment,
    preference: Option<SetupPreference>,
    demo: bool,
    cx: &mut Context<BestOnPanel>,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let recommendation = assessment.recommendation();
    let choices = [
        (None, "Automatic"),
        (Some(SetupPreference::SteamDeck), "Steam Deck"),
        (Some(SetupPreference::Pc), "PC"),
        (Some(SetupPreference::Both), "Both"),
    ];
    v_flex()
        .gap_3()
        .when(recommendation == BestOn::NeedsReview, |panel| {
            panel.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("There is not enough evidence for a recommendation."),
            )
        })
        .child(fit_row(
            SetupIcon::SteamDeck,
            "Steam Deck",
            &assessment.steam_deck,
            matches!(recommendation, BestOn::SteamDeck | BestOn::Both),
            cx,
        ))
        .child(fit_row(
            SetupIcon::Pc,
            "PC",
            &assessment.pc,
            matches!(recommendation, BestOn::Pc | BestOn::Both),
            cx,
        ))
        .child(div().text_xs().text_color(muted).child(if demo {
            "Fit rates each setup. Confidence rates the result. Demo values are examples."
        } else {
            "Fit uses Valve's Deck rating, controller support, and where you played. \
             PC assumes a capable Windows PC. Confidence shows how much evidence exists."
        }))
        .child(div().text_xs().text_color(muted).child("Your preference"))
        .child(
            h_flex()
                .flex_wrap()
                .gap_1()
                .children(
                    choices
                        .into_iter()
                        .enumerate()
                        .map(|(index, (choice, label))| {
                            Button::new(("setup-choice", index))
                                .xsmall()
                                .outline()
                                .label(label)
                                .selected(preference == choice)
                                .on_click(
                                    cx.listener(move |_, _, _, cx| {
                                        cx.emit(ChoosePreference(choice))
                                    }),
                                )
                        }),
                ),
        )
        .child(div().text_xs().text_color(muted).child(match preference {
            Some(choice) => format!("Filed under {} by your choice.", choice.best_on().label()),
            None => "Uses the recommendation.".into(),
        }))
}

/// One setup: icon, name, fit score with a bar, reason, and any blocker.
/// A recommended setup has a filled background. Unknown and blocked fits show
/// an empty bar, never a zero score.
fn fit_row(
    icon: SetupIcon,
    label: &'static str,
    fit: &SetupFit,
    recommended: bool,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let score = fit
        .score
        .filter(|score| *score <= 100 && fit.blocker.is_none());
    let value = if fit.blocker.is_some() {
        "Blocked".to_owned()
    } else {
        score.map_or("Unknown".into(), |score| format!("{score}/100"))
    };
    v_flex()
        .gap_1p5()
        .p_2()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .when(recommended, |row| row.bg(theme.secondary))
        .child(
            h_flex()
                .gap_2()
                .child(Icon::new(icon).size(px(16.)))
                .child(div().flex_1().text_sm().font_semibold().child(label))
                .child(
                    div()
                        .text_xs()
                        .text_color(if fit.blocker.is_some() {
                            theme.danger
                        } else {
                            theme.muted_foreground
                        })
                        .child(value),
                ),
        )
        .child(
            div()
                .h(px(4.))
                .w_full()
                .rounded_full()
                .bg(theme.muted_foreground.opacity(0.15))
                .when_some(score, |bar, score| {
                    bar.child(
                        div()
                            .h_full()
                            .w(relative(f32::from(score) / 100.))
                            .rounded_full()
                            .bg(theme
                                .foreground
                                .opacity(if recommended { 0.8 } else { 0.4 })),
                    )
                }),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(fit.reason.clone()),
        )
        .when_some(fit.blocker.clone(), |row, blocker| {
            row.child(div().text_xs().text_color(theme.danger).child(blocker))
        })
}
