//! Explicit whole-version review. The shared record service retains every branch.
use super::editor::EditorEvent;
use crate::model::Library;
use gamesync_desktop::{
    library::{LibraryRevision, LibraryStore},
    record_store::{RecordSnapshot, RecordStore},
    records::GameRevision,
};
use gpui::{div, prelude::*, App, Entity, EventEmitter, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Selectable as _, StyledExt as _,
};
use std::path::PathBuf;
use uuid::Uuid;

pub struct ConflictReview {
    library: Entity<Library>,
    root: PathBuf,
    manifest: LibraryRevision,
    game_id: Uuid,
    snapshot: Option<RecordSnapshot>,
    chosen: Option<Uuid>,
    busy: bool,
    message: String,
}
impl EventEmitter<EditorEvent> for ConflictReview {}

impl ConflictReview {
    pub fn new(
        library: Entity<Library>,
        root: PathBuf,
        manifest: LibraryRevision,
        game_id: Uuid,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        let mut review = Self {
            library,
            root,
            manifest,
            game_id,
            snapshot: None,
            chosen: None,
            busy: false,
            message: String::new(),
        };
        review.reload(cx);
        review
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.snapshot = None;
        self.chosen = None;
        self.message = "Reading saved versions…".into();
        let root = self.root.clone();
        let id = self.game_id;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { RecordStore::open(root)?.inspect(id) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(snapshot) => {
                        this.message = if !snapshot.issues.is_empty() {
                            format!("Files need attention: {}", snapshot.issues.join("; "))
                        } else if !snapshot.has_conflict() {
                            "No conflicting saved versions. Close to return to your details.".into()
                        } else {
                            "Select one complete version to keep. All alternatives stay in history."
                                .into()
                        };
                        this.snapshot = Some(snapshot);
                    }
                    Err(error) => this.message = format!("Could not read versions: {error:#}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn same_library(&self, cx: &App) -> bool {
        self.library.read(cx).source.as_ref() == Some(&(self.root.clone(), self.manifest.clone()))
    }

    fn resolve(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.same_library(cx) {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if !snapshot.issues.is_empty() || !snapshot.has_conflict() {
            return;
        }
        let Some(chosen) = self.chosen else {
            return;
        };
        let expected = snapshot.heads.clone();
        let root = self.root.clone();
        let manifest = self.manifest.clone();
        let id = self.game_id;
        self.busy = true;
        self.message = "Saving your choice…".into();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                LibraryStore::open(root)?.resolve_game(&manifest, id, &expected, chosen)
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                // Require a fresh review after any attempt, including a late sync conflict.
                this.chosen = None;
                this.snapshot = None;
                this.message = match result {
                    Ok(_) => { cx.emit(EditorEvent::Saved); "Choice saved. Other versions remain in history. Close to return to your details.".into() }
                    Err(error) => format!("Choice was not confirmed: {error:#}. Read versions again before choosing."),
                };
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}

impl Render for ConflictReview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let same = self.same_library(cx);
        let can_resolve = same
            && !self.busy
            && self.chosen.is_some()
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.issues.is_empty() && s.has_conflict());
        let records: Vec<_> = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .heads
                    .iter()
                    .filter_map(|id| snapshot.revisions.get(id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let fields: Vec<_> = records
            .iter()
            .map(|record| version_fields(record, &self.manifest))
            .collect();
        let differing: std::collections::BTreeSet<_> = fields
            .iter()
            .flatten()
            .filter_map(|(name, value)| {
                fields
                    .iter()
                    .any(|other| {
                        other.iter().find(|(key, _)| key == name).map(|(_, v)| v) != Some(value)
                    })
                    .then_some(name.clone())
            })
            .collect();
        let conflicts = self.library.read(cx).conflicts.clone();
        v_flex().size_full()
            .child(v_flex().id("conflict-scroll").flex_1().min_h_0().overflow_y_scroll().p_4().gap_3()
                .child(div().text_lg().font_semibold().child("Review versions"))
                .child(div().text_xs().child("Only differences are shown. Keep one complete saved version. Your unsaved draft stays intact."))
                .children(conflicts.into_iter().map(|(id, title)| {
                    Button::new(gpui::SharedString::from(format!("conflict-game-{id}"))).label(title).selected(id == self.game_id)
                        .disabled(self.busy || !same).on_click(cx.listener(move |this, _, _, cx| {
                            this.game_id = id; this.reload(cx);
                        }))
                }))
                .children(records.iter().enumerate().map(|(index, record)| {
                    let id = record.revision_id;
                    v_flex().flex_shrink_0().gap_2().p_3().border_1().border_color(cx.theme().border)
                        .child(Button::new(gpui::SharedString::from(format!("choose-version-{id}")))
                            .label(format!("Version {}", index + 1)).selected(self.chosen == Some(id))
                            .disabled(self.busy || !same).on_click(cx.listener(move |this, _, _, cx| {
                                this.chosen = Some(id); cx.notify();
                            })))
                        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!("ID {}", &id.to_string()[..8])))
                        .children(fields[index].iter().filter(|(name, _)| differing.contains(name)).cloned().map(|(name, value)| {
                            v_flex().flex_shrink_0().gap_1()
                                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(name))
                                .child(div().text_sm().child(if value.is_empty() { "Empty".into() } else { value }))
                        }))
                })))
            .child(v_flex().flex_shrink_0().p_3().gap_2().border_t_1().border_color(cx.theme().border)
                .child(div().text_xs().child(if same { self.message.clone() } else {
                    "Library definitions changed. Close this review and open it again.".into()
                }))
                .child(Button::new("keep-version").primary().label("Keep selected version").disabled(!can_resolve)
                    .on_click(cx.listener(|this, _, _, cx| this.resolve(cx))))
                .child(h_flex().gap_2()
                    .child(Button::new("reload-versions").label("Read again").disabled(self.busy || !same)
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx))))
                    .child(Button::new("close-review").label("Close").disabled(self.busy)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Closed))))))
    }
}

// Show all fields affected by whole-version selection, including archive state
// and unfamiliar properties. Never imply that only personal values are selected.
fn version_fields(record: &GameRevision, manifest: &LibraryRevision) -> Vec<(String, String)> {
    let game = &record.game;
    let personal = &game.personal;
    let mut fields = vec![
        ("Title".into(), game.title.clone()),
        (
            "In library".into(),
            if record.deleted {
                "Archived"
            } else {
                "Visible"
            }
            .into(),
        ),
        (
            "Status".into(),
            manifest
                .definitions
                .status(&personal.status)
                .map(|s| s.label.clone())
                .unwrap_or(personal.status.clone()),
        ),
        (
            "Rating".into(),
            personal
                .rating
                .map(|r| format!("{:.1} / 5", r as f32 / 2.))
                .unwrap_or("Unrated".into()),
        ),
        (
            "Favorite".into(),
            if personal.favorite { "Yes" } else { "No" }.into(),
        ),
        (
            "Hidden".into(),
            if personal.hidden { "Yes" } else { "No" }.into(),
        ),
        ("Collections".into(), format!("{:?}", personal.collections)),
        ("Tags".into(), format!("{:?}", personal.tags)),
        ("Notes".into(), personal.notes.clone()),
        (
            "Description source".into(),
            if personal.description.is_some() {
                "Personal"
            } else {
                "Steam"
            }
            .into(),
        ),
        (
            "Personal description".into(),
            personal.description.clone().unwrap_or_default(),
        ),
        (
            "Cover".into(),
            personal.cover.clone().unwrap_or("No override".into()),
        ),
    ];
    if let Some(steam) = &game.steam {
        fields.push((
            "Steam".into(),
            format!(
                "App {} · {} minutes · {}",
                steam.app_id,
                steam.playtime_minutes,
                if steam.owned { "Owned" } else { "Not owned" }
            ),
        ));
        fields.push((
            "Steam description".into(),
            steam.description.clone().unwrap_or_default(),
        ));
        if let Some(metadata) = &steam.metadata {
            fields.push(("Steam metadata".into(), format!("{metadata:#?}")));
        }
        if !steam.extra.is_empty() {
            fields.push((
                "Other Steam properties".into(),
                format!("{:#?}", steam.extra),
            ));
        }
    } else {
        fields.push(("Steam".into(), "Not linked".into()));
    }
    for (label, extra) in [
        ("Other personal properties", &personal.extra),
        ("Other game properties", &game.extra),
        ("Other record properties", &record.extra),
    ] {
        if !extra.is_empty() {
            fields.push((label.into(), format!("{extra:#?}")));
        }
    }
    fields
}
