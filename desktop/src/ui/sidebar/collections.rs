//! Sidebar collection edits and drops share the library's guarded write boundary.
use super::*;
use gamesync_desktop::library::{CollectionDefinition, LibraryRevision, LibraryStore};
use gpui_component::input::InputEvent;
use std::path::PathBuf;
use uuid::Uuid;

pub(super) struct NameEdit {
    pub input: Entity<InputState>,
    pub id: Option<Uuid>,
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

    pub fn begin_name(&mut self, id: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.edit.is_some() {
            return;
        }
        let library = self.library.read(cx);
        if let Some(issue) = &library.write_issue {
            self.message = format!("Collections unavailable: {issue}");
            cx.notify();
            return;
        }
        let Some((root, base)) = library.source.clone() else {
            return;
        };
        let name = id
            .and_then(|id| base.definitions.collections.iter().find(|c| c.id == id))
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Collection name")
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
            id,
            root,
            base,
            _subscription: subscription,
        });
        self.collections_open = true;
        self.collections_motion.set(1., cx);
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
        if let Some(id) = edit.id {
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

    fn write_definitions(&mut self, root: PathBuf, base: LibraryRevision, cx: &mut Context<Self>) {
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
                                let active_scope_exists = match lib.scope {
                                    crate::model::Scope::Collection(id) => saved
                                        .definitions
                                        .collections
                                        .iter()
                                        .any(|c| c.id == id && !c.archived),
                                    _ => true,
                                };
                                lib.source = Some((root, saved));
                                update_labels(lib);
                                if !active_scope_exists {
                                    lib.set_scope(crate::model::Scope::All);
                                }
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
                        this.add_game(game, id, cx)
                    },
                ))
            })
            .child(self.row(crate::model::Scope::Collection(id), IconName::Folder, cx))
            .context_menu(move |menu, _, _| {
                let rename = rename.clone();
                let remove = remove.clone();
                menu.item(PopupMenuItem::new("Rename").disabled(disabled).on_click(
                    move |_, window, cx| {
                        rename.update(cx, |this, cx| this.begin_name(Some(id), window, cx))
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

    fn add_game(
        &mut self,
        dragged: &super::super::grid::DraggedGame,
        id: Uuid,
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
            self.message = "Resolve this game's conflict before adding it.".into();
            cx.notify();
            return;
        }
        if record.game.personal.collections.contains(&id) {
            self.show_toast("Already in this collection.", cx);
            cx.notify();
            return;
        }
        let mut personal = record.game.personal.clone();
        personal.collections.push(id);
        self.busy = true;
        self.message = "Adding game…".into();
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
                        this.show_toast("Added to collection.", cx);
                        this.library.update(cx, |lib, cx| {
                            if lib.source.as_ref().is_some_and(|(path, _)| path == &root) {
                                if let Some(game) =
                                    lib.games.iter_mut().find(|g| g.id == saved.game_id)
                                {
                                    game.record = Some(saved);
                                }
                                update_labels(lib);
                                lib.set_scope(lib.scope.clone());
                                cx.notify();
                            }
                        });
                    }
                    Err(error) => {
                        this.message = format!("Not added: {error}. Refresh and try again.")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn update_labels(lib: &mut crate::model::Library) {
    let Some((_, manifest)) = &lib.source else {
        return;
    };
    for game in &mut lib.games {
        game.collections = manifest
            .definitions
            .collections
            .iter()
            .filter(|c| {
                !c.archived
                    && game
                        .record
                        .as_ref()
                        .is_some_and(|r| r.game.personal.collections.contains(&c.id))
            })
            .map(|c| c.name.clone())
            .collect();
    }
}
