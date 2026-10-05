//! Focused card that opens like a book: the cover turns on its spine to show
//! two pages of details. The editor owns drafts and guarded saves.

use super::conflicts::ConflictReview;
use super::editor::{EditorEvent, InspectorEditor};
use crate::{model::Library, ui::thumb_cache::LruImageCache};
use gpui::{
    canvas, div, image_cache, img, prelude::*, px, App, Bounds, Entity, FocusHandle, ObjectFit,
    Pixels, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
};
use std::f32::consts::PI;

pub struct DetailPanel {
    focus: FocusHandle,
    filmstrip: Entity<super::filmstrip::Filmstrip>,
    strip_shown: bool,
    strip_motion: super::motion::Motion,
    // Keep the open card when an edit removes it from the current library filter.
    viewed: Option<uuid::Uuid>,
    take_focus: bool,
    /// The book is open, or opening. `turn` is the cover's angle from 0 to PI.
    open: bool,
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
    best_on: Option<Entity<super::best_on::BestOnPanel>>,
    review: Option<Entity<ConflictReview>>,
    cover_refresh: Option<uuid::Uuid>,
    details_resync: Option<uuid::Uuid>,
    steam_notice: Option<(uuid::Uuid, String)>,
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
            open: false,
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
            best_on: None,
            review: None,
            cover_refresh: None,
            details_resync: None,
            steam_notice: None,
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
        self.open = self.busy(cx);
        self.turn = super::card_motion::Spring::new(if self.open { PI } else { 0. });
        self.pitch.set(0.);
        self.yaw.set(0.);
        self.take_focus = true;
        cx.notify();
    }

    fn flip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.edit_focus = window.focused(cx).filter(|focus| focus != &self.focus);
            self.restore_edit_focus = false;
        } else {
            self.restore_edit_focus = self.edit_focus.is_some();
        }
        window.focus(&self.focus);
        self.open = !self.open;
        let target = if self.open { PI } else { 0. };
        if !super::motion::card_3d_enabled(cx) {
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
            || self.details_resync.is_some()
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
                EditorEvent::ShowScope(_) => {}
            }
            cx.notify();
        })
        .detach();
        self.viewed = Some(id);
        self.open = true;
        self.turn = super::card_motion::Spring::new(PI);
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
        self.steam_notice = Some((id, "Getting cover from Steam…".into()));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(
                    async move { gamesync_desktop::steam::covers::refresh(&root, id) },
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                this.cover_refresh = None;
                this.steam_notice = Some((
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

    /// Refetch store details, reviews, and Steam tags for the open game.
    fn resync_details(&mut self, cx: &mut Context<Self>) {
        if self.details_resync.is_some() {
            return;
        }
        let Some((root, _)) = self.library.read(cx).source.clone() else {
            return;
        };
        let Some(id) = self.viewed else {
            return;
        };
        self.details_resync = Some(id);
        self.steam_notice = Some((id, "Getting details from Steam…".into()));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { gamesync_desktop::steam::resync_game(&root, id) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.details_resync = None;
                let notice = match result {
                    Ok(failures) if failures.is_empty() => "Details updated from Steam.".into(),
                    Ok(failures) => format!("Some details did not update: {}", failures.join("; ")),
                    Err(error) => format!("Details not changed: {error}"),
                };
                // The watcher also refreshes, but an explicit request makes it prompt.
                cx.emit(EditorEvent::Saved);
                this.steam_notice = Some((id, notice));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Store page, cover refresh, and details resync for a Steam game.
    fn actions_menu(&self, app_id: u32, cx: &mut Context<Self>) -> impl IntoElement {
        let cover = cx.entity();
        let details = cx.entity();
        let refreshing = self.cover_refresh.is_some();
        let resyncing = self.details_resync.is_some();
        Button::new("card-actions")
            .ghost()
            .xsmall()
            .icon(IconName::Ellipsis)
            .tooltip("Game actions")
            .dropdown_menu(move |menu, _, _| {
                let cover = cover.clone();
                let details = details.clone();
                menu.item(
                    PopupMenuItem::new("Open store page").on_click(move |_, _, cx| {
                        cx.open_url(&format!("https://store.steampowered.com/app/{app_id}/"))
                    }),
                )
                .separator()
                .item(
                    PopupMenuItem::new(if refreshing {
                        "Refreshing cover…"
                    } else {
                        "Refresh cover"
                    })
                    .disabled(refreshing)
                    .on_click(move |_, _, cx| cover.update(cx, |this, cx| this.refresh_cover(cx))),
                )
                .item(
                    PopupMenuItem::new(if resyncing {
                        "Resyncing details…"
                    } else {
                        "Resync details"
                    })
                    .disabled(resyncing)
                    .on_click(move |_, _, cx| {
                        details.update(cx, |this, cx| this.resync_details(cx))
                    }),
                )
            })
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
                EditorEvent::ShowScope(scope) => cx.emit(EditorEvent::ShowScope(scope.clone())),
            }
            cx.notify();
        })
        .detach();
        // The detail panel places the editor's pages, so it renders its changes.
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
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
            if self
                .best_on
                .as_ref()
                .is_none_or(|panel| panel.read(cx).game_id != game.id)
            {
                let panel = cx
                    .new(|cx| super::best_on::BestOnPanel::new(self.library.clone(), game.id, cx));
                cx.subscribe(
                    &panel,
                    |this, _, event: &super::best_on::ChoosePreference, cx| {
                        if let Some(editor) = &this.editor {
                            editor
                                .update(cx, |editor, cx| editor.set_setup_preference(event.0, cx));
                        }
                    },
                )
                .detach();
                self.best_on = Some(panel);
            }
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
        let available = if self.viewport.height > px(0.) {
            self.viewport
        } else {
            let window_size = window.viewport_size();
            gpui::size(window_size.width, window_size.height - px(50.))
        };
        // A page keeps the card's proportions. The open book needs room for two.
        let width = ((f32::from(available.height) - 270.) / 1.46)
            .min((f32::from(available.width) - 64.) / 2.)
            .clamp(160., 460.);
        let height = width * 1.46;
        let tilt = super::motion::card_3d_enabled(cx);
        // Only the Metal compositor turns a face on its edge. Other renderers
        // switch between the closed card and the open book without motion.
        let book_motion = tilt && cfg!(target_os = "macos");
        let turning = book_motion && self.turn.active();
        let angle = if book_motion {
            self.turn.value()
        } else if self.open {
            PI
        } else {
            0.
        };
        let closed = !self.open && !turning;
        let spread = self.open && !turning;
        if spread && self.restore_edit_focus {
            self.restore_edit_focus = false;
            if let Some(focus) = &self.edit_focus {
                window.focus(focus);
            }
        }
        if window.is_window_active()
            && tilt
            && (turning || self.pitch.active() || self.yaw.active())
        {
            window.request_animation_frame();
        }
        // The spine starts at the closed card's left edge and ends at the
        // center of the open book, so the visible object stays centered.
        let spine = width * (0.5 + (1. - angle.cos()) / 4.);
        let radius = crate::theme::interface_radius(cx, px(17.));
        let dimensions = gpui::size(px(width), px(height));
        let pitch = if tilt { self.pitch.value() } else { 0. };
        let yaw = if tilt { self.yaw.value() } else { 0. };
        let game_id = game.as_ref().map(|g| g.id);
        let layer_id = |role| game_id.map_or(0, |id| super::card::surface_id(id, role));

        let mut stage = div().id("focused-card-surface").relative().size_full();
        let front = game
            .as_ref()
            .map(|game| super::card::front(game, width, None, false, cx));
        if closed {
            stage = stage.child(
                div()
                    .absolute()
                    .left(px(spine))
                    .top_0()
                    .child(gpui::card_layer(
                        layer_id(1),
                        dimensions,
                        gpui::CardPose {
                            pitch,
                            yaw,
                            material: true,
                            ..Default::default()
                        },
                        radius,
                        div().children(front),
                    )),
            );
        } else if let Some(game) = &game {
            let (left, right) = self.pages(game, cx);
            if spread {
                stage = stage.child(
                    h_flex()
                        .size_full()
                        .rounded(radius)
                        .overflow_hidden()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .shadow(super::card::card_shadow(true))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .border_r_1()
                                .border_color(cx.theme().border)
                                .child(left),
                        )
                        .child(div().flex_1().min_w_0().h_full().child(right)),
                );
            } else {
                // While turning, the cover is a leaf hinged on the spine: its
                // front is the card, its back is the left page. The right page
                // lies flat beneath it.
                let leaf = gpui::CardPose {
                    pitch,
                    yaw: yaw - angle,
                    material: true,
                    ..Default::default()
                };
                stage =
                    stage
                        .child(
                            div()
                                .absolute()
                                .left(px(spine))
                                .top_0()
                                .w(dimensions.width)
                                .h(dimensions.height)
                                .rounded_r(radius)
                                .overflow_hidden()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().background)
                                .shadow(super::card::card_shadow(false))
                                .child(right),
                        )
                        .child(
                            div()
                                .absolute()
                                .left(px(spine))
                                .top_0()
                                .child(gpui::card_layer(
                                    layer_id(1),
                                    dimensions,
                                    gpui::CardPose { hinge: -1., ..leaf },
                                    radius,
                                    div().children(front),
                                )),
                        )
                        .child(
                            div().absolute().left(px(spine - width)).top_0().child(
                                gpui::card_layer(
                                    layer_id(2),
                                    dimensions,
                                    gpui::CardPose {
                                        back: true,
                                        hinge: 1.,
                                        ..leaf
                                    },
                                    radius,
                                    div()
                                        .w(dimensions.width)
                                        .h(dimensions.height)
                                        .rounded(radius)
                                        .overflow_hidden()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().background)
                                        .child(left),
                                ),
                            ),
                        );
            }
        }
        if !spread {
            stage = stage.child(self.shield(spine, width, height, turning, cx));
        }

        let viewport_target = cx.entity();
        v_flex()
            .relative()
            .child(
                canvas(
                    move |bounds, _, cx| {
                        viewport_target.update(cx, |this, cx| {
                            if this.viewport != bounds.size {
                                this.viewport = bounds.size;
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .id("card-viewer")
            .occlude()
            .track_focus(&self.focus)
            .size_full()
            .bg(super::card::tabletop(cx))
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &crate::SaveDetails, _, cx| {
                if let Some(editor) = &this.editor {
                    editor.update(cx, |editor, cx| editor.save_now(cx));
                }
            }))
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
                    .child(
                        Button::new("toggle-filmstrip")
                            .ghost()
                            .icon(IconName::PanelBottom)
                            .selected(self.strip_shown)
                            .tooltip("Toggle thumbnail strip (T)")
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_strip(cx))),
                    )
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
                            .w(px(width * 2.))
                            .h(dimensions.height)
                            .child(stage),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("card-front-tab")
                                    .ghost()
                                    .small()
                                    .label("Front")
                                    .tooltip("Close the card (Space)")
                                    .selected(!self.open)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.open {
                                            this.flip(window, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("card-back-tab")
                                    .ghost()
                                    .small()
                                    .label("Details")
                                    .tooltip("Open the card (Space)")
                                    .selected(self.open)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if !this.open {
                                            this.flip(window, cx);
                                        }
                                    })),
                            ),
                    ),
            )
            .when(strip_progress > 0., |viewer| {
                viewer.child(
                    div()
                        .w_full()
                        .h(px(96. * strip_progress))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(self.filmstrip.clone()),
                )
            })
    }
}

impl DetailPanel {
    /// Left and right pages of the open book. The editor owns personal fields;
    /// without a library store, read-only text stands in for them.
    fn pages(
        &mut self,
        game: &crate::model::Game,
        cx: &mut Context<Self>,
    ) -> (gpui::AnyElement, gpui::AnyElement) {
        let muted = cx.theme().muted_foreground;
        let cover = div()
            .mx_3()
            .mt_3()
            .h(px(150.))
            .flex_shrink_0()
            .rounded(crate::theme::interface_radius(cx, px(10.)))
            .overflow_hidden()
            .bg(cx.theme().secondary)
            .child(
                img(super::card::cover_image(game))
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            );
        // Only games with Steam Deck evidence or demo values show Best on.
        let assessed = game.record.as_ref().is_some_and(|record| {
            gamesync_desktop::suitability::assessment(&record.game).is_some()
        });
        let best_on = self
            .best_on
            .clone()
            .filter(|_| assessed)
            .map(|panel| panel.into_any_element());
        let (summary, personal) = match (&self.review, &self.editor) {
            // A conflict review replaces the editable fields until it closes.
            (Some(review), _) => (
                div()
                    .px_4()
                    .pt_3()
                    .font_family("Georgia")
                    .text_xl()
                    .child(game.title.clone())
                    .into_any_element(),
                review.clone().into_any_element(),
            ),
            (None, Some(editor)) => editor.update(cx, |editor, cx| {
                (editor.summary_page(cx), editor.personal_page(best_on, cx))
            }),
            (None, None) => preview_pages(game, best_on, cx),
        };
        let left = v_flex().size_full().child(cover).child(summary);
        // Menu results show under the menu that ran them.
        let notice = self
            .steam_notice
            .as_ref()
            .filter(|(id, _)| *id == game.id)
            .map(|(_, notice)| {
                div()
                    .px_4()
                    .pt_4()
                    .pr(px(44.))
                    .text_xs()
                    .text_color(muted)
                    .child(notice.clone())
            });
        let menu = self
            .review
            .is_none()
            .then(|| game.record.as_ref()?.game.steam.as_ref())
            .flatten()
            .map(|steam| self.actions_menu(steam.app_id, cx));
        let right = div()
            .relative()
            .size_full()
            .child(
                v_flex()
                    .size_full()
                    .children(notice)
                    .child(div().flex_1().min_h_0().child(personal)),
            )
            .children(menu.map(|menu| div().absolute().top_2().right_2().child(menu)));
        (left.into_any_element(), right.into_any_element())
    }

    /// Input cover for the closed or turning card. A click on the closed card
    /// opens it; pointer position tilts it. While the cover turns, the shield
    /// covers the whole book so a click cannot land on a moving page.
    fn shield(
        &self,
        spine: f32,
        width: f32,
        height: f32,
        turning: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        let pointer_entity = entity.downgrade();
        div()
            .absolute()
            .top_0()
            .left(px(if turning { 0. } else { spine }))
            .w(px(if turning { width * 2. } else { width }))
            .h(px(height))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| this.bounds = bounds);
                    },
                    move |_, _, window, _| {
                        let entity = pointer_entity.clone();
                        // Hover callbacks exclude pressed pointers. Reset
                        // outside the face even when a drag crossed it.
                        window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                            if phase != gpui::DispatchPhase::Capture {
                                return;
                            }
                            let _ = entity.update(cx, |this, cx| {
                                if !this.bounds.contains(&event.position)
                                    && (this.pitch.value() != 0. || this.yaw.value() != 0.)
                                {
                                    this.pitch.set(0.);
                                    this.yaw.set(0.);
                                    cx.notify();
                                }
                            });
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .id("turn-shield")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                        if this.open || this.turn.active() || !super::motion::card_3d_enabled(cx) {
                            return;
                        }
                        let x = f32::from(event.position.x - this.bounds.origin.x)
                            / f32::from(this.bounds.size.width).max(1.);
                        let y = f32::from(event.position.y - this.bounds.origin.y)
                            / f32::from(this.bounds.size.height).max(1.);
                        this.pitch
                            .set((0.5 - y.clamp(0., 1.)) * 16_f32.to_radians());
                        this.yaw.set((x.clamp(0., 1.) - 0.5) * 24_f32.to_radians());
                        cx.notify();
                    }))
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        if !hovered {
                            this.pitch.set(0.);
                            this.yaw.set(0.);
                            cx.notify();
                        }
                    }))
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.open && !this.turn.active() {
                            this.flip(window, cx);
                        }
                        cx.stop_propagation();
                    })),
            )
    }
}

/// Read-only pages for a library without a store, such as the Best on demo.
/// They follow the editor's page order so both modes read the same.
fn preview_pages(
    game: &crate::model::Game,
    best_on: Option<gpui::AnyElement>,
    cx: &App,
) -> (gpui::AnyElement, gpui::AnyElement) {
    let theme = cx.theme();
    let hint = |text: &'static str| {
        div()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(text)
    };
    let chips = |items: &[String]| {
        h_flex()
            .flex_wrap()
            .gap_1()
            .children(items.iter().map(|item| {
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_full()
                    .text_xs()
                    .bg(theme.secondary)
                    .text_color(theme.secondary_foreground)
                    .child(item.clone())
            }))
            .when(items.is_empty(), |row| {
                row.child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("None"),
                )
            })
    };
    let summary = v_flex()
        .id("preview-summary")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px_4()
        .pt_3()
        .pb_4()
        .gap_3()
        .child(
            h_flex()
                .items_start()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family("Georgia")
                        .text_xl()
                        .line_clamp(2)
                        .child(game.title.clone()),
                )
                .when(game.favorite, |row| {
                    row.child(Icon::new(crate::assets::FavoriteIcon).size(px(22.)))
                }),
        )
        .child(div().text_sm().child(game.description.clone()))
        .when(!game.wishlisted(), |page| {
            page.child(div().text_sm().child(format!(
                "{:.1} hours played",
                game.playtime_minutes as f32 / 60.
            )))
            .child(hint("Collections"))
            .child(chips(&game.collections))
        })
        .child(hint("Tags"))
        .child(chips(&game.tags))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Preview card. Changes are unavailable in this preview."),
        );
    let rating = game.rating.unwrap_or(0);
    let personal = v_flex()
        .id("preview-personal")
        .size_full()
        .overflow_y_scroll()
        .p_4()
        .gap_3()
        .when(!game.wishlisted(), |page| {
            page.child(
                h_flex()
                    .flex_wrap()
                    .gap_x_6()
                    .gap_y_2()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(hint("Status"))
                            .child(div().text_sm().child(game.status_label.clone())),
                    )
                    .child(
                        v_flex().gap_1().child(hint("Rating")).child(
                            h_flex()
                                .gap_1()
                                .children((1u8..=5).map(|stars| {
                                    if rating >= stars * 2 {
                                        Icon::new(crate::assets::RatingIcon)
                                    } else {
                                        Icon::new(IconName::Star)
                                    }
                                    .size(px(20.))
                                }))
                                .child(
                                    div()
                                        .pl_2()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(game.rating_label()),
                                ),
                        ),
                    ),
            )
        })
        .children(best_on);
    (summary.into_any_element(), personal.into_any_element())
}
