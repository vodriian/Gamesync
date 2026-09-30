//! Inline personal fields. Debounced writes keep drafts until revision checks succeed.
use crate::model::Library;
use gamesync_desktop::{
    library::{LibraryRevision, LibraryStore},
    records::{GameRevision, PersonalData},
};
use gpui::{
    div, prelude::*, px, App, Corner, Entity, EventEmitter, Focusable as _, Subscription, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    popover::Popover,
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
};
use std::path::PathBuf;

pub enum EditorEvent {
    Saved,
    Closed,
    /// Leave the card and show a library scope, for example a genre.
    ShowScope(crate::model::Scope),
}

pub struct InspectorEditor {
    library: Entity<Library>,
    root: PathBuf,
    manifest: LibraryRevision,
    base: GameRevision,
    personal: PersonalData,
    /// Search text in the Add tag picker. Tags themselves live in `personal`.
    tag_query: Entity<InputState>,
    notes: Entity<InputState>,
    scroll: gpui::ScrollHandle,
    saving: bool,
    pending: Option<gpui::Task<()>>,
    failed: bool,
    published_from: Option<uuid::Uuid>,
    message: String,
    /// The control changed last. Save status shows under it.
    field: Option<Field>,
    /// Hides "Saved." after a short time.
    message_clear: Option<gpui::Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<EditorEvent> for InspectorEditor {}

/// Places for inline save status. `Top` is used before any control changes,
/// for example when the game changed on another computer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Top,
    Status,
    Rating,
    Collections,
    Tags,
    Notes,
}

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
        let tag_query =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search or create a tag"));
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
        subscriptions.push(cx.subscribe(&notes, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.field = Some(Field::Notes);
                this.changed(cx);
                cx.notify();
            }
        }));
        // Enter adds the first match, or creates the typed tag.
        subscriptions.push(
            cx.subscribe_in(
                &tag_query,
                window,
                |this, _, event, window, cx| match event {
                    InputEvent::PressEnter { .. } => {
                        let (matches, create) = this.tag_suggestions(cx);
                        if let Some(tag) = matches.into_iter().next().or(create) {
                            this.add_tag(&tag, window, cx);
                        }
                    }
                    InputEvent::Change => cx.notify(),
                    _ => {}
                },
            ),
        );
        Self {
            library,
            root,
            manifest,
            base,
            personal,
            tag_query,
            notes,
            scroll: gpui::ScrollHandle::new(),
            saving: false,
            pending: None,
            failed: false,
            published_from: None,
            message: String::new(),
            field: None,
            message_clear: None,
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
    fn change_now(&mut self, field: Field, cx: &mut Context<Self>) {
        self.field = Some(field);
        self.changed(cx);
        self.save(cx);
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.failed = false;
        if !self.saving && self.value(cx) == self.base.game.personal {
            self.pending = None;
            self.show_saved(cx);
            return;
        }
        self.message_clear = None;
        self.message = "Unsaved changes…".into();
        self.schedule(cx);
        cx.notify();
    }
    /// "Saved." is brief confirmation, so it clears itself after two seconds.
    fn show_saved(&mut self, cx: &mut Context<Self>) {
        self.message = "Saved.".into();
        self.message_clear = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.message == "Saved." {
                    this.message.clear();
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    /// Save status, errors, and recovery actions under the control that
    /// changed. Nothing is shown when there is nothing to report.
    fn status_line(&self, slot: Field, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.field.unwrap_or(Field::Top) != slot {
            return None;
        }
        let blocked = self.blocked(cx);
        if self.message.is_empty() && blocked.is_none() && !self.failed {
            return None;
        }
        let saving = self.saving;
        Some(
            v_flex()
                .gap_1()
                .children(blocked.as_deref().map(|text| hint(text, cx)))
                .when(!self.message.is_empty(), |column| {
                    column.child(hint(&self.message, cx))
                })
                .when(self.failed || blocked.is_some(), |column| {
                    column.child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("retry-save")
                                    .xsmall()
                                    .label("Retry save")
                                    .disabled(saving || blocked.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                            )
                            .child(
                                Button::new("reload-details")
                                    .xsmall()
                                    .ghost()
                                    .label("Discard changes")
                                    .disabled(saving)
                                    .on_click(
                                        cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Closed)),
                                    ),
                            ),
                    )
                }),
        )
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
        value.notes = self.notes.read(cx).value().to_string();
        value
    }

    /// Steam facts under the description: year and reviews as text, then
    /// genre badges that open the genre's smart collection.
    fn steam_facts(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let metadata = self.base.game.steam.as_ref()?.metadata.as_ref()?;
        let mut facts: Vec<String> = metadata.release_year.iter().cloned().collect();
        match (&metadata.review_label, metadata.review_percent) {
            (Some(label), Some(percent)) => facts.push(format!("{label}, {percent}%")),
            (Some(label), None) => facts.push(label.clone()),
            (None, Some(percent)) => facts.push(format!("{percent}% positive")),
            (None, None) => {}
        }
        if facts.is_empty() && metadata.genres.is_empty() {
            return None;
        }
        Some(
            v_flex()
                .gap_2()
                .when(!facts.is_empty(), |column| {
                    column.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(facts.join(" · ")),
                    )
                })
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_1()
                        .children(metadata.genres.iter().map(|genre| {
                            let rule = gamesync_desktop::smart::SmartRule::Genre(genre.clone());
                            Button::new(gpui::SharedString::from(format!("genre-{genre}")))
                                .xsmall()
                                .outline()
                                .rounded(px(12.))
                                .label(genre.clone())
                                .tooltip(format!("Show all {genre} games"))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(EditorEvent::ShowScope(crate::model::Scope::Smart(
                                        rule.clone(),
                                    )))
                                }))
                        })),
                ),
        )
    }

    /// Wishlist price details use the same compact treatment as library views.
    fn price_block(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let library = self.library.read(cx);
        let game = library.games.iter().find(|g| g.id == self.base.game_id);
        v_flex()
            .gap_1()
            .child(hint("Price", cx))
            .child(game.map_or_else(
                || div().text_lg().child("Price loading…").into_any_element(),
                |game| super::price::wishlist_price(game, cx),
            ))
    }

    /// Picker rows for the current search. See `suggest_tags`.
    fn tag_suggestions(&self, cx: &App) -> (Vec<String>, Option<String>) {
        let library = self.library.read(cx);
        suggest_tags(
            library.games.iter().flat_map(|game| &game.tags),
            &self.personal.tags,
            &self.tag_query.read(cx).value(),
        )
    }

    fn add_tag(&mut self, tag: &str, window: &mut Window, cx: &mut Context<Self>) {
        let tag = tag.trim();
        if tag.is_empty()
            || self
                .personal
                .tags
                .iter()
                .any(|t| t.to_lowercase() == tag.to_lowercase())
        {
            return;
        }
        self.personal.tags.push(tag.to_owned());
        self.tag_query
            .update(cx, |query, cx| query.set_value("", window, cx));
        self.change_now(Field::Tags, cx);
    }

    fn remove_tag(&mut self, tag: &str, cx: &mut Context<Self>) {
        self.personal.tags.retain(|t| t != tag);
        self.change_now(Field::Tags, cx);
    }

    fn tags_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = cx.entity();
        let query = self.tag_query.clone();
        h_flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .children(self.personal.tags.iter().map(|tag| {
                let name = tag.clone();
                h_flex()
                    .id(gpui::SharedString::from(format!("tag-{tag}")))
                    .h(px(24.))
                    .pl_2()
                    .pr_0p5()
                    .gap_0p5()
                    .rounded_full()
                    .text_xs()
                    .bg(cx.theme().secondary)
                    .text_color(cx.theme().secondary_foreground)
                    .child(tag.clone())
                    .child(
                        Button::new(gpui::SharedString::from(format!("remove-tag-{tag}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip(format!("Remove {tag}"))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.remove_tag(&name, cx)),
                            ),
                    )
            }))
            .child(
                Popover::new("tag-picker")
                    .anchor(Corner::TopLeft)
                    .track_focus(&query.focus_handle(cx))
                    .trigger(
                        Button::new("add-tag")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .label("Add tag"),
                    )
                    .content(move |_, _, cx| {
                        let (matches, create) = editor.read(cx).tag_suggestions(cx);
                        let empty = matches.is_empty() && create.is_none();
                        let row =
                            |id: String,
                             label: String,
                             tag: String,
                             editor: Entity<InspectorEditor>| {
                                h_flex()
                                    .id(gpui::SharedString::from(id))
                                    .px_2()
                                    .py_1()
                                    .rounded(cx.theme().radius)
                                    .text_sm()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(cx.theme().accent))
                                    .child(label)
                                    .on_click(move |_, window, cx| {
                                        editor.update(cx, |this, cx| this.add_tag(&tag, window, cx))
                                    })
                            };
                        v_flex()
                            .w(px(240.))
                            .gap_1()
                            .child(Input::new(&query).small())
                            .children(matches.into_iter().map(|tag| {
                                row(format!("pick-{tag}"), tag.clone(), tag, editor.clone())
                            }))
                            .children(create.map(|name| {
                                row(
                                    "create-tag".into(),
                                    format!("Create “{name}”"),
                                    name,
                                    editor.clone(),
                                )
                            }))
                            .when(empty, |list| {
                                list.child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Type to create a tag."),
                                )
                            })
                    }),
            )
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
                        this.show_saved(cx);
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

/// Existing library tags that match `query`, most used first, without tags
/// the game already has. Also the query as a new tag when no tag has that
/// name. Matching ignores case.
fn suggest_tags<'a>(
    library: impl Iterator<Item = &'a String>,
    current: &[String],
    query: &str,
) -> (Vec<String>, Option<String>) {
    const LIMIT: usize = 8;
    let query = query.trim();
    let needle = query.to_lowercase();
    let on_game = |name: &str| current.iter().any(|t| t.to_lowercase() == name);
    let mut counts = std::collections::HashMap::<&String, usize>::new();
    for tag in library {
        *counts.entry(tag).or_default() += 1;
    }
    let exists = counts.keys().any(|t| t.to_lowercase() == needle);
    let mut matches: Vec<_> = counts
        .into_iter()
        .filter(|(tag, _)| {
            let lower = tag.to_lowercase();
            lower.contains(&needle) && !on_game(&lower)
        })
        .collect();
    matches.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    let create = (!query.is_empty() && !exists && !on_game(&needle)).then(|| query.to_owned());
    (
        matches
            .into_iter()
            .take(LIMIT)
            .map(|(tag, _)| tag.clone())
            .collect(),
        create,
    )
}

impl Render for InspectorEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let favorite = self.personal.favorite;
        let wishlisted = self.base.game.wishlisted();
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
                    .children(self.status_line(Field::Top, cx))
                    .when(!wishlisted, |fields| {
                        fields.child(hint(
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
                    })
                    .child(
                        div().text_sm().child(
                            self.base
                                .game
                                .description()
                                .unwrap_or("No Steam description.")
                                .to_owned(),
                        ),
                    )
                    .children(self.steam_facts(cx))
                    .children(wishlisted.then(|| self.price_block(cx)))
                    // Owned-game fields. Wishlist games are not owned, so they have
                    // no status, rating, favorite, or collections.
                    .when(!wishlisted, |fields| {
                        fields
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
                                                            this.personal.set_status(key.clone());
                                                            this.change_now(Field::Status, cx);
                                                        })
                                                    }),
                                            );
                                        }
                                        menu
                                    }),
                            )
                            .children(self.status_line(Field::Status, cx))
                            // Rating and favorite share one row: a small label above each control.
                            .child(
                                // Wraps on a narrow card so the heart never overflows.
                                h_flex()
                                    .flex_wrap()
                                    .gap_x_6()
                                    .gap_y_2()
                                    .items_start()
                                    .child(v_flex().gap_1().child(hint("Your rating", cx)).child(
                                        h_flex().children((1u8..=5).map(|stars| {
                                            let value = stars * 2;
                                            // Keep old half-star values visible; new choices use whole stars.
                                            let fill = rating
                                                .unwrap_or(0)
                                                .saturating_sub(value - 2)
                                                .min(2);
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
                                                        .h(px(24.))
                                                        .child(
                                                            Icon::new(IconName::Star).size(px(24.)),
                                                        )
                                                        .child(
                                                            div()
                                                                .absolute()
                                                                .top_0()
                                                                .left_0()
                                                                .overflow_hidden()
                                                                .w(px(f32::from(fill) * 12.))
                                                                .h_full()
                                                                .child(
                                                                    Icon::new(
                                                                        crate::assets::RatingIcon,
                                                                    )
                                                                    .size(px(24.)),
                                                                ),
                                                        ),
                                                )
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.personal.rating = (this.personal.rating
                                                        != Some(value))
                                                    .then_some(value);
                                                    this.change_now(Field::Rating, cx);
                                                }))
                                        })),
                                    ))
                                    .child(
                                        v_flex().gap_1().child(hint("Favorite", cx)).child(
                                            Button::new("edit-favorite")
                                                .ghost()
                                                .w(px(36.))
                                                .h(px(36.))
                                                .p_0()
                                                .tooltip(if favorite {
                                                    "Remove from favorites"
                                                } else {
                                                    "Add to favorites"
                                                })
                                                .child(
                                                    if favorite {
                                                        Icon::new(crate::assets::FavoriteIcon)
                                                    } else {
                                                        Icon::new(IconName::Heart)
                                                    }
                                                    .size(px(24.)),
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.personal.favorite =
                                                        !this.personal.favorite;
                                                    this.change_now(Field::Rating, cx);
                                                })),
                                        ),
                                    ),
                            )
                            .children(self.status_line(Field::Rating, cx))
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
                                            let member = self.personal.collections.contains(&id);
                                            // Filled with a check when the game is in it; outlined
                                            // and muted otherwise, so both states read at a glance.
                                            Button::new(gpui::SharedString::from(format!(
                                                "member-{id}"
                                            )))
                                            .small()
                                            .label(collection.name)
                                            .when(member, |chip| {
                                                chip.primary().icon(IconName::Check)
                                            })
                                            .when(!member, |chip| {
                                                chip.outline()
                                                    .text_color(cx.theme().muted_foreground)
                                            })
                                            .tooltip(if member {
                                                "In this collection. Click to remove."
                                            } else {
                                                "Click to add to this collection."
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if this.personal.collections.contains(&id) {
                                                    this.personal.collections.retain(|v| *v != id);
                                                } else {
                                                    this.personal.collections.push(id);
                                                }
                                                this.change_now(Field::Collections, cx);
                                            }))
                                        }),
                                ),
                            )
                            .children(self.status_line(Field::Collections, cx))
                    })
                    .child(hint("Tags", cx))
                    .child(self.tags_row(cx))
                    .children(self.status_line(Field::Tags, cx))
                    .child(hint("Notes", cx))
                    .child(
                        // Notes fill the rest of the card, so the card ends with the
                        // last field instead of empty space.
                        Input::new(&self.notes)
                            .flex_1()
                            .min_h(px(88.))
                            .flex_shrink_0()
                            .small()
                            .disabled(false),
                    )
                    .children(self.status_line(Field::Notes, cx)),
            )
    }
}

fn hint(text: &str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::suggest_tags;

    #[test]
    fn tag_suggestions_rank_by_use_and_offer_new_tags_once() {
        let library: Vec<String> = ["Cozy", "Cozy", "Co-op", "Puzzle"]
            .map(String::from)
            .to_vec();
        let current = vec!["Puzzle".to_string()];
        let (matches, create) = suggest_tags(library.iter(), &current, "co");
        assert_eq!(matches, ["Cozy", "Co-op"]);
        assert_eq!(create.as_deref(), Some("co"));
        // An existing tag in another case is suggested, not created again.
        let (matches, create) = suggest_tags(library.iter(), &current, "cozy");
        assert_eq!((matches, create), (vec!["Cozy".to_string()], None));
        // A tag the game already has is neither suggested nor created.
        assert_eq!(
            suggest_tags(library.iter(), &current, "PUZZLE"),
            (Vec::new(), None)
        );
        // An empty search lists tags without offering to create one.
        assert_eq!(suggest_tags(library.iter(), &current, " ").1, None);
    }
}
