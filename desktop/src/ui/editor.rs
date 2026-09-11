//! One explicit inspector draft. Disk writes run on the background executor.
use crate::model::Library;
use gamesync_desktop::{
    library::{LibraryRevision, LibraryStore},
    records::{GameRevision, PersonalData},
};
use gpui::{div, prelude::*, px, App, Entity, EventEmitter, Subscription, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, Sizable as _, StyledExt as _,
};
use std::path::PathBuf;

pub enum EditorEvent {
    Saved,
    Closed,
}

pub struct InspectorEditor {
    library: Entity<Library>,
    root: PathBuf,
    manifest: LibraryRevision,
    base: GameRevision,
    personal: PersonalData,
    tags: Entity<InputState>,
    notes: Entity<InputState>,
    description: Entity<InputState>,
    saving: bool,
    published_from: Option<uuid::Uuid>,
    message: String,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<EditorEvent> for InspectorEditor {}

impl InspectorEditor {
    pub fn new(
        library: Entity<Library>,
        root: PathBuf,
        manifest: LibraryRevision,
        base: GameRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let personal = base.game.personal.clone();
        let tags = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(2)
                .default_value(personal.tags.join("\n"))
        });
        let notes = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(3)
                .default_value(personal.notes.clone())
        });
        let description = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(4)
                .default_value(personal.description.clone().unwrap_or_default())
        });
        let mut subscriptions = vec![cx.observe(&library, |_, _, cx| cx.notify())];
        for input in [&tags, &notes, &description] {
            subscriptions.push(cx.subscribe(input, |_, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }
        Self {
            library,
            root,
            manifest,
            base,
            personal,
            tags,
            notes,
            description,
            saving: false,
            published_from: None,
            message: String::new(),
            _subscriptions: subscriptions,
        }
    }

    fn value(&self, cx: &App) -> PersonalData {
        let mut value = self.personal.clone();
        value.tags = self
            .tags
            .read(cx)
            .value()
            .lines()
            .map(str::to_owned)
            .collect();
        value.notes = self.notes.read(cx).value().to_string();
        if value.description.is_some() {
            value.description = Some(self.description.read(cx).value().to_string());
        }
        value
    }

    pub fn busy(&self, cx: &App) -> bool {
        self.saving || self.value(cx) != self.base.game.personal
    }

    fn blocked(&self, cx: &App) -> Option<String> {
        let library = self.library.read(cx);
        if let Some(issue) = &library.write_issue {
            return Some(issue.clone());
        }
        if library.source.as_ref() != Some(&(self.root.clone(), self.manifest.clone())) {
            return Some(
                "Library changed. Discard this draft to review the latest definitions.".into(),
            );
        }
        let latest = library
            .games
            .iter()
            .find(|game| game.id == self.base.game_id)
            .and_then(|game| game.record.as_ref());
        // A confirmed write can reach this form before the reader sees it.
        // Keep Save disabled during that short gap without reporting a conflict.
        if self.published_from.is_some()
            && latest.map(|game| game.revision_id) == self.published_from
        {
            return Some("Waiting for library refresh.".into());
        }
        if latest.is_none_or(|game| game.revision_id != self.base.revision_id) {
            return Some(
                "Game changed. Your draft is kept. Discard it to review the latest details.".into(),
            );
        }
        None
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving || self.blocked(cx).is_some() {
            return;
        }
        let personal = self.value(cx);
        if personal == self.base.game.personal {
            return;
        }
        self.saving = true;
        self.message = "Saving…".into();
        let root = self.root.clone();
        let manifest = self.manifest.clone();
        let base = self.base.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    LibraryStore::open(root)?.edit_game_personal(
                        &manifest,
                        base.game_id,
                        base.revision_id,
                        personal,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(record) => {
                        this.personal = record.game.personal.clone();
                        this.published_from = Some(this.base.revision_id);
                        this.base = record;
                        this.message = "Saved.".into();
                        cx.emit(EditorEvent::Saved);
                    }
                    Err(error) => {
                        this.message = format!("Save failed: {error:#}. Your draft is kept.")
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for InspectorEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let blocked = self.blocked(cx);
        let dirty = self.value(cx) != self.base.game.personal;
        let saving = self.saving;
        let selected = self.personal.status.clone();
        let statuses = self.manifest.definitions.statuses.clone();
        let status_label = self
            .manifest
            .definitions
            .status(&selected)
            .map(|status| status.label.clone())
            .unwrap_or(selected.clone());
        let target = cx.entity();
        let rating = self.personal.rating;
        v_flex()
            .id("inspector-editor")
            .on_action(cx.listener(|this, _: &crate::SaveDetails, _, cx| this.save(cx)))
            .size_full()
            .child(
                v_flex()
                    .id("edit-fields")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(self.base.game.title.clone()),
                    )
                    .child(hint("Finish this edit to browse other details.", cx))
                    .child(hint("Status", cx))
                    .child(
                        Button::new("edit-status")
                            .label(status_label)
                            .disabled(saving)
                            .dropdown_menu(move |mut menu, _, _| {
                                for status in &statuses {
                                    let target = target.clone();
                                    let key = status.key.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(status.label.clone())
                                            .checked(key == selected)
                                            .on_click(move |_, _, cx| {
                                                target.update(cx, |this, cx| {
                                                    if !this.saving {
                                                        this.personal.status = key.clone();
                                                        cx.notify();
                                                    }
                                                })
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(hint("Your rating", cx))
                    .child(h_flex().children((1u8..=5).map(|stars| {
                        let value = stars * 2;
                        // Keep old half-star values visible; new choices use whole stars.
                        let fill = rating.unwrap_or(0).saturating_sub(value - 2).min(2);
                        Button::new(("rating-star", usize::from(stars)))
                            .ghost()
                            .w(px(36.))
                            .h(px(36.))
                            .p_0()
                            .disabled(saving)
                            .tooltip(if rating == Some(value) {
                                "Clear rating".to_owned()
                            } else {
                                format!(
                                    "Rate {stars} {}",
                                    if stars == 1 { "star" } else { "stars" }
                                )
                            })
                            .child(
                                div()
                                    .relative()
                                    .w(px(24.))
                                    .h(px(28.))
                                    .text_size(px(26.))
                                    .line_height(px(28.))
                                    .child("☆")
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .left_0()
                                            .overflow_hidden()
                                            .w(px(f32::from(fill) * 12.))
                                            .h_full()
                                            .child(div().w(px(24.)).child("★")),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.personal.rating =
                                    (this.personal.rating != Some(value)).then_some(value);
                                this.message.clear();
                                cx.notify();
                            }))
                    })))
                    .child(
                        Checkbox::new("edit-favorite")
                            .label("Favorite")
                            .checked(self.personal.favorite)
                            .disabled(saving)
                            .on_click(cx.listener(|this, value: &bool, _, cx| {
                                this.personal.favorite = *value;
                                cx.notify();
                            })),
                    )
                    .child(hint("Tags · one per line", cx))
                    .child(
                        Input::new(&self.tags)
                            .h(px(64.))
                            .flex_shrink_0()
                            .small()
                            .disabled(saving),
                    )
                    .child(hint("Notes", cx))
                    .child(
                        Input::new(&self.notes)
                            .h(px(88.))
                            .flex_shrink_0()
                            .small()
                            .disabled(saving),
                    )
                    .child(
                        Checkbox::new("edit-description-override")
                            .label("Use my description")
                            .checked(self.personal.description.is_some())
                            .disabled(saving)
                            .on_click(cx.listener(|this, value: &bool, _, cx| {
                                this.personal.description = value.then(String::new);
                                cx.notify();
                            })),
                    )
                    .when(self.personal.description.is_some(), |column| {
                        column.child(
                            Input::new(&self.description)
                                .h(px(112.))
                                .flex_shrink_0()
                                .small()
                                .disabled(saving),
                        )
                    })
                    .when(self.personal.description.is_none(), |column| {
                        column.child(
                            div().text_sm().child(
                                self.base
                                    .game
                                    .steam
                                    .as_ref()
                                    .and_then(|s| s.description.clone())
                                    .unwrap_or_else(|| "No Steam description.".into()),
                            ),
                        )
                    }),
            )
            .child(
                v_flex()
                    .p_3()
                    .flex_shrink_0()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .when(blocked.is_some(), |column| {
                        column.child(hint(blocked.as_deref().unwrap_or_default(), cx))
                    })
                    .when(!self.message.is_empty(), |column| {
                        column.child(hint(&self.message, cx))
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("save-details")
                                    .primary()
                                    .label("Save")
                                    .tooltip("Save details (⌘/Ctrl S)")
                                    .disabled(saving || !dirty || blocked.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                            )
                            .child(
                                Button::new("discard-details")
                                    .label(if dirty { "Discard" } else { "Done" })
                                    .disabled(saving)
                                    .on_click(
                                        cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Closed)),
                                    ),
                            ),
                    ),
            )
    }
}

fn hint(text: &str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_owned())
}
