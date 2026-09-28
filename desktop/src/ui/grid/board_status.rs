//! Inline status editing on the board. Library definitions own status keys,
//! labels, and order; the board, Filter menu, and editor all read them.
use super::*;
use gamesync_desktop::{board, library::LibraryRevision};
use gpui_component::{
    input::{Input, InputEvent, InputState},
    Sizable as _,
};
use std::path::PathBuf;

#[derive(Clone, PartialEq)]
pub(super) enum StatusTarget {
    New,
    Rename(String),
}

pub(super) struct StatusEdit {
    pub target: StatusTarget,
    input: Entity<InputState>,
    root: PathBuf,
    base: LibraryRevision,
    _subscription: gpui::Subscription,
}

impl GameGrid {
    pub(super) fn begin_status(
        &mut self,
        target: StatusTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let library = self.library.read(cx);
        if let Some(issue) = &library.write_issue {
            self.feedback = format!("Statuses unavailable: {issue}");
            cx.notify();
            return;
        }
        let Some((root, base)) = library.source.clone() else {
            return;
        };
        let label = match &target {
            StatusTarget::Rename(key) => base.definitions.status(key).map(|s| s.label.clone()),
            StatusTarget::New => None,
        }
        .unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Status name")
                .default_value(label)
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        let subscription = cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.save_status(cx);
            }
        });
        self.status_edit = Some(StatusEdit {
            target,
            input,
            root,
            base,
            _subscription: subscription,
        });
        self.feedback.clear();
        cx.notify();
    }

    pub(super) fn cancel_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.status_edit = None;
        self.feedback.clear();
        window.focus(&self.focus);
        cx.notify();
    }

    fn save_status(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = &self.status_edit else {
            return;
        };
        if self.saving {
            return;
        }
        let mut base = edit.base.clone();
        let label = edit.input.read(cx).value().to_string();
        let result = match &edit.target {
            StatusTarget::New => board::add_status(&mut base.definitions, &label).map(|_| ()),
            StatusTarget::Rename(key) => board::rename_status(&mut base.definitions, key, &label),
        };
        match result {
            Ok(()) => self.write_statuses(edit.root.clone(), base, cx),
            Err(error) => {
                self.feedback = error.to_string();
                cx.notify();
            }
        }
    }

    /// Move a status one place in the shared order. `later` moves it right.
    pub(super) fn move_status(&mut self, key: &str, later: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let library = self.library.read(cx);
        if library.write_issue.is_some() {
            return;
        }
        let Some((root, mut base)) = library.source.clone() else {
            return;
        };
        if board::move_status(&mut base.definitions, key, later) {
            self.write_statuses(root, base, cx);
        }
    }

    fn write_statuses(&mut self, root: PathBuf, base: LibraryRevision, cx: &mut Context<Self>) {
        self.saving = true;
        self.feedback.clear();
        cx.notify();
        let save = crate::ui::definitions::save(&self.library, root, base, cx);
        cx.spawn(async move |this, cx| {
            let result = save.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.status_edit = None;
                        this.restore_focus = true;
                    }
                    // An open editor keeps the typed name for another try.
                    Err(error) => this.feedback = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn status_editor(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(edit) = &self.status_edit else {
            return div().into_any_element();
        };
        v_flex()
            .p_2()
            .gap_2()
            .child(Input::new(&edit.input).small().disabled(self.saving))
            .when(!self.feedback.is_empty(), |editor| {
                editor.child(div().px_1().text_xs().child(self.feedback.clone()))
            })
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("save-status")
                            .small()
                            .primary()
                            .label(if self.saving { "Saving…" } else { "Save" })
                            .disabled(self.saving)
                            .on_click(cx.listener(|this, _, _, cx| this.save_status(cx))),
                    )
                    .child(
                        Button::new("cancel-status")
                            .small()
                            .ghost()
                            .label("Cancel")
                            .disabled(self.saving)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.cancel_status(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
}
