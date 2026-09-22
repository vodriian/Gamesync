//! Collection definitions use the same guarded revision writes as library statuses.
use gamesync_desktop::library::{LibrarySnapshot, LibraryStore};
use gpui::{div, prelude::*, Window};
use gpui_component::{button::Button, v_flex, ActiveTheme as _, Disableable as _, StyledExt as _};
use std::path::PathBuf;
use uuid::Uuid;

pub struct Collections {
    root: PathBuf,
    snapshot: Option<LibrarySnapshot>,
    busy: bool,
    message: String,
}
impl Collections {
    pub fn new(root: PathBuf, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = Self {
            root,
            snapshot: None,
            busy: false,
            message: String::new(),
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
    fn save(&mut self, choice: Uuid, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(base) = snapshot
            .revisions
            .get(&choice)
            .or_else(|| snapshot.current())
        else {
            return;
        };
        let mut definitions = base.definitions.clone();
        {
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
        }
        if let Err(error) = definitions.validate() {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        let root = self.root.clone();
        let heads = snapshot.heads.clone();
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let store = LibraryStore::open(root)?;
                    store.resolve(&heads, choice, definitions)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(_) => {
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(div().text_lg().font_semibold().child("Review collections"))
            .when(alternatives.is_empty(), |v| v.child("No collection conflicts. Use the sidebar to create or rename a collection."))
            .when(!alternatives.is_empty(), |v| v.child("Library definitions conflict. Keep one version below. Collections absent from that version become archived; other status keys remain available."))
            .children(alternatives.into_iter().map(|version| {
                let id = version.revision_id;
                v_flex().p_4().gap_2().rounded_lg().bg(cx.theme().secondary)
                    .child(format!("Version {}", &id.to_string()[..8]))
                    .child(format!("Library: {}", version.definitions.name))
                    .child(format!("Collections: {}", version.definitions.collections.iter().map(|c| format!("{}{}", c.name, if c.archived { " (removed)" } else { "" })).collect::<Vec<_>>().join(" · ")))
                    .child(format!("Statuses: {}", version.definitions.statuses.iter().map(|s| s.label.clone()).collect::<Vec<_>>().join(" · ")))
                    .child(Button::new(gpui::SharedString::from(format!("keep-{id}"))).label("Keep this version").disabled(self.busy).on_click(cx.listener(move |this, _, _, cx| this.save(id, cx))))
            }))
            .child(self.message.clone())
            .child(Button::new("read-collections").label("Read again").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.read(cx))))
    }
}
