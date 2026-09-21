//! Portrait grid adapted from Eagle's row-virtualized grid.
//! Only visible rows build elements. The image cache is shared with the inspector.

use crate::{
    model::{Game, Library},
    ui::thumb_cache::LruImageCache,
};
use gpui::{
    canvas, div, image_cache, img, prelude::*, px, size, Bounds, Entity, FocusHandle, Focusable,
    ObjectFit, Pixels, ScrollStrategy, Size, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::ScrollableElement as _,
    v_flex, v_virtual_list, ActiveTheme as _, StyledExt as _, VirtualListScrollHandle,
};
use std::{rc::Rc, sync::Arc};

const GAP: Pixels = px(24.);
const CAPTION: Pixels = px(52.);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LibraryView {
    Cards,
    Grid,
    Table,
}
pub struct OpenGame;
impl gpui::EventEmitter<OpenGame> for GameGrid {}

pub struct GameGrid {
    view: LibraryView,
    hovered: Option<uuid::Uuid>,
    preserve_viewport: bool,
    hover_bounds: Bounds<Pixels>,
    pitch: super::card_motion::Spring,
    yaw: super::card_motion::Spring,
    library: Entity<Library>,
    cache: Entity<LruImageCache>,
    focus: FocusHandle,
    scroll: VirtualListScrollHandle,
    width: Pixels,
    columns: usize,
    cell: Pixels,
    minimum: Pixels,
    last_visible: Arc<Vec<usize>>,
    row_sizes: Rc<Vec<Size<Pixels>>>,
}

impl Focusable for GameGrid {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl GameGrid {
    pub fn new(
        library: Entity<Library>,
        cache: Entity<LruImageCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |this, library, cx| {
            let visible = library.read(cx).visible.clone();
            if *this.last_visible != *visible {
                this.hovered = None;
                this.last_visible = visible;
                this.rebuild_rows();
                if !this.preserve_viewport {
                    this.scroll
                        .base_handle()
                        .set_offset(gpui::point(px(0.), px(0.)));
                }
            }
            cx.notify();
        })
        .detach();
        Self {
            view: LibraryView::Cards,
            hovered: None,
            preserve_viewport: false,
            hover_bounds: Bounds::default(),
            pitch: super::card_motion::Spring::new(0.),
            yaw: super::card_motion::Spring::new(0.),
            last_visible: library.read(cx).visible.clone(),
            library,
            cache,
            focus: cx.focus_handle(),
            scroll: VirtualListScrollHandle::new(),
            width: px(0.),
            columns: 1,
            cell: px(150.),
            minimum: px(260.),
            row_sizes: Rc::new(Vec::new()),
        }
    }

    pub fn preserve_viewport(&mut self, preserve: bool) {
        self.preserve_viewport = preserve;
    }

    pub fn set_view(&mut self, view: LibraryView, cx: &mut Context<Self>) {
        self.hovered = None;
        self.view = view;
        self.minimum = px(if view == LibraryView::Cards {
            260.
        } else {
            145.
        });
        self.measure(self.width);
        self.rebuild_rows();
        if let Some(slot) = self.library.read(cx).visible.iter().position(|&i| {
            Some(self.library.read(cx).games[i].id) == self.library.read(cx).selected
        }) {
            self.scroll
                .scroll_to_item(slot / self.columns, ScrollStrategy::Top);
        }
        cx.notify();
    }

    fn rebuild_rows(&mut self) {
        self.row_sizes = Rc::new(vec![
            size(
                self.width,
                match self.view {
                    LibraryView::Cards => self.cell * 1.46 + GAP,
                    LibraryView::Grid => self.cell * 1.5 + CAPTION + GAP,
                    LibraryView::Table => px(66.),
                }
            );
            self.last_visible.len().div_ceil(self.columns)
        ]);
    }

    fn measure(&mut self, width: Pixels) -> bool {
        let usable = f32::from(width).max(0.);
        let columns = if self.view == LibraryView::Table {
            1
        } else {
            ((usable + f32::from(GAP)) / (f32::from(self.minimum) + f32::from(GAP)))
                .floor()
                .max(1.) as usize
        };
        let cell = px(((usable - f32::from(GAP) * (columns - 1) as f32) / columns as f32).max(1.));
        if columns == self.columns && (f32::from(self.cell - cell)).abs() < 0.5 {
            return false;
        }
        self.width = width;
        self.columns = columns;
        self.cell = cell;
        self.rebuild_rows();
        true
    }

    fn open(&mut self, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.hovered = None;
        self.pitch = super::card_motion::Spring::new(0.);
        self.yaw = super::card_motion::Spring::new(0.);
        window.focus(&self.focus);
        self.library.update(cx, |lib, cx| {
            lib.select_slot(slot);
            cx.notify();
        });
        cx.emit(OpenGame);
    }

    fn navigate(&mut self, delta: isize, cx: &mut Context<Self>) {
        let slot = self.library.update(cx, |lib, cx| {
            let slot = lib.step_selection(delta);
            cx.notify();
            slot
        });
        if let Some(slot) = slot {
            // In gpui-component 0.5.1, Top reveals a row only when it is outside
            // the viewport. Center moves even visible rows and causes jumps.
            self.scroll
                .scroll_to_item(slot / self.columns, ScrollStrategy::Top);
        }
    }
    fn card_cell(
        &mut self,
        data: &GameCell,
        cell_size: Pixels,
        active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let slot = data.slot;
        let id = data.game.id;
        let reduced = super::motion::reduced(cx) || !cfg!(target_os = "macos");
        let hovering = self.hovered == Some(id) && !reduced;
        let face_size = cell_size - px(24.);
        let face = super::card::front(&data.game, f32::from(face_size), None, active, cx);
        let face = if hovering {
            if window.is_window_active() && (self.pitch.active() || self.yaw.active()) {
                window.request_animation_frame();
            }
            gpui::card_layer(
                super::card::surface_id(id, 3),
                size(face_size, face_size * 1.46),
                gpui::CardPose {
                    pitch: self.pitch.value(),
                    yaw: self.yaw.value(),
                    back: false,
                    material: true,
                    frosted_top: 0.,
                },
                px(17.),
                face,
            )
        } else {
            face.into_any_element()
        };
        let entity = cx.entity();
        div()
            .id(slot)
            .relative()
            .w(cell_size)
            .h(cell_size * 1.46)
            .flex_shrink_0()
            .p(px(12.))
            .cursor_pointer()
            .child(face)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| {
                            if this.hovered == Some(id) {
                                this.hover_bounds = bounds;
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                if *hovered {
                    this.hovered = Some(id);
                    this.pitch = super::card_motion::Spring::new(0.);
                    this.yaw = super::card_motion::Spring::new(0.);
                } else if this.hovered == Some(id) {
                    this.pitch.set(0.);
                    this.yaw.set(0.);
                }
                cx.notify();
            }))
            .on_mouse_move(
                cx.listener(move |this, event: &gpui::MouseMoveEvent, _, cx| {
                    if this.hovered != Some(id)
                        || super::motion::reduced(cx)
                        || !cfg!(target_os = "macos")
                    {
                        return;
                    }
                    let bounds = this.hover_bounds;
                    let x = f32::from(event.position.x - bounds.origin.x)
                        / f32::from(bounds.size.width).max(1.);
                    let y = f32::from(event.position.y - bounds.origin.y)
                        / f32::from(bounds.size.height).max(1.);
                    this.pitch
                        .set((0.5 - y.clamp(0., 1.)) * 10_f32.to_radians());
                    this.yaw.set((x.clamp(0., 1.) - 0.5) * 16_f32.to_radians());
                    cx.notify();
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.open(slot, window, cx)))
            .into_any_element()
    }
    fn render_grid(
        &mut self,
        library: &Entity<Library>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let columns = self.columns;
        let cell_size = self.cell;

        let visible = library.read(cx).visible.clone();

        image_cache(self.cache.clone())
            .size_full()
            .child(
                v_flex()
                    .id("asset-grid")
                    .relative()
                    .size_full()
                    .child(
                        v_virtual_list(
                            cx.entity(),
                            "grid",
                            self.row_sizes.clone(),
                            move |this, rows, window, cx| {
                                let library = this.library.clone();

                                // Copy just what the visible cells need out of
                                // the shared model in one short borrow; building
                                // elements needs `cx` mutably for the click
                                // listeners.
                                let rows_data: Vec<Vec<GameCell>> = {
                                    let library = library.read(cx);
                                    rows.clone()
                                        .map(|row| {
                                            let start = row * columns;
                                            let end = (start + columns).min(visible.len());
                                            (start..end)
                                                .map(|slot| {
                                                    let idx = visible[slot];
                                                    GameCell::new(slot, &library.games[idx])
                                                })
                                                .collect()
                                        })
                                        .collect()
                                };

                                rows_data
                                    .into_iter()
                                    .map(|cells| {
                                        h_flex()
                                            .gap(GAP)
                                            .pb(if this.view == LibraryView::Table {
                                                px(0.)
                                            } else {
                                                GAP
                                            })
                                            .children(cells.into_iter().map(|data| {
                                                let active = false;
                                                if this.view == LibraryView::Table {
                                                    return table_row(&data, cell_size, active, cx);
                                                }
                                                if this.view == LibraryView::Cards {
                                                    return this.card_cell(
                                                        &data, cell_size, active, window, cx,
                                                    );
                                                }
                                                cell(&data, cell_size, active, 1., cx)
                                            }))
                                            .into_any_element()
                                    })
                                    .collect()
                            },
                        )
                        .pt(px(if self.view == LibraryView::Table {
                            110.
                        } else {
                            80.
                        }))
                        .pb(px(32.))
                        .track_scroll(&self.scroll),
                    )
                    // The scrollbar is an overlay sibling driven by the same
                    // handle; `VirtualList` itself is not a ParentElement.
                    .vertical_scrollbar(&self.scroll),
            )
            .into_any_element()
    }
}

impl Render for GameGrid {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let measure = canvas(
            move |bounds: Bounds<Pixels>, _, cx| {
                entity.update(cx, |this, cx| {
                    if this.measure(bounds.size.width) {
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let library = self.library.clone();
        let empty = library.read(cx).visible.is_empty();
        div()
            .id("game-grid")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    if this.library.read(cx).selected.is_some() {
                        cx.emit(OpenGame);
                    }
                    return;
                }
                let delta = match event.keystroke.key.as_str() {
                    "left" => -1,
                    "right" => 1,
                    "up" => -(this.columns as isize),
                    "down" => this.columns as isize,
                    _ => {
                        cx.propagate();
                        return;
                    }
                };
                this.navigate(delta, cx);
            }))
            .child(measure)
            .child(if empty {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(if library.read(cx).games.is_empty() {
                        "No games yet"
                    } else {
                        "No games found"
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if library.read(cx).games.is_empty() {
                                "Connect Steam in Settings to sync your games."
                            } else {
                                "Try another search or status."
                            }),
                    )
                    .when(library.read(cx).games.is_empty(), |column| {
                        column.child(
                            div().pt_2().child(
                                Button::new("connect-steam")
                                    .primary()
                                    .label("Connect Steam")
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(Box::new(crate::ConnectSteam), cx)
                                    }),
                            ),
                        )
                    })
                    .into_any_element()
            } else {
                self.render_grid(&library, cx)
            })
    }
}

/// Clone only the visible records, so rendering never holds a model borrow.
struct GameCell {
    slot: usize,
    game: Game,
}
impl GameCell {
    fn new(slot: usize, game: &Game) -> Self {
        Self {
            slot,
            game: game.clone(),
        }
    }
}

fn cell(
    data: &GameCell,
    width: Pixels,
    selected: bool,
    scale: f32,
    cx: &mut Context<GameGrid>,
) -> gpui::AnyElement {
    let slot = data.slot;
    let muted = cx.theme().muted_foreground;
    v_flex()
        .id(slot)
        .w(width)
        .flex_shrink_0()
        .gap_1()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| {
            window.focus(&this.focus);
            this.open(slot, window, cx);
        }))
        .child(
            div()
                .id("cover")
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .h(width * 1.5)
                .overflow_hidden()
                .rounded(cx.theme().radius)
                .border_2()
                .border_color(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().transparent
                })
                .hover(|style| style.border_color(cx.theme().primary.opacity(0.55)))
                .active(|style| style.border_color(cx.theme().primary))
                .bg(cx.theme().secondary)
                .child(
                    img(data
                        .game
                        .cover_path
                        .clone()
                        .map(gpui::ImageSource::from)
                        .unwrap_or_else(|| data.game.cover.clone().into()))
                    .w(width * scale)
                    .h(width * 1.5 * scale)
                    .flex_shrink_0()
                    .object_fit(ObjectFit::Cover)
                    .with_fallback(move || {
                        v_flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .text_color(muted)
                            .child(gpui_component::Icon::new(gpui_component::IconName::File))
                            .child(div().text_xs().child("Cover unavailable"))
                            .into_any_element()
                    }),
                ),
        )
        .child(
            v_flex()
                .h(CAPTION - px(8.))
                .px_1()
                .gap_0p5()
                .overflow_hidden()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .truncate()
                        .child(data.game.title.clone()),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .text_xs()
                        .text_color(muted)
                        .child(data.game.status_label.clone())
                        .child(data.game.rating.map_or(String::new(), |rating| {
                            format!("{:.1}", f32::from(rating) / 2.)
                        })),
                ),
        )
        .into_any_element()
}

fn table_row(
    data: &GameCell,
    width: Pixels,
    selected: bool,
    cx: &mut Context<GameGrid>,
) -> gpui::AnyElement {
    let slot = data.slot;
    h_flex()
        .id(slot)
        .w(width)
        .h(px(66.))
        .px_3()
        .gap_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .bg(if selected {
            cx.theme().secondary
        } else {
            cx.theme().background
        })
        .hover(|s| s.bg(cx.theme().secondary))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| this.open(slot, window, cx)))
        .child(
            img(data
                .game
                .cover_path
                .clone()
                .map(gpui::ImageSource::from)
                .unwrap_or_else(|| data.game.cover.clone().into()))
            .w(px(30.))
            .h(px(44.))
            .rounded(px(4.))
            .object_fit(ObjectFit::Cover)
            .with_fallback(|| div().w(px(30.)).h(px(44.)).child("✦").into_any_element()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .font_medium()
                .child(data.game.title.clone()),
        )
        .child(
            div()
                .w(px(140.))
                .text_sm()
                .child(data.game.status_label.clone()),
        )
        .child(div().w(px(100.)).text_sm().child(data.game.rating_label()))
        .child(
            div()
                .w(px(100.))
                .text_sm()
                .child(format!("{:.1} h", data.game.playtime_minutes as f32 / 60.)),
        )
        .into_any_element()
}
