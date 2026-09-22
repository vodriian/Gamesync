//! Context actions edit only personal fields against the revisions shown in the menu.
use super::*;
use gamesync_desktop::{
    library::{LibraryRevision, LibraryStore},
    records::{GameRevision, PersonalData},
};
use gpui_component::{
    input::{Input, InputState},
    menu::{PopupMenu, PopupMenuItem},
};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone)]
struct Target {
    root: PathBuf,
    manifest: LibraryRevision,
    base: GameRevision,
}

pub(super) struct NoteDraft {
    target: Target,
    input: Entity<InputState>,
}

use gamesync_desktop::bulk::Change;

fn item(
    label: String,
    checked: bool,
    change: Change,
    target: &Target,
    grid: &Entity<GameGrid>,
) -> PopupMenuItem {
    let target = target.clone();
    let grid = grid.clone();
    PopupMenuItem::new(label)
        .checked(checked)
        .on_click(move |_, _, cx| {
            let target = target.clone();
            let mut personal = target.base.game.personal.clone();
            change.clone().apply(&mut personal);
            grid.update(cx, |grid, cx| {
                grid.save_personal(target, personal, false, cx)
            });
        })
}

pub(super) fn menu(
    grid: Entity<GameGrid>,
    id: Uuid,
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let view = grid.read(cx);
    let library = view.library.read(cx);
    if view.busy() || library.write_issue.is_some() || library.conflicts.contains_key(&id) {
        return menu.item(PopupMenuItem::new("Game changes are unavailable").disabled(true));
    }
    let Some((root, manifest)) = library.source.clone() else {
        return menu.label("Games are still loading");
    };
    let Some(base) = library
        .games
        .iter()
        .find(|g| g.id == id)
        .and_then(|g| g.record.clone())
    else {
        return menu;
    };
    let target = Target {
        root,
        manifest,
        base,
    };
    let personal = &target.base.game.personal;
    let favorite = personal.favorite;
    let hidden = personal.hidden;
    let has_note = !personal.notes.is_empty();
    let status_target = target.clone();
    let status_grid = grid.clone();
    let collection_target = target.clone();
    let collection_grid = grid.clone();
    let note_target = target.clone();
    let note_grid = grid.clone();
    menu.item(item(
        if favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        }
        .into(),
        favorite,
        Change::Favorite(!favorite),
        &target,
        &grid,
    ))
    .submenu("Status", window, cx, move |mut menu, _, _| {
        for status in &status_target.manifest.definitions.statuses {
            menu = menu.item(item(
                status.label.clone(),
                status.key == status_target.base.game.personal.status,
                Change::Status(status.key.clone()),
                &status_target,
                &status_grid,
            ));
        }
        menu
    })
    .submenu("Collections", window, cx, move |mut menu, _, _| {
        let collections: Vec<_> = collection_target
            .manifest
            .definitions
            .collections
            .iter()
            .filter(|c| !c.archived)
            .collect();
        if collections.is_empty() {
            return menu.item(PopupMenuItem::new("No collections yet").disabled(true));
        }
        for collection in collections {
            let member = collection_target
                .base
                .game
                .personal
                .collections
                .contains(&collection.id);
            menu = menu.item(item(
                format!(
                    "{} {}",
                    if member { "Remove from" } else { "Add to" },
                    collection.name
                ),
                member,
                Change::Collection(collection.id, !member),
                &collection_target,
                &collection_grid,
            ));
        }
        menu
    })
    .separator()
    .item(
        PopupMenuItem::new(if has_note {
            "Edit note…"
        } else {
            "Add note…"
        })
        .on_click(move |_, window, cx| {
            note_grid.update(cx, |grid, cx| {
                if grid.busy() {
                    return;
                }
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true)
                        .rows(5)
                        .placeholder("Write a note…")
                        .default_value(note_target.base.game.personal.notes.clone())
                });
                input.update(cx, |input, cx| input.focus(window, cx));
                grid.note = Some(NoteDraft {
                    target: note_target.clone(),
                    input,
                });
                grid.feedback.clear();
                cx.notify();
            });
        }),
    )
    .separator()
    .item(item(
        if hidden { "Unhide game" } else { "Hide game" }.into(),
        false,
        Change::Hidden(!hidden),
        &target,
        &grid,
    ))
}

impl GameGrid {
    pub fn busy(&self) -> bool {
        self.saving || self.note.is_some()
    }

    fn save_personal(
        &mut self,
        target: Target,
        personal: PersonalData,
        note: bool,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        if self.library.read(cx).write_issue.is_some() {
            self.feedback = "Changes are unavailable. Refresh and try again.".into();
            cx.notify();
            return;
        }
        if personal == target.base.game.personal {
            if note {
                self.note = None;
                self.restore_focus = true;
            }
            cx.notify();
            return;
        }
        self.saving = true;
        self.feedback.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let root = target.root.clone();
            let result = cx
                .background_spawn(async move {
                    LibraryStore::open(&target.root)?.edit_game_personal(
                        &target.manifest,
                        target.base.game_id,
                        target.base.revision_id,
                        personal,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(record) => {
                        if this
                            .library
                            .read(cx)
                            .source
                            .as_ref()
                            .is_some_and(|(path, _)| path == &root)
                        {
                            // This edit can move the game between group rows. Keep the
                            // current virtual-list offset for that one model update.
                            this.preserve_next_library_update = true;
                            this.library.update(cx, |lib, cx| {
                                lib.apply_personal_record(record);
                                cx.notify();
                            });
                        }
                        if note {
                            this.note = None;
                        }
                        this.restore_focus = true;
                    }
                    Err(error) => {
                        this.feedback = if note { format!("Not saved: {error}. Your note is kept here. Copy it before cancelling and reopening the note.") } else { format!("Not saved: {error}. Refresh and try again.") };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn note_dialog(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(note) = &self.note else {
            return div().into_any_element();
        };
        div()
            .id("note-overlay")
            .absolute()
            .size_full()
            .inset_0()
            .bg(gpui::black().opacity(0.25))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && !this.saving {
                    this.note = None;
                    this.feedback.clear();
                    window.focus(&this.focus);
                    cx.notify();
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .child(
                v_flex()
                    .w(px(380.))
                    .max_w_full()
                    .p_5()
                    .gap_3()
                    .rounded_lg()
                    .shadow_lg()
                    .bg(cx.theme().popover)
                    .text_color(cx.theme().popover_foreground)
                    .child(
                        div()
                            .font_semibold()
                            .child(format!("Note · {}", note.target.base.game.title)),
                    )
                    .child(Input::new(&note.input).h(px(140.)).disabled(self.saving))
                    .when(!self.feedback.is_empty(), |view| {
                        view.child(div().text_sm().child(self.feedback.clone()))
                    })
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-note")
                                    .label("Cancel")
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.note = None;
                                        this.feedback.clear();
                                        window.focus(&this.focus);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("save-note")
                                    .primary()
                                    .label(if self.saving { "Saving…" } else { "Save" })
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let Some(note) = &this.note else {
                                            return;
                                        };
                                        let mut personal = note.target.base.game.personal.clone();
                                        personal.notes = note.input.read(cx).value().to_string();
                                        this.save_personal(note.target.clone(), personal, true, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
