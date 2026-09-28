//! Sidebar collection and status edits and drops share the library's guarded write boundary.
use super::*;
use gamesync_desktop::{
    bulk::Change,
    library::{CollectionDefinition, LibraryRevision, LibraryStore},
};
use gpui_component::input::InputEvent;
use std::path::PathBuf;
use uuid::Uuid;

/// The definition one inline name editor creates or renames.
#[derive(Clone, PartialEq)]
pub enum NameTarget {
    NewCollection,
    Collection(Uuid),
    NewStatus,
    Status(String),
}

pub(super) struct NameEdit {
    pub input: Entity<InputState>,
    pub target: NameTarget,
    root: PathBuf,
    base: LibraryRevision,
    _subscription: gpui::Subscription,
}

impl LibrarySidebar {
    pub(super) fn name_editor(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(edit) = &self.edit else {
            return div().into_any_element();
        };
        v_flex()
            .gap_1()
            .flex_shrink_0()
            .child(Input::new(&edit.input).small().disabled(self.busy))
            .when(!self.message.is_empty(), |editor| {
                editor.child(div().px_1().text_xs().child(self.message.clone()))
            })
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("save-collection-name")
                            .small()
                            .primary()
                            .label("Save")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.save_name(cx))),
                    )
                    .child(
                        Button::new("cancel-collection-name")
                            .text_color(cx.theme().sidebar_foreground)
                            .small()
                            .ghost()
                            .label("Cancel")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit = None;
                                this.message.clear();
                                window.focus(&this.focus);
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn begin_name(&mut self, target: NameTarget, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.edit.is_some() {
            return;
        }
        let library = self.library.read(cx);
        if let Some(issue) = &library.write_issue {
            self.message = format!("Changes unavailable: {issue}");
            cx.notify();
            return;
        }
        let Some((root, base)) = library.source.clone() else {
            return;
        };
        let name = match &target {
            NameTarget::Collection(id) => base
                .definitions
                .collections
                .iter()
                .find(|c| c.id == *id)
                .map(|c| c.name.clone()),
            NameTarget::Status(key) => base.definitions.status(key).map(|s| s.label.clone()),
            NameTarget::NewCollection | NameTarget::NewStatus => None,
        }
        .unwrap_or_default();
        let status = matches!(target, NameTarget::NewStatus | NameTarget::Status(_));
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(if status {
                    "Status name"
                } else {
                    "Collection name"
                })
                .default_value(name)
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        let subscription = cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.save_name(cx);
            }
        });
        self.edit = Some(NameEdit {
            input,
            target,
            root,
            base,
            _subscription: subscription,
        });
        if status {
            self.status_open = true;
            self.status_motion.set(1., cx);
        } else {
            self.collections_open = true;
            self.collections_motion.set(1., cx);
        }
        self.message.clear();
        cx.notify();
    }

    pub(super) fn save_name(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(edit) = &self.edit else {
            return;
        };
        let mut base = edit.base.clone();
        let name = edit.input.read(cx).value().trim().to_owned();
        let status = match &edit.target {
            NameTarget::NewStatus => {
                Some(gamesync_desktop::board::add_status(&mut base.definitions, &name).map(|_| ()))
            }
            NameTarget::Status(key) => Some(gamesync_desktop::board::rename_status(
                &mut base.definitions,
                key,
                &name,
            )),
            _ => None,
        };
        if let Some(result) = status {
            match result {
                Ok(()) => self.write_definitions(edit.root.clone(), base, cx),
                Err(error) => {
                    self.message = error.to_string();
                    cx.notify();
                }
            }
            return;
        }
        if name.is_empty() || name.len() > 120 {
            self.message = if name.is_empty() {
                "Enter a collection name."
            } else {
                "Choose a shorter collection name."
            }
            .into();
            cx.notify();
            return;
        }
        if let NameTarget::Collection(id) = edit.target {
            let Some(collection) = base.definitions.collections.iter_mut().find(|c| c.id == id)
            else {
                return;
            };
            collection.name = name;
        } else {
            base.definitions.collections.push(CollectionDefinition {
                id: Uuid::new_v4(),
                name,
                archived: false,
                extra: Default::default(),
            });
        }
        self.write_definitions(edit.root.clone(), base, cx);
    }

    fn remove_collection(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let library = self.library.read(cx);
        if library.write_issue.is_some() {
            return;
        }
        let Some((root, mut base)) = library.source.clone() else {
            return;
        };
        let Some(collection) = base.definitions.collections.iter_mut().find(|c| c.id == id) else {
            return;
        };
        collection.archived = true;
        self.write_definitions(root, base, cx);
    }

    pub(super) fn write_definitions(
        &mut self,
        root: PathBuf,
        base: LibraryRevision,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = base.definitions.validate() {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        self.busy = true;
        self.message = "Saving…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let path = root.clone();
            let result = cx
                .background_spawn(async move {
                    LibraryStore::open(path)?.edit(base.revision_id, base.definitions)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(saved) => {
                        this.edit = None;
                        this.restore_focus = true;
                        this.message.clear();
                        this.library.update(cx, |lib, cx| {
                            if lib.source.as_ref().is_some_and(|(path, _)| path == &root) {
                                lib.apply_definitions(root, saved);
                                cx.notify();
                            }
                        });
                    }
                    Err(error) => {
                        this.message =
                            format!("Not saved: {error}. Cancel and try again after refresh.")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn collection_row(&self, id: Uuid, cx: &mut Context<Self>) -> gpui::AnyElement {
        let rename = cx.entity();
        let remove = cx.entity();
        let disabled =
            self.busy || self.edit.is_some() || self.library.read(cx).write_issue.is_some();
        div()
            .id(gpui::SharedString::from(format!("collection-{id}")))
            .rounded(cx.theme().radius)
            .when(!disabled, |row| {
                row.drag_over::<super::super::grid::DraggedGame>(|style, _, _, cx| {
                    style.bg(cx.theme().primary.opacity(0.25))
                })
                .on_drop(cx.listener(
                    move |this, game: &super::super::grid::DraggedGame, _, cx| {
                        this.drop_game(game, Change::Collection(id, true), cx)
                    },
                ))
            })
            .child(self.row(crate::model::Scope::Collection(id), IconName::Folder, cx))
            .context_menu(move |menu, _, _| {
                let rename = rename.clone();
                let remove = remove.clone();
                menu.item(PopupMenuItem::new("Rename").disabled(disabled).on_click(
                    move |_, window, cx| {
                        rename.update(cx, |this, cx| {
                            this.begin_name(NameTarget::Collection(id), window, cx)
                        })
                    },
                ))
                .separator()
                .item(
                    PopupMenuItem::new("Remove collection")
                        .disabled(disabled)
                        .on_click(move |_, _, cx| {
                            remove.update(cx, |this, cx| this.remove_collection(id, cx))
                        }),
                )
            })
            .into_any_element()
    }

    /// Apply one personal change to a dropped game: add it to a collection or
    /// move it to a status. Status drops clear the board position.
    pub(super) fn drop_game(
        &mut self,
        dragged: &super::super::grid::DraggedGame,
        change: Change,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.edit.is_some() {
            return;
        }
        let lib = self.library.read(cx);
        if let Some(issue) = &lib.write_issue {
            self.message = format!("Not saved: {issue}");
            cx.notify();
            return;
        }
        let Some((root, manifest)) = lib.source.clone() else {
            return;
        };
        if dragged.library_id != Some(manifest.library_id) {
            return;
        }
        let Some(record) = lib
            .games
            .iter()
            .find(|g| g.id == dragged.id)
            .and_then(|g| g.record.clone())
        else {
            return;
        };
        if lib.conflicts.contains_key(&dragged.id) {
            self.message = "Resolve this game's conflict before changing it.".into();
            cx.notify();
            return;
        }
        let (done, already) = match &change {
            Change::Status(key) => {
                let label = lib.scope_label(&crate::model::Scope::Status(key.clone()));
                (format!("Moved to {label}."), format!("Already in {label}."))
            }
            _ => (
                "Added to collection.".to_owned(),
                "Already in this collection.".to_owned(),
            ),
        };
        let mut personal = record.game.personal.clone();
        change.apply(&mut personal);
        if personal == record.game.personal {
            self.show_toast(&already, cx);
            return;
        }
        self.busy = true;
        self.message = "Saving game…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let path = root.clone();
            let result = cx
                .background_spawn(async move {
                    LibraryStore::open(path)?.edit_game_personal(
                        &manifest,
                        record.game_id,
                        record.revision_id,
                        personal,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(saved) => {
                        this.show_toast(&done, cx);
                        this.library.update(cx, |lib, cx| {
                            if lib.source.as_ref().is_some_and(|(path, _)| path == &root) {
                                lib.apply_personal_record(saved);
                                cx.notify();
                            }
                        });
                    }
                    Err(error) => {
                        this.message = format!("Not saved: {error}. Refresh and try again.")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
