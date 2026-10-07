//! Play now controls for the shared AI analysis job (`ui::analysis_job`).
//! Dealing never waits for analysis.
use super::*;
use crate::ui::analysis_job::Scope;
use gamesync_desktop::{ai, recommendations::analysis};
use gpui::{div, px, AnyElement, SharedString};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Sizable as _,
};

impl PlayNowView {
    /// Toolbar control: Analyze, or progress with Cancel, or Retry after a failure.
    pub(super) fn analysis_control(&self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let job = self.analysis.read(cx);
        let status = (!job.status().is_empty()).then(|| {
            div()
                .max_w(px(360.))
                .truncate()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(job.status().to_owned()))
        });
        let analysis = self.analysis.clone();
        let button = if job.running() {
            Button::new("analysis-cancel")
                .ghost()
                .label("Cancel")
                .disabled(job.cancelling())
                .on_click(move |_, _, cx| analysis.update(cx, |job, cx| job.cancel(cx)))
        } else if job.can_retry() {
            Button::new("analysis-retry")
                .ghost()
                .label("Retry")
                .icon(PlayIcon("ai-beautify"))
                .on_click(move |_, _, cx| analysis.update(cx, |job, cx| job.retry(cx)))
        } else {
            Button::new("analysis-start")
                .ghost()
                .label("Analyze")
                .icon(PlayIcon("ai-beautify"))
                .tooltip("Estimate missing game profiles with AI")
                .disabled(self.active_play.is_some())
                .on_click(move |_, window, cx| {
                    analysis.update(cx, |job, cx| job.confirm(Scope::PlayNow, window, cx))
                })
        };
        h_flex()
            .gap_2()
            .children(status)
            .child(button)
            .into_any_element()
    }

    /// Profile modal: the current AI suggestion for this game and a one-game request.
    pub(super) fn analysis_summary(&self, id: Uuid, cx: &mut gpui::Context<Self>) -> AnyElement {
        let game = self.game(id, cx);
        let record = game.as_ref().and_then(|g| g.record.as_ref());
        let current = record.and_then(|r| analysis::current_analysis(&r.game));
        let stale =
            record.is_some_and(|r| r.game.recommendation_analysis.is_some()) && current.is_none();
        let muted = cx.theme().muted_foreground;
        let heading = match current {
            Some(a) => {
                let name = ai::provider(&a.provider).map_or(a.provider.as_str(), |p| p.name);
                format!(
                    "AI suggestion from {name} ({}), {}% confident.",
                    a.model, a.confidence
                )
            }
            None if stale => "The AI suggestion is out of date: the game details changed.".into(),
            None => "No AI suggestion yet.".into(),
        };
        let job = self.analysis.read(cx);
        let running = job.running();
        let note: Option<SharedString> = if running || !job.status().is_empty() {
            Some(job.status().to_owned().into())
        } else if current.is_none() {
            Some(
                "This sends the title, Steam description, tags, and genres to your AI provider."
                    .into(),
            )
        } else {
            None
        };
        let analysis = self.analysis.clone();
        v_flex()
            .gap_2()
            .p_3()
            .rounded(px(7.))
            .bg(cx.theme().secondary)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_sm().child(heading))
                    .child(
                        Button::new("analysis-one")
                            .small()
                            .outline()
                            .icon(PlayIcon("ai-beautify"))
                            .label(if current.is_some() {
                                "Refresh suggestion"
                            } else {
                                "Get suggestion"
                            })
                            .disabled(running || self.saving)
                            .on_click(move |_, _, cx| {
                                analysis.update(cx, |job, cx| job.analyze_one(id, cx))
                            }),
                    ),
            )
            .children(
                current
                    .and_then(|a| a.reason.clone())
                    .map(|reason| div().text_sm().child(SharedString::from(reason))),
            )
            .children(note.map(|note| div().text_xs().text_color(muted).child(note)))
            .into_any_element()
    }
}
