//! Inline personal fields. Debounced writes keep drafts until revision checks succeed.
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
    v_flex, ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, StyledExt as _,
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
    scroll: gpui::ScrollHandle,
    saving: bool,
    pending: Option<gpui::Task<()>>,
    failed: bool,
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
        let mut subscriptions = vec![cx.observe(&library, |this, _, cx| {
            this.reconcile(cx);
            cx.notify();
        })];
        for input in [&tags, &notes] {
            subscriptions.push(cx.subscribe(input, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.changed(cx);
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
            scroll: gpui::ScrollHandle::new(),
            saving: false,
            pending: None,
            failed: false,
            published_from: None,
            message: String::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn game_id(&self) -> uuid::Uuid {
        self.base.game_id
    }

    fn reconcile(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let lib = self.library.read(cx);
        if let Some(latest) = lib
            .games
            .iter()
            .find(|g| g.id == self.base.game_id)
            .and_then(|g| g.record.clone())
        {
            // A provider refresh can advance the revision while the user types.
            // Rebase only when personal data is unchanged; otherwise retain the draft.
            if latest.revision_id != self.base.revision_id
                && Some(latest.revision_id) != self.published_from
                && latest.game.personal == self.base.game.personal
            {
                self.base = latest;
                self.published_from = None;
            }
        }
        if !self.failed && self.value(cx) != self.base.game.personal {
            self.schedule(cx);
        }
    }
    pub fn needs_reload(
        &self,
        record: &GameRevision,
        manifest: &LibraryRevision,
        cx: &App,
    ) -> bool {
        !self.busy(cx)
            && (&self.manifest != manifest
                || (record.revision_id != self.base.revision_id
                    && Some(record.revision_id) != self.published_from))
    }
    fn change_now(&mut self, cx: &mut Context<Self>) {
        self.changed(cx);
        self.save(cx);
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.failed = false;
        if !self.saving && self.value(cx) == self.base.game.personal {
            self.pending = None;
            self.message = "Saved.".into();
            cx.notify();
            return;
        }
        self.message = "Unsaved changes…".into();
        self.schedule(cx);
        cx.notify();
    }
    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.pending = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(450))
                .await;
            let _ = this.update(cx, |this, cx| this.save(cx));
        }));
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
            return Some("Collections or statuses changed. Your unsaved changes are kept.".into());
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
            return Some("This game changed elsewhere. Your unsaved changes are kept.".into());
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
        let submitted = personal.clone();
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
                        if this.value(cx) == submitted {
                            this.personal = record.game.personal.clone();
                        }
                        this.failed = false;
                        this.published_from = Some(this.base.revision_id);
                        this.base = record;
                        this.message = "Saved.".into();
                        cx.emit(EditorEvent::Saved);
                        if this.value(cx) != this.base.game.personal {
                            this.schedule(cx);
                        }
                    }
                    Err(error) => {
                        this.failed = true;
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
                    .track_scroll(&self.scroll)
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(self.base.game.title.clone()),
                    )
                    .child(hint(
                        &format!(
                            "{:.1} hours played",
                            self.base
                                .game
                                .steam
                                .as_ref()
                                .map_or(0, |s| s.playtime_minutes)
                                as f32
                                / 60.
                        ),
                        cx,
                    ))
                    .children(self.base.game.steam.as_ref().map(|steam| {
                        let id = steam.app_id;
                        Button::new("inline-store")
                            .ghost()
                            .label("View store page")
                            .on_click(move |_, _, cx| {
                                cx.open_url(&format!("https://store.steampowered.com/app/{id}/"))
                            })
                    }))
                    .child(hint("Status", cx))
                    .child(
                        Button::new("edit-status")
                            .label(status_label)
                            .dropdown_menu(move |mut menu, _, _| {
                                for status in &statuses {
                                    let target = target.clone();
                                    let key = status.key.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(status.label.clone())
                                            .checked(key == selected)
                                            .on_click(move |_, _, cx| {
                                                target.update(cx, |this, cx| {
                                                    this.personal.status = key.clone();
                                                    this.change_now(cx);
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
                                this.change_now(cx);
                            }))
                    })))
                    .child(
                        Checkbox::new("edit-favorite")
                            .label("Favorite")
                            .checked(self.personal.favorite)
                            .on_click(cx.listener(|this, value: &bool, _, cx| {
                                this.personal.favorite = *value;
                                this.change_now(cx);
                            })),
                    )
                    .child(hint("Collections", cx))
                    .child(
                        h_flex().flex_wrap().gap_2().children(
                            self.manifest
                                .definitions
                                .collections
                                .clone()
                                .into_iter()
                                .filter(|c| !c.archived)
                                .map(|collection| {
                                    let id = collection.id;
                                    Button::new(gpui::SharedString::from(format!("member-{id}")))
                                        .small()
                                        .label(collection.name)
                                        .selected(self.personal.collections.contains(&id))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.personal.collections.contains(&id) {
                                                this.personal.collections.retain(|v| *v != id);
                                            } else {
                                                this.personal.collections.push(id);
                                            }
                                            this.change_now(cx);
                                        }))
                                }),
                        ),
                    )
                    .child(hint("Tags · one per line", cx))
                    .child(
                        Input::new(&self.tags)
                            .h(px(64.))
                            .flex_shrink_0()
                            .small()
                            .disabled(false),
                    )
                    .child(hint("Notes", cx))
                    .child(
                        Input::new(&self.notes)
                            .h(px(88.))
                            .flex_shrink_0()
                            .small()
                            .disabled(false),
                    )
                    .children(
                        self.base
                            .game
                            .steam
                            .as_ref()
                            .and_then(|s| s.metadata.as_ref())
                            .map(|metadata| {
                                let mut facts = Vec::new();
                                facts.extend(metadata.release_year.clone());
                                facts.extend(metadata.review_label.clone());
                                if let Some(percent) = metadata.review_percent {
                                    facts.push(format!("{percent}% positive"));
                                }
                                facts.extend(metadata.genres.clone());
                                hint(&facts.join(" · "), cx)
                            }),
                    )
                    .child(hint("Description", cx))
                    .child(
                        div().text_sm().child(
                            self.base
                                .game
                                .description()
                                .unwrap_or("No Steam description.")
                                .to_owned(),
                        ),
                    ),
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
                    .when(self.failed || blocked.is_some(), |column| {
                        column.child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("retry-save")
                                        .label("Retry save")
                                        .disabled(saving || blocked.is_some())
                                        .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                                )
                                .child(
                                    Button::new("reload-details")
                                        .label("Discard unsaved changes")
                                        .disabled(saving)
                                        .on_click(
                                            cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Closed)),
                                        ),
                                ),
                        )
                    }),
            )
    }
}

fn hint(text: &str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_owned())
}
