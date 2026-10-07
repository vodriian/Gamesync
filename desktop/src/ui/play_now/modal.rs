//! Shared modal host keeps drafts attached to Play now and closes only after
//! a successful save or an explicit discard. The underlying game stays open.
use super::*;
use gpui::{px, Subscription};
use gpui_component::WindowExt as _;

struct EditorModal {
    view: Entity<PlayNowView>,
    kind: Modal,
    _subscription: Subscription,
}
impl Render for EditorModal {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.view.update(cx, |view, cx| match self.kind {
            Modal::Context => view.setup(true, cx),
            Modal::Profile(_) => view.profile_view(cx),
        })
    }
}
impl PlayNowView {
    pub(super) fn edit_context(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.busy() || self.active_play.is_some() {
            return;
        }
        self.context_before_edit = Some(self.context.clone());
        self.message.clear();
        self.open_modal(Modal::Context, window, cx);
    }
    pub(super) fn open_modal(
        &mut self,
        kind: Modal,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.modal = Some(kind);
        let view = cx.entity();
        let modal = cx.new(|cx| {
            let subscription = cx.observe_in(&view, window, |_, view, window, cx| {
                if view.read(cx).modal.is_none() {
                    window.close_dialog(cx);
                }
                cx.notify();
            });
            EditorModal {
                view: view.clone(),
                kind,
                _subscription: subscription,
            }
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            let target = view.clone();
            let footer = view.clone();
            dialog
                .title(match kind {
                    Modal::Context => "Your preferences",
                    Modal::Profile(_) => "Your game profile",
                })
                .w(px(if kind == Modal::Context { 590. } else { 720. }))
                .margin_top(px(40.))
                .h((window.viewport_size().height - px(80.)).min(px(760.)))
                .overlay_closable(false)
                .on_ok(|_, _, _| false)
                .close_button(!view.read(cx).busy())
                .keyboard(!view.read(cx).busy())
                .on_cancel(move |_, _, cx| {
                    target.update(cx, |view, cx| {
                        if view.saving
                            || view.pending_save.is_some()
                            || view.draft.as_ref().is_some_and(|d| d.dirty)
                        {
                            view.message = "Save the profile or discard your changes first.".into();
                            cx.notify();
                            return false;
                        }
                        if let Some(context) = view.context_before_edit.take() {
                            view.context = context;
                            view.change_context(None, cx);
                        }
                        view.draft = None;
                        // Dialog handles closure here; its observer must not close twice.
                        view.modal = None;
                        true
                    })
                })
                .child(modal.clone())
                .when(matches!(kind, Modal::Profile(_)), |dialog| {
                    dialog.footer(move |_, _, _, cx| {
                        vec![footer.update(cx, |view, cx| view.profile_actions(cx))]
                    })
                })
        });
        cx.notify();
    }
}
