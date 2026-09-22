//! Focused card front/back. The existing editor owns drafts and guarded saves.

use super::conflicts::ConflictReview;
use super::editor::{EditorEvent, InspectorEditor};
use crate::{model::Library, ui::thumb_cache::LruImageCache};
use gpui::{
    canvas, div, image_cache, prelude::*, px, App, Bounds, Entity, FocusHandle, Pixels, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
};

pub struct DetailPanel {
    focus: FocusHandle,
    filmstrip: Entity<super::filmstrip::Filmstrip>,
    strip_shown: bool,
    strip_motion: super::motion::Motion,
    // Keep the open card when an edit removes it from the current library filter.
    viewed: Option<uuid::Uuid>,
    take_focus: bool,
    back: bool,
    turn: super::card_motion::Spring,
    pitch: super::card_motion::Spring,
    yaw: super::card_motion::Spring,
    edit_focus: Option<FocusHandle>,
    restore_edit_focus: bool,
    bounds: Bounds<Pixels>,
    viewport: gpui::Size<Pixels>,
    library: Entity<Library>,
    cache: Entity<LruImageCache>,
    editor: Option<Entity<InspectorEditor>>,
    review: Option<Entity<ConflictReview>>,
    cover_refresh: Option<uuid::Uuid>,
    cover_notice: Option<(uuid::Uuid, String)>,
}
impl gpui::EventEmitter<EditorEvent> for DetailPanel {}

impl DetailPanel {
    pub fn new(
        library: Entity<Library>,
        cache: Entity<LruImageCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        let filmstrip =
            cx.new(|cx| super::filmstrip::Filmstrip::new(library.clone(), cache.clone(), cx));
        cx.subscribe(
            &filmstrip,
            |this, _, event: &super::filmstrip::SelectGame, cx| {
                if this.busy(cx) {
                    return;
                }
                this.library.update(cx, |library, cx| {
                    library.select_slot(event.0);
                    cx.notify();
                });
                this.present(cx);
            },
        )
        .detach();
        Self {
            filmstrip,
            strip_shown: true,
            strip_motion: super::motion::Motion::new(1.),
            focus: cx.focus_handle(),
            viewed: None,
            take_focus: true,
            back: false,
            turn: super::card_motion::Spring::new(0.),
            pitch: super::card_motion::Spring::new(0.),
            yaw: super::card_motion::Spring::new(0.),
            edit_focus: None,
            restore_edit_focus: false,
            bounds: Bounds::default(),
            viewport: gpui::Size::default(),
            library,
            cache,
            editor: None,
            review: None,
            cover_refresh: None,
            cover_notice: None,
        }
    }
}

impl DetailPanel {
    fn toggle_strip(&mut self, cx: &mut Context<Self>) {
        self.strip_shown = !self.strip_shown;
        self.strip_motion
            .set(if self.strip_shown { 1. } else { 0. }, cx);
    }

    pub fn present(&mut self, cx: &mut Context<Self>) {
        // A failed or queued draft must remain attached to its own game.
        if let Some(editor) = &self.editor {
            if editor.read(cx).busy(cx) {
                let id = editor.read(cx).game_id();
                self.library.update(cx, |library, cx| {
                    library.selected = Some(id);
                    cx.notify();
                });
            }
        }
        self.edit_focus = None;
        self.restore_edit_focus = false;
        self.viewed = self.library.read(cx).selected_game().map(|game| game.id);
        self.back = self.busy(cx);
        self.turn =
            super::card_motion::Spring::new(if self.back { std::f32::consts::PI } else { 0. });
        self.pitch.set(0.);
        self.yaw.set(0.);
        self.take_focus = true;
        cx.notify();
    }

    fn flip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.back {
            self.edit_focus = window.focused(cx).filter(|focus| focus != &self.focus);
            self.restore_edit_focus = false;
        } else {
            self.restore_edit_focus = self.edit_focus.is_some();
        }
        window.focus(&self.focus);
        self.back = !self.back;
        let target = if self.back { std::f32::consts::PI } else { 0. };
        if super::motion::reduced(cx) {
            self.turn = super::card_motion::Spring::new(target);
        } else {
            self.turn.set(target);
        }
        self.pitch.set(0.);
        self.yaw.set(0.);
        cx.notify();
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        self.library.update(cx, |lib, cx| {
            lib.step_selection(delta);
            cx.notify();
        });
        self.edit_focus = None;
        self.restore_edit_focus = false;
        self.viewed = self.library.read(cx).selected_game().map(|game| game.id);
        self.pitch.set(0.);
        self.yaw.set(0.);
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.busy(cx) {
            self.editor = None;
            self.review = None;
            self.viewed = None;
            cx.notify();
        }
    }

    pub fn busy(&self, cx: &App) -> bool {
        self.cover_refresh.is_some()
            || self
                .review
                .as_ref()
                .is_some_and(|review| review.read(cx).busy())
            || self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.read(cx).busy(cx))
    }

    pub fn review_conflicts(&mut self, cx: &mut Context<Self>) {
        if self
            .review
            .as_ref()
            .is_some_and(|review| review.read(cx).busy())
        {
            return;
        }
        let library = self.library.read(cx);
        let Some((root, manifest)) = library.source.clone() else {
            return;
        };
        let Some(id) = library
            .selected
            .filter(|id| library.conflicts.contains_key(id))
            .or_else(|| library.conflicts.keys().next().copied())
        else {
            return;
        };
        let review = cx.new(|cx| ConflictReview::new(self.library.clone(), root, manifest, id, cx));
        cx.subscribe(&review, |this, _, event, cx| {
            match event {
                EditorEvent::Closed => this.review = None,
                EditorEvent::Saved => cx.emit(EditorEvent::Saved),
            }
            cx.notify();
        })
        .detach();
        self.viewed = Some(id);
        self.back = true;
        self.turn = super::card_motion::Spring::new(std::f32::consts::PI);
        self.take_focus = true;
        self.review = Some(review);
        cx.notify();
    }

    fn refresh_cover(&mut self, cx: &mut Context<Self>) {
        if self.cover_refresh.is_some() {
            return;
        }
        let library = self.library.read(cx);
        let Some((root, _)) = library.source.clone() else {
            return;
        };
        let Some(id) = self.viewed else {
            return;
        };
        self.cover_refresh = Some(id);
        self.cover_notice = Some((id, "Getting cover from Steam…".into()));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(
                    async move { gamesync_desktop::steam::covers::refresh(&root, id) },
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                this.cover_refresh = None;
                this.cover_notice = Some((
                    id,
                    match result {
                        Ok(record) => {
                            cx.emit(EditorEvent::Saved);
                            if record.game.personal.cover.is_some() {
                                "Steam cover refreshed. Your personal cover remains in use.".into()
                            } else {
                                "Cover refreshed.".into()
                            }
                        }
                        Err(error) => format!("Cover not changed: {error}"),
                    },
                ));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let library = self.library.read(cx);
        let Some((root, manifest)) = library.source.clone() else {
            return;
        };
        let Some(base) = library
            .games
            .iter()
            .find(|game| Some(game.id) == self.viewed)
            .and_then(|game| game.record.clone())
        else {
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.take_focus {
            self.take_focus = false;
            window.focus(&self.focus);
        }
        let game = self
            .library
            .read(cx)
            .games
            .iter()
            .find(|game| Some(game.id) == self.viewed)
            .cloned();
        if let Some(game) = &game {
            let replace_editor = self.editor.as_ref().is_none_or(|e| {
                let editor = e.read(cx);
                (editor.game_id() != game.id && !editor.busy(cx))
                    || game
                        .record
                        .as_ref()
                        .zip(self.library.read(cx).source.as_ref())
                        .is_some_and(|(record, (_, manifest))| {
                            editor.needs_reload(record, manifest, cx)
                        })
            });
            if replace_editor {
                self.editor = None;
                self.edit(window, cx);
            }
        }
        let busy = self.busy(cx);
        let strip_progress = if super::motion::reduced(cx) {
            if self.strip_shown {
                1.
            } else {
                0.
            }
        } else {
            self.strip_motion.value()
        };
        let available_height = if self.viewport.height > px(0.) {
            self.viewport.height
        } else {
            window.viewport_size().height - px(50.)
        };
        let width = ((f32::from(available_height) - 270.) / 1.46).clamp(180., 460.);
        let reduced = super::motion::reduced(cx) || !cfg!(target_os = "macos");
        let turning = !reduced && self.turn.active();
        let turn = if reduced {
            if self.back {
                std::f32::consts::PI
            } else {
                0.
            }
        } else {
            self.turn.value()
        };
        let show_back = turn > std::f32::consts::FRAC_PI_2;
        if show_back && !turning && self.restore_edit_focus {
            self.restore_edit_focus = false;
            if let Some(focus) = &self.edit_focus {
                window.focus(focus);
            }
        }
        if window.is_window_active()
            && !reduced
            && (turning || self.pitch.active() || self.yaw.active())
        {
            window.request_animation_frame();
        }
        let entity = cx.entity();
        let pointer_entity = entity.downgrade();
        let mut body = v_flex()
            .w(px(width))
            .h(px(width * 1.46))
            .flex_shrink_0()
            .rounded(px(17.))
            .overflow_hidden()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background);
        if let Some(review) = &self.review {
            body = body.child(review.clone());
        } else {
            body = body.child(
                h_flex()
                    .px_4()
                    .pt_4()
                    .pb_2()
                    .justify_between()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("GameSync / Card details")
                    .child("02"),
            );
            if let Some(editor) = &self.editor {
                body = body.child(div().flex_1().min_h_0().child(editor.clone()));
            } else if let Some(game) = &game {
                body = body.child(
                    v_flex()
                        .id("demo-card-back")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_4()
                        .gap_4()
                        .child(
                            div()
                                .font_family("Georgia")
                                .text_2xl()
                                .child(game.title.clone()),
                        )
                        .child(div().text_sm().child(game.status_label.clone()))
                        .child(div().text_sm().child(format!(
                            "{}  ·  {:.1} hours played",
                            game.rating_label(),
                            game.playtime_minutes as f32 / 60.
                        )))
                        .child(div().text_sm().child(game.description.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Preview card. Changes are unavailable in this preview."),
                        ),
                );
            }
            if game
                .as_ref()
                .is_some_and(|g| g.record.as_ref().is_some_and(|r| r.game.steam.is_some()))
            {
                body = body.child(
                    Button::new("refresh-cover")
                        .ghost()
                        .small()
                        .label(if self.cover_refresh.is_some() {
                            "Refreshing cover…"
                        } else {
                            "Refresh cover from Steam"
                        })
                        .disabled(self.cover_refresh.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_cover(cx))),
                );
            }
            if let Some((id, notice)) = &self.cover_notice {
                if game.as_ref().is_some_and(|g| g.id == *id) {
                    body = body.child(div().p_3().text_xs().child(notice.clone()));
                }
            }
        }
        let dimensions = gpui::size(px(width), px(width * 1.46));
        let pose = gpui::CardPose {
            pitch: if reduced { 0. } else { self.pitch.value() },
            yaw: turn + if reduced { 0. } else { self.yaw.value() },
            back: false,
            material: true,
            frosted_top: 0.,
        };
        let face = if show_back && !turning {
            div()
                .w(dimensions.width)
                .h(dimensions.height)
                .rounded(px(17.))
                .shadow(super::card::card_shadow(true))
                .child(body)
                .into_any_element()
        } else {
            let front = game
                .as_ref()
                .map(|game| super::card::front(game, width, None, false, cx));
            let front = gpui::card_layer(
                game.as_ref()
                    .map_or(0, |g| super::card::surface_id(g.id, 1)),
                dimensions,
                pose,
                px(17.),
                div().children(front),
            );
            if turning {
                div()
                    .relative()
                    .size_full()
                    .child(front)
                    .child(
                        div().absolute().inset_0().child(gpui::card_layer(
                            game.as_ref()
                                .map_or(0, |g| super::card::surface_id(g.id, 2)),
                            dimensions,
                            gpui::CardPose { back: true, ..pose },
                            px(17.),
                            body,
                        )),
                    )
                    .into_any_element()
            } else {
                front
            }
        };
        let viewport_target = cx.entity();
        v_flex()
            .relative()
            .child(canvas(move |bounds, _, cx| {
                viewport_target.update(cx, |this, cx| {
                    if this.viewport != bounds.size { this.viewport = bounds.size; cx.notify(); }
                });
            }, |_, _, _, _| {}).absolute().inset_0())
            .id("card-viewer")
            .occlude()
            .track_focus(&self.focus)
            .size_full()
            .bg(super::card::tabletop(cx))
            .text_color(cx.theme().foreground)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(EditorEvent::Closed);
                } else if this.focus.is_focused(window) {
                    match event.keystroke.key.as_str() {
                        "space" => this.flip(window, cx),
                        "t" => this.toggle_strip(cx),
                        "left" => this.step(-1, cx),
                        "right" => this.step(1, cx),
                        _ if this.turn.active() => cx.stop_propagation(),
                        _ => cx.propagate(),
                    }
                } else if this.turn.active() {
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .child(
                h_flex()
                    .h(px(66.))
                    .px_6()
                    .gap_2()
                    .flex_shrink_0()
                    .child(
                        Button::new("close-card")
                            .ghost()
                            .label("Back to library")
                            .tooltip("Back to library (Esc)")
                            .icon(IconName::ArrowLeft)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Closed))),
                    )
                    .child(div().flex_1())
                    .child(Button::new("toggle-filmstrip").ghost()
                        .icon(IconName::PanelBottom).selected(self.strip_shown)
                        .tooltip("Toggle thumbnail strip (T)")
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_strip(cx))))
                    .child(
                        Button::new("previous-card")
                            .ghost()
                            .icon(IconName::ChevronLeft)
                            .tooltip("Previous game")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                window.focus(&this.focus);
                                this.step(-1, cx);
                            })),
                    )
                    .child(
                        Button::new("next-card")
                            .ghost()
                            .icon(IconName::ChevronRight)
                            .tooltip("Next game")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                window.focus(&this.focus);
                                this.step(1, cx);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .items_center()
                    .justify_center()
                    .gap_5()
                    .child(
                        image_cache(self.cache.clone())
                            .w(dimensions.width)
                            .h(dimensions.height)
                            .child(
                                div()
                                    .id("focused-card-surface")
                                    .relative()
                                    .size_full()
                                    .child(face)
                                    .child(
                                        canvas(
                                            move |bounds, _, cx| {
                                                entity.update(cx, |this, _| this.bounds = bounds);
                                            },
                                            move |_, _, window, _| {
                                                let entity = pointer_entity.clone();
                                                // Hover callbacks exclude pressed pointers. Reset
                                                // outside the face even when a drag crossed it.
                                                window.on_mouse_event(
                                                    move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                                                        if phase != gpui::DispatchPhase::Capture {
                                                            return;
                                                        }
                                                        let _ = entity.update(cx, |this, cx| {
                                                            if !this.bounds.contains(&event.position)
                                                                && (this.pitch.value() != 0.
                                                                    || this.yaw.value() != 0.)
                                                            {
                                                                this.pitch.set(0.);
                                                                this.yaw.set(0.);
                                                                cx.notify();
                                                            }
                                                        });
                                                    },
                                                );
                                            },
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                    .when(!show_back || turning, |surface| {
                                        surface.child(
                                            div()
                                                .id("turn-shield")
                                                .absolute()
                                                .inset_0()
                                                .occlude()
                                                .on_mouse_move(cx.listener(
                                                    |this, event: &gpui::MouseMoveEvent, _, cx| {
                                                        if this.back
                                                            || this.turn.active()
                                                            || super::motion::reduced(cx)
                                                        {
                                                            return;
                                                        }
                                                        let x = f32::from(
                                                            event.position.x - this.bounds.origin.x,
                                                        ) / f32::from(
                                                            this.bounds.size.width,
                                                        )
                                                        .max(1.);
                                                        let y = f32::from(
                                                            event.position.y - this.bounds.origin.y,
                                                        ) / f32::from(
                                                            this.bounds.size.height,
                                                        )
                                                        .max(1.);
                                                        this.pitch.set(
                                                            (0.5 - y.clamp(0., 1.))
                                                                * 16_f32.to_radians(),
                                                        );
                                                        this.yaw.set(
                                                            (x.clamp(0., 1.) - 0.5)
                                                                * 24_f32.to_radians(),
                                                        );
                                                        cx.notify();
                                                    },
                                                ))
                                                .on_hover(cx.listener(|this, hovered, _, cx| {
                                                    if !hovered {
                                                        this.pitch.set(0.);
                                                        this.yaw.set(0.);
                                                        cx.notify();
                                                    }
                                                }))
                                                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                                                .on_mouse_down(
                                                    gpui::MouseButton::Left,
                                                    |_, _, cx| cx.stop_propagation(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    if !this.back && !this.turn.active() {
                                                        this.flip(window, cx);
                                                    }
                                                    cx.stop_propagation();
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("card-front-tab")
                                    .ghost()
                                    .small()
                                    .label("Front")
                                    .tooltip("Show cover (Space to turn)")
                                    .selected(!self.back)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.back {
                                            this.flip(window, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("card-back-tab")
                                    .ghost()
                                    .small()
                                    .label("Details")
                                    .tooltip("Show details (Space to turn)")
                                    .selected(self.back)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if !this.back {
                                            this.flip(window, cx);
                                        }
                                    })),
                            ),
                    ),
            )
            .when(strip_progress > 0., |viewer| viewer.child(
                div().w_full().h(px(96. * strip_progress)).flex_shrink_0().overflow_hidden()
                    .child(self.filmstrip.clone())
            ))
    }
}
