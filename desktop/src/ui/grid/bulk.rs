use super::*;
use gamesync_desktop::{
    bulk::{self, Change},
    library::LibraryStore,
};
use gpui_component::{
    checkbox::Checkbox,
    menu::{DropdownMenu as _, PopupMenuItem},
    Sizable as _,
};

fn action(
    label: impl Into<gpui::SharedString>,
    change: Change,
    grid: &Entity<GameGrid>,
) -> PopupMenuItem {
    let grid = grid.clone();
    PopupMenuItem::new(label).on_click(move |_, _, cx| {
        grid.update(cx, |grid, cx| grid.apply_bulk(change.clone(), cx));
    })
}

impl GameGrid {
    pub(super) fn table_header(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let visible = self.library.read(cx).visible.len();
        h_flex()
            .absolute()
            .top(px(68.))
            .left_0()
            .right_0()
            .h(px(42.))
            .px(px(32.))
            .gap_4()
            .occlude()
            .bg(cx.theme().background)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .relative()
                    .child(
                        Checkbox::new("select-all-games")
                            .checked(visible > 0 && self.selection.len() == visible)
                            .disabled(visible == 0 || self.saving)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.selection.clear();
                                if *checked {
                                    let lib = this.library.read(cx);
                                    this.selection
                                        .extend(lib.visible.iter().map(|&i| lib.games[i].id));
                                }
                                window.focus(&this.focus);
                                this.feedback.clear();
                                cx.notify();
                            })),
                    )
                    .when(
                        !self.selection.is_empty() && self.selection.len() < visible,
                        |cell| {
                            cell.child(
                                div()
                                    .absolute()
                                    .left(px(4.))
                                    .top(px(7.))
                                    .w(px(8.))
                                    .h(px(2.))
                                    .bg(cx.theme().primary),
                            )
                        },
                    ),
            )
            .child(div().w(px(30.)))
            .child(div().flex_1().child("Title"))
            .child(div().w(px(140.)).child("Status"))
            .child(div().w(px(100.)).child("Rating"))
            .child(div().w(px(100.)).child("Playtime"))
            .into_any_element()
    }

    pub(super) fn bulk_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let lib = self.library.read(cx);
        let statuses = lib.statuses.clone();
        let collections = lib
            .source
            .as_ref()
            .map(|(_, m)| m.definitions.collections.clone())
            .unwrap_or_default();
        let hidden = lib.scope == crate::model::Scope::Hidden;
        let disabled = self.busy() || lib.write_issue.is_some();
        let grid = cx.entity();
        let favorite_grid = grid.clone();
        let status_grid = grid.clone();
        let collection_grid = grid.clone();
        h_flex()
            .absolute()
            .bottom_4()
            .left(px(20.))
            .gap_1()
            .p_1()
            .occlude()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .shadow_lg()
            .text_sm()
            .child(
                div()
                    .px_2()
                    .text_color(cx.theme().primary)
                    .child(if self.saving {
                        "Saving…".to_string()
                    } else {
                        format!("{} selected", self.selection.len())
                    }),
            )
            .child(
                Button::new("bulk-favorite")
                    .small()
                    .ghost()
                    .label("Favorite")
                    .disabled(disabled)
                    .dropdown_menu(move |menu, _, _| {
                        menu.item(action(
                            "Add to favorites",
                            Change::Favorite(true),
                            &favorite_grid,
                        ))
                        .item(action(
                            "Remove from favorites",
                            Change::Favorite(false),
                            &favorite_grid,
                        ))
                    }),
            )
            .child(
                Button::new("bulk-status")
                    .small()
                    .ghost()
                    .label("Status")
                    .disabled(disabled)
                    .dropdown_menu(move |mut menu, _, _| {
                        for status in &statuses {
                            menu = menu.item(action(
                                status.label.clone(),
                                Change::Status(status.key.clone()),
                                &status_grid,
                            ));
                        }
                        menu
                    }),
            )
            .child(
                Button::new("bulk-collection")
                    .small()
                    .ghost()
                    .label("Collection")
                    .disabled(disabled)
                    .dropdown_menu(move |mut menu, window, cx| {
                        for collection in collections.iter().filter(|c| !c.archived) {
                            let grid = collection_grid.clone();
                            let id = collection.id;
                            menu = menu.submenu(
                                collection.name.clone(),
                                window,
                                cx,
                                move |menu, _, _| {
                                    menu.item(action(
                                        "Add to collection",
                                        Change::Collection(id, true),
                                        &grid,
                                    ))
                                    .item(action(
                                        "Remove from collection",
                                        Change::Collection(id, false),
                                        &grid,
                                    ))
                                },
                            );
                        }
                        if collections.iter().all(|c| c.archived) {
                            menu = menu.label("No collections yet");
                        }
                        menu
                    }),
            )
            .child(
                Button::new("bulk-hide")
                    .small()
                    .ghost()
                    .label(if hidden { "Unhide" } else { "Hide" })
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.apply_bulk(Change::Hidden(!hidden), cx)
                    })),
            )
            .child(
                Button::new("clear-selection")
                    .small()
                    .ghost()
                    .label("Clear")
                    .disabled(self.saving)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selection.clear();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn apply_bulk(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.busy() || self.selection.is_empty() {
            return;
        }
        let lib = self.library.read(cx);
        if lib.write_issue.is_some() {
            return;
        }
        let Some((root, manifest)) = lib.source.clone() else {
            return;
        };
        let mut unavailable = Vec::new();
        let targets = self
            .selection
            .iter()
            .filter_map(|id| {
                if lib.conflicts.contains_key(id) {
                    unavailable.push((*id, "Resolve this game's conflict first".to_string()));
                    return None;
                }
                match lib
                    .games
                    .iter()
                    .find(|g| g.id == *id)
                    .and_then(|g| g.record.clone())
                {
                    Some(record) => Some(record),
                    None => {
                        unavailable.push((*id, "Game is no longer available".to_string()));
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        self.saving = true;
        self.feedback.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let path = root.clone();
            let result = cx.background_spawn(async move {
                let store = LibraryStore::open(&path)?;
                Ok::<_, anyhow::Error>(bulk::apply(&store, &manifest, targets, &change))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(mut outcome) => {
                        outcome.failed.extend(unavailable);
                        let count = outcome.saved.len();
                        if !outcome.failed.is_empty() {
                            this.selection = outcome.failed.iter().map(|(id, _)| *id).collect();
                        }
                        if this
                            .library
                            .read(cx)
                            .source
                            .as_ref()
                            .is_some_and(|(path, _)| path == &root)
                        {
                            // Bulk changes use the same grouped rows as context actions.
                            this.preserve_next_library_update = true;
                            this.library.update(cx, |lib, cx| {
                                lib.apply_personal_records(outcome.saved);
                                cx.notify();
                            });
                        } else {
                            this.selection.clear();
                        }
                        this.feedback = if let Some((_, error)) = outcome.failed.first() {
                            format!("{count} updated. {} could not be updated: {error}. Refresh and try again.", outcome.failed.len())
                        } else { cx.emit(super::BulkSaved(format!("{count} games updated.")));
                            String::new() };
                    }
                    Err(error) => this.feedback = format!("No games updated: {error}. Refresh and try again."),
                }
                cx.notify();
            });
        }).detach();
    }
}
