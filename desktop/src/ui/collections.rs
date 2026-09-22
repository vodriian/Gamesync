//! Collection definitions use the same guarded revision writes as library statuses.
use gamesync_desktop::library::{CollectionDefinition, LibrarySnapshot, LibraryStore};
use gpui::{div, prelude::*, Entity, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    v_flex, ActiveTheme as _, Disableable as _, StyledExt as _,
};
use std::path::PathBuf;
use uuid::Uuid;

pub struct Collections {
    root: PathBuf,
    snapshot: Option<LibrarySnapshot>,
    name: Entity<InputState>,
    selected: Option<Uuid>,
    busy: bool,
    message: String,
    clear_name: bool,
}
impl Collections {
    pub fn new(root: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = Self {
            root,
            snapshot: None,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("Collection name")),
            selected: None,
            busy: false,
            message: String::new(),
            clear_name: false,
        };
        // Start only after GPUI has registered the new view and its window.
        let entity = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = entity.update(cx, |view, cx| view.read(cx));
        });
        view
    }
    pub fn busy(&self) -> bool {
        self.busy
    }
    fn read(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let root = self.root.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { LibraryStore::open(root)?.inspect() })
                .await;
            let applied = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(snapshot) => {
                        this.message = snapshot.issues.join("; ");
                        this.snapshot = Some(snapshot);
                    }
                    Err(e) => this.message = e.to_string(),
                }
                cx.notify();
            });
            if let Err(error) = applied {
                log::warn!("Collection view update failed: {error}");
            }
        })
        .detach();
    }
    fn save(&mut self, archive: bool, choice: Option<Uuid>, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(base) = choice
            .and_then(|id| snapshot.revisions.get(&id))
            .or_else(|| snapshot.current())
        else {
            return;
        };
        let mut definitions = base.definitions.clone();
        if choice.is_some() {
            // Keep IDs from the other branch as archived definitions, so membership is recoverable.
            for version in snapshot.revisions.values() {
                for collection in &version.definitions.collections {
                    if !definitions
                        .collections
                        .iter()
                        .any(|c| c.id == collection.id)
                    {
                        let mut collection = collection.clone();
                        collection.archived = true;
                        definitions.collections.push(collection);
                    }
                }
                for status in &version.definitions.statuses {
                    if definitions.status(&status.key).is_none() {
                        definitions.statuses.push(status.clone());
                    }
                }
            }
        } else if let Some(id) = self.selected {
            let Some(collection) = definitions.collections.iter_mut().find(|c| c.id == id) else {
                return;
            };
            if archive {
                collection.archived = true;
            } else {
                collection.name = self.name.read(cx).value().trim().to_owned();
            }
        } else {
            definitions.collections.push(CollectionDefinition {
                id: Uuid::new_v4(),
                name: self.name.read(cx).value().trim().to_owned(),
                archived: false,
                extra: Default::default(),
            });
        }
        if let Err(error) = definitions.validate() {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        let root = self.root.clone();
        let expected = base.revision_id;
        let heads = snapshot.heads.clone();
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let store = LibraryStore::open(root)?;
                    if let Some(chosen) = choice {
                        store.resolve(&heads, chosen, definitions)
                    } else {
                        store.edit(expected, definitions)
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(_) => {
                        this.selected = None;
                        this.clear_name = true;
                        this.read(cx);
                    }
                    Err(e) => this.message = format!("Not saved: {e}. Read again before retrying."),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for Collections {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.clear_name {
            self.clear_name = false;
            self.name
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        let current = self.snapshot.as_ref().and_then(|s| s.current()).cloned();
        let alternatives: Vec<_> = self
            .snapshot
            .as_ref()
            .filter(|s| s.has_conflict())
            .map(|s| {
                s.heads
                    .iter()
                    .filter_map(|id| s.revisions.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default();
        v_flex().id("collections-window").size_full().overflow_y_scroll().p_6().gap_4().bg(cx.theme().background).text_color(cx.theme().foreground)
            .child(gpui_component::TitleBar::new().border_b_0())
            .child(div().text_xl().font_semibold().child("Collections"))
            .child("Group games your way. Removing a collection keeps its games.")
            .children(current.as_ref().into_iter().flat_map(|m| m.definitions.collections.clone()).filter(|c| !c.archived).map(|collection| {
                Button::new(gpui::SharedString::from(collection.id.to_string())).label(collection.name.clone()).disabled(self.busy).on_click(cx.listener(move |this, _, window, cx| {
                    this.selected = Some(collection.id); this.name.update(cx, |input, cx| input.set_value(collection.name.clone(), window, cx)); cx.notify();
                }))
            }))
            .when(current.is_some(), |view| view.child(Input::new(&self.name).disabled(self.busy)).child(h_flex().gap_2()
                .child(Button::new("save-collection").primary().label(if self.selected.is_some() { "Rename" } else { "Create" }).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.save(false, None, cx))))
                .child(Button::new("new-collection").label("New").disabled(self.busy).on_click(cx.listener(|this, _, window, cx| { this.selected = None; this.name.update(cx, |i, cx| i.set_value("", window, cx)); cx.notify(); })))
                .child(Button::new("remove-collection").label("Remove").disabled(self.busy || self.selected.is_none()).on_click(cx.listener(|this, _, _, cx| this.save(true, None, cx))))))
            .when(!alternatives.is_empty(), |v| v.child("Library definitions conflict. Keep one version below. Collections absent from that version become archived; other status keys remain available."))
            .children(alternatives.into_iter().map(|version| {
                let id = version.revision_id;
                v_flex().p_4().gap_2().rounded_lg().bg(cx.theme().secondary)
                    .child(format!("Version {}", &id.to_string()[..8]))
                    .child(format!("Library: {}", version.definitions.name))
                    .child(format!("Collections: {}", version.definitions.collections.iter().map(|c| format!("{}{}", c.name, if c.archived { " (removed)" } else { "" })).collect::<Vec<_>>().join(" · ")))
                    .child(format!("Statuses: {}", version.definitions.statuses.iter().map(|s| s.label.clone()).collect::<Vec<_>>().join(" · ")))
                    .child(Button::new(gpui::SharedString::from(format!("keep-{id}"))).label("Keep this version").disabled(self.busy).on_click(cx.listener(move |this, _, _, cx| this.save(false, Some(id), cx))))
            }))
            .child(self.message.clone())
            .child(Button::new("read-collections").label("Read again").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.read(cx))))
    }
}
