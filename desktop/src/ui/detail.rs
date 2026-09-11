//! Eagle's inspector layout adapted to game metadata.

use super::editor::{EditorEvent, InspectorEditor};
use crate::{model::Library, ui::thumb_cache::LruImageCache};
use gpui::{div, image_cache, img, prelude::*, px, App, Entity, ObjectFit, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, IconName, StyledExt as _,
};

pub struct DetailPanel {
    library: Entity<Library>,
    cache: Entity<LruImageCache>,
    editor: Option<Entity<InspectorEditor>>,
}
impl gpui::EventEmitter<EditorEvent> for DetailPanel {}

impl DetailPanel {
    pub fn new(
        library: Entity<Library>,
        cache: Entity<LruImageCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        Self {
            library,
            cache,
            editor: None,
        }
    }
}

impl DetailPanel {
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.busy(cx) {
            self.editor = None;
            cx.notify();
        }
    }

    pub fn busy(&self, cx: &App) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).busy(cx))
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let library = self.library.read(cx);
        let Some((root, manifest)) = library.source.clone() else {
            return;
        };
        let Some(base) = library.selected_game().and_then(|game| game.record.clone()) else {
            return;
        };
        let editor = cx
            .new(|cx| InspectorEditor::new(self.library.clone(), root, manifest, base, window, cx));
        cx.subscribe(&editor, |this, _, event, cx| {
            match event {
                EditorEvent::Closed => this.editor = None,
                EditorEvent::Saved => cx.emit(EditorEvent::Saved),
            }
            cx.notify();
        })
        .detach();
        self.editor = Some(editor);
        cx.notify();
    }
}

impl Render for DetailPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = v_flex()
            .id("detail")
            .w(px(320.))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar);
        if let Some(editor) = &self.editor {
            return panel.child(editor.clone()).into_any_element();
        }
        let Some(game) = self.library.read(cx).selected_game().cloned() else {
            return panel
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::PanelRight))
                .child(div().text_sm().child("Select a game"))
                .child(div().text_xs().child("Its details will appear here."))
                .into_any_element();
        };

        panel
            .child(
                image_cache(self.cache.clone())
                    .w_full()
                    .h(px(240.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .size_full()
                            .p_3()
                            .flex()
                            .justify_center()
                            .bg(cx.theme().secondary)
                            .child(
                                img(game
                                    .cover_path
                                    .clone()
                                    .map(gpui::ImageSource::from)
                                    .unwrap_or_else(|| game.cover.clone().into()))
                                .h_full()
                                .object_fit(ObjectFit::Contain)
                                .with_fallback(|| {
                                    div().p_4().child("Cover unavailable").into_any_element()
                                }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .id("detail-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .gap_4()
                    .child(div().text_lg().font_semibold().child(game.title.clone()))
                    .child(
                        h_flex()
                            .gap_4()
                            .child(field("Status", game.status_label.clone(), cx))
                            .child(field("Your rating", game.rating_label(), cx)),
                    )
                    .child(field(
                        "Time played",
                        format!("{:.1} hours", game.playtime_minutes as f32 / 60.),
                        cx,
                    ))
                    .when(game.favorite, |column| {
                        column.child(field("Favorite", "Yes", cx))
                    })
                    .child(field("About", game.description.clone(), cx))
                    .when(
                        game.record
                            .as_ref()
                            .is_some_and(|record| !record.game.personal.notes.is_empty()),
                        |column| {
                            column.child(field(
                                "Notes",
                                game.record
                                    .as_ref()
                                    .map(|record| record.game.personal.notes.clone())
                                    .unwrap_or_default(),
                                cx,
                            ))
                        },
                    )
                    .child(
                        v_flex().gap_2().child(label("Tags", cx)).child(
                            h_flex()
                                .flex_wrap()
                                .gap_1()
                                .children(game.tags.iter().map(|tag| {
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().secondary)
                                        .text_xs()
                                        .child(tag.clone())
                                })),
                        ),
                    )
                    .when(!self.library.read(cx).demo, |column| {
                        column.child(
                            Button::new("edit-details")
                                .primary()
                                .label("Edit details")
                                .on_click(cx.listener(|this, _, window, cx| this.edit(window, cx))),
                        )
                    })
                    .when(self.library.read(cx).demo, |column| {
                        column.child(div().text_xs().child("Open a library to edit details."))
                    }),
            )
            .into_any_element()
    }
}

fn label(text: &str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_owned())
}

fn field(name: &str, value: impl Into<String>, cx: &App) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(label(name, cx))
        .child(div().text_sm().child(value.into()))
}
