//! Best on on the open book's right page. Fit is the only metric: one scale
//! with Steam Deck growing left from the center and PC growing right. (i) has
//! a tooltip and opens the fit steps in a modal. Real games use Steam
//! evidence, your rules, and optional ProtonDB data; the demo uses saved
//! fixture values. The detail panel saves a preference through the editor.

use super::best_on_state::BestOnGlobal;
use crate::{assets::SetupIcon, model::Library};
use gamesync_desktop::suitability::{Assessment, BestOn, SetupFit, SetupPreference};
use gpui::{
    div, prelude::*, px, relative, rgb, App, Entity, EventEmitter, Hsla, SharedString, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _, StyledExt as _,
    WindowExt as _,
};
use uuid::Uuid;

pub(super) struct BestOnPanel {
    library: Entity<Library>,
    pub game_id: Uuid,
}

/// The user chose a setup. None returns to Automatic.
pub(super) struct ChoosePreference(pub Option<SetupPreference>);
impl EventEmitter<ChoosePreference> for BestOnPanel {}

impl BestOnPanel {
    pub fn new(library: Entity<Library>, game_id: Uuid, cx: &mut Context<Self>) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        if let Some(state) = cx
            .try_global::<BestOnGlobal>()
            .map(|global| global.0.clone())
        {
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
        }
        Self { library, game_id }
    }
}

/// Setup colors from the Elyx tokens `deck` and `pc`.
fn setup_colors(cx: &App) -> (Hsla, Hsla) {
    if cx.theme().is_dark() {
        (rgb(0x9485ff).into(), rgb(0x5cc785).into())
    } else {
        (rgb(0x5638ff).into(), rgb(0x13aa48).into())
    }
}

/// "PC only" when Steam Deck is blocked, so a blocker reads as a result.
fn result_label(assessment: &Assessment) -> &'static str {
    match assessment.recommendation() {
        BestOn::Pc if assessment.steam_deck.blocker.is_some() => "PC only",
        other => other.label(),
    }
}

/// The fit of the recommended setup; Both shows the higher one.
fn headline_fit(assessment: &Assessment) -> Option<u8> {
    let deck = assessment
        .steam_deck
        .score
        .filter(|_| assessment.steam_deck.blocker.is_none());
    let pc = assessment
        .pc
        .score
        .filter(|_| assessment.pc.blocker.is_none());
    match assessment.recommendation() {
        BestOn::SteamDeck => deck,
        BestOn::Pc => pc,
        BestOn::Both => deck.max(pc),
        BestOn::NeedsReview => None,
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
        let muted = cx.theme().muted_foreground;
        let fit = headline_fit(&assessment);
        let title: SharedString = record.game.title.clone().into();
        let details = assessment.clone();

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
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .child(result_label(&assessment)),
                            )
                            .child(
                                Button::new("best-on-info")
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(SetupIcon::Info).size(px(16.)))
                                    .tooltip("How fit is calculated")
                                    .on_click(move |_, window, cx| {
                                        open_fit_details(
                                            title.clone(),
                                            details.clone(),
                                            demo,
                                            window,
                                            cx,
                                        )
                                    }),
                            ),
                    )
                    .child(match fit {
                        Some(fit) => h_flex()
                            .items_baseline()
                            .child(div().text_sm().font_medium().child(fit.to_string()))
                            .child(div().text_xs().font_medium().child("/100")),
                        None => h_flex().child(div().text_sm().text_color(muted).child("–")),
                    }),
            )
            .child(fit_scale(&assessment, cx))
            .child(div().text_xs().text_color(muted).child("Your preference"))
            .child(preference_choices(preference, cx))
            .into_any_element()
    }
}

/// Both setups on one scale. Each half is 0–100 from the center mark.
/// Unknown and unsupported fits keep an empty half, never a zero score. The
/// reasons are in the fit details, not under the scale.
fn fit_scale(assessment: &Assessment, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let (deck_color, pc_color) = setup_colors(cx);
    let track = theme.muted_foreground.opacity(0.2);
    let usable = |fit: &SetupFit| {
        fit.score
            .filter(|score| *score <= 100 && fit.blocker.is_none())
    };
    let deck = usable(&assessment.steam_deck);
    let pc = usable(&assessment.pc);
    let label = |fit: &SetupFit, score: Option<u8>| match (score, &fit.blocker) {
        (_, Some(_)) => "Not supported".to_owned(),
        (Some(score), None) => format!("{score}/100"),
        (None, None) => "Unknown".to_owned(),
    };
    let half = |score: Option<u8>, color: Hsla, from_end: bool| {
        h_flex()
            .flex_1()
            .h(px(4.))
            .rounded_full()
            .bg(track)
            .when(from_end, |bar| bar.justify_end())
            .when_some(score, |bar, score| {
                bar.child(
                    div()
                        .h_full()
                        .w(relative(f32::from(score) / 100.))
                        .rounded_full()
                        .bg(color),
                )
            })
    };
    let side = |icon: SetupIcon, name: &'static str, color: Hsla, blocked: bool, end: bool| {
        v_flex()
            .gap_2()
            .when(end, |side| side.items_end())
            .child(Icon::new(icon).size(px(16.)).text_color(if blocked {
                theme.muted_foreground
            } else {
                color
            }))
            .child(
                div()
                    .text_sm()
                    .font_medium()
                    .when(blocked, |text| text.text_color(theme.muted_foreground))
                    .child(name),
            )
    };
    let deck_blocked = assessment.steam_deck.blocker.is_some();
    v_flex()
        .gap_2()
        .p_2()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.secondary)
        .child(
            h_flex()
                .justify_between()
                .items_end()
                .child(side(
                    SetupIcon::SteamDeck,
                    "Steam Deck",
                    deck_color,
                    deck_blocked,
                    false,
                ))
                .child(side(
                    SetupIcon::Pc,
                    "PC",
                    pc_color,
                    assessment.pc.blocker.is_some(),
                    true,
                )),
        )
        .child(
            h_flex()
                .h(px(12.))
                .items_center()
                .child(half(deck, deck_color, true))
                .child(
                    div()
                        .w(px(4.))
                        .h(px(12.))
                        .rounded_full()
                        .bg(theme.selection),
                )
                .child(half(pc, pc_color, false)),
        )
        .child(
            h_flex()
                .justify_between()
                .text_xs()
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(label(&assessment.steam_deck, deck)),
                )
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(label(&assessment.pc, pc)),
                ),
        )
}

fn preference_choices(
    preference: Option<SetupPreference>,
    cx: &mut Context<BestOnPanel>,
) -> impl IntoElement {
    let choices = [
        (None, "Automatic"),
        (Some(SetupPreference::SteamDeck), "Steam Deck"),
        (Some(SetupPreference::Pc), "PC"),
        (Some(SetupPreference::Both), "Both"),
    ];
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
                        .on_click(cx.listener(move |_, _, _, cx| cx.emit(ChoosePreference(choice))))
                }),
        )
}

/// The fit steps for each setup, in a modal.
fn open_fit_details(
    title: SharedString,
    assessment: Assessment,
    demo: bool,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        let (deck_color, pc_color) = setup_colors(cx);
        let recommendation = assessment.recommendation();
        let deck = &assessment.steam_deck;
        let pc = &assessment.pc;
        let outcome = match (recommendation, deck.score, pc.score) {
            (BestOn::SteamDeck, Some(d), Some(p)) => {
                format!("Steam Deck is {} points ahead, so this game is best on Steam Deck.", d.saturating_sub(p))
            }
            (BestOn::Pc, _, _) if deck.blocker.is_some() => {
                "Steam Deck is blocked, so this game is best on PC.".to_owned()
            }
            (BestOn::Pc, Some(d), Some(p)) => {
                format!("PC is {} points ahead, so this game is best on PC.", p.saturating_sub(d))
            }
            (BestOn::Both, _, _) => "The fits are within 15 points, so both setups suit it.".to_owned(),
            _ => "There is not enough evidence for a recommendation.".to_owned(),
        };
        let uses_proton = deck.steps.iter().any(|step| step.label.starts_with("ProtonDB"));
        dialog
            .title(format!("Fit for {title}"))
            .w(px(460.))
            .child(
                v_flex()
                    .gap_4()
                    .child(div().text_xs().text_color(muted).child(if demo {
                        "Demo values are examples. Fit rates how well each setup suits this game, from 0 to 100."
                    } else {
                        "Fit rates how well each setup suits this game, from 0 to 100. Steam data comes first, then where you played, then your rules. Your preference on the card always wins."
                    }))
                    .child(fit_steps(SetupIcon::SteamDeck, "Steam Deck", deck, deck_color, cx))
                    .child(fit_steps(SetupIcon::Pc, "PC", pc, pc_color, cx))
                    .child(div().text_xs().text_color(muted).child(format!(
                        "{outcome} Fits within 15 points show Both."
                    )))
                    .when(uses_proton, |column| {
                        column.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(gamesync_desktop::protondb::CREDIT),
                        )
                    }),
            )
            .footer(move |_, _, _, _| {
                vec![
                    Button::new("fit-rules")
                        .label("Edit rules in Settings")
                        .on_click(|_, window, cx| {
                            window.close_dialog(cx);
                            if let Some(state) =
                                cx.try_global::<BestOnGlobal>().map(|global| global.0.clone())
                            {
                                state.update(cx, |state, cx| {
                                    state.show_rules = true;
                                    cx.notify();
                                });
                            }
                            window.dispatch_action(Box::new(crate::OpenSettings), cx);
                        })
                        .into_any_element(),
                    Button::new("fit-close")
                        .primary()
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx))
                        .into_any_element(),
                ]
            })
    });
}

/// One setup: its total, then each step with its points.
fn fit_steps(
    icon: SetupIcon,
    name: &'static str,
    fit: &SetupFit,
    color: Hsla,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let total = match (&fit.blocker, fit.score) {
        (Some(_), _) => "Not supported".to_owned(),
        (None, Some(score)) => format!("{score}/100"),
        (None, None) => "Unknown".to_owned(),
    };
    let row = |label: String, value: String| {
        h_flex()
            .justify_between()
            .gap_3()
            .text_sm()
            .items_start()
            // The label wraps; the points keep their column.
            .child(div().flex_1().min_w_0().child(label))
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(theme.muted_foreground)
                    .child(value),
            )
    };
    let mut column = v_flex().gap_1().child(
        h_flex()
            .justify_between()
            .text_sm()
            .font_medium()
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(icon).size(px(16.)).text_color(color))
                    .child(name),
            )
            .child(total),
    );
    if fit.steps.is_empty() {
        column = column.child(div().text_sm().child(fit.reason.clone()));
    }
    for step in &fit.steps {
        let value = match (step.start, step.points) {
            (true, 0) => String::new(),
            (true, points) => points.to_string(),
            (false, 0) => "0".into(),
            (false, points) if points > 0 => format!("+{points}"),
            (false, points) => format!("−{}", -points),
        };
        column = column.child(row(step.label.clone(), value));
    }
    column.when_some(fit.blocker.clone(), |column, blocker| {
        column.child(div().text_sm().child(blocker))
    })
}
