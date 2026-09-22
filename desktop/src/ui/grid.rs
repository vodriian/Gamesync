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
use gpui_component::{menu::ContextMenuExt as _, Disableable as _};
use std::{rc::Rc, sync::Arc};
mod actions;
mod bulk;

const GAP: Pixels = px(24.);
// Keep spacing inside the full-width scroll mask so shadows can use the gutter.
const CONTENT_INSET: Pixels = px(20.);
const CAPTION: Pixels = px(52.);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LibraryView {
    Cards,
    Grid,
    Table,
}
/// The source library identity prevents drops into a different loaded store.
#[derive(Clone)]
pub(super) struct DraggedGame {
    pub id: uuid::Uuid,
    pub library_id: Option<uuid::Uuid>,
    title: String,
    offset: gpui::Point<Pixels>,
}
impl Render for DraggedGame {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .ml(self.offset.x + px(12.))
            .mt(self.offset.y + px(12.))
            .px_3()
            .py_2()
            .gap_2()
            .rounded_lg()
            .shadow_lg()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .child(gpui_component::Icon::new(gpui_component::IconName::Plus).size_4())
            .child(self.title.clone())
    }
}
fn drag_game(data: &GameCell) -> DraggedGame {
    DraggedGame {
        id: data.game.id,
        library_id: data.library_id,
        title: data.game.title.clone(),
        offset: gpui::Point::default(),
    }
}

pub struct BulkSaved(pub String);
impl gpui::EventEmitter<BulkSaved> for GameGrid {}

pub struct OpenGame;
impl gpui::EventEmitter<OpenGame> for GameGrid {}

pub struct GameGrid {
    selection: std::collections::BTreeSet<uuid::Uuid>,
    note: Option<actions::NoteDraft>,
    saving: bool,
    feedback: String,
    restore_focus: bool,
    view: LibraryView,
    hovered: Option<uuid::Uuid>,
    preserve_viewport: bool,
    preserve_next_library_update: bool,
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
    groups: Vec<(String, Vec<usize>)>,
    rows: Vec<(Option<String>, Vec<usize>)>,
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
            let lib = library.read(cx);
            let visible = lib.visible.clone();
            let preserve_next_update = std::mem::take(&mut this.preserve_next_library_update);
            this.selection
                .retain(|id| visible.iter().any(|&i| lib.games[i].id == *id));
            if *this.last_visible != *visible || this.groups != lib.groups {
                this.groups = lib.groups.clone();
                this.hovered = None;
                this.last_visible = visible;
                this.rebuild_rows();
                if !this.preserve_viewport && !preserve_next_update {
                    this.scroll
                        .base_handle()
                        .set_offset(gpui::point(px(0.), px(0.)));
                }
            }
            cx.notify();
        })
        .detach();
        Self {
            selection: Default::default(),
            note: None,
            saving: false,
            feedback: String::new(),
            restore_focus: false,
            view: LibraryView::Cards,
            hovered: None,
            preserve_viewport: false,
            preserve_next_library_update: false,
            hover_bounds: Bounds::default(),
            pitch: super::card_motion::Spring::new(0.),
            yaw: super::card_motion::Spring::new(0.),
            last_visible: library.read(cx).visible.clone(),
            library: library.clone(),
            cache,
            focus: cx.focus_handle(),
            scroll: VirtualListScrollHandle::new(),
            width: px(0.),
            columns: 1,
            cell: px(150.),
            minimum: px(260.),
            row_sizes: Rc::new(Vec::new()),
            groups: library.read(cx).groups.clone(),
            rows: Vec::new(),
        }
    }

    pub fn preserve_viewport(&mut self, preserve: bool) {
        self.preserve_viewport = preserve;
    }

    pub fn set_view(&mut self, view: LibraryView, cx: &mut Context<Self>) {
        self.hovered = None;
        self.view = view;
        self.selection.clear();
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
                .scroll_to_item(self.row_for_slot(slot), ScrollStrategy::Top);
        }
        cx.notify();
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        if self.groups.is_empty() {
            let slots: Vec<_> = (0..self.last_visible.len()).collect();
            self.rows
                .extend(slots.chunks(self.columns).map(|s| (None, s.to_vec())));
        } else {
            for (label, slots) in &self.groups {
                self.rows
                    .push((Some(format!("{} · {}", label, slots.len())), Vec::new()));
                self.rows
                    .extend(slots.chunks(self.columns).map(|s| (None, s.to_vec())));
            }
        }
        self.row_sizes = Rc::new(
            self.rows
                .iter()
                .map(|(header, _)| {
                    size(
                        self.width,
                        if header.is_some() {
                            px(44.)
                        } else {
                            match self.view {
                                LibraryView::Cards => self.cell * 1.46 + GAP,
                                LibraryView::Grid => self.cell * 1.5 + CAPTION + GAP,
                                LibraryView::Table => px(66.),
                            }
                        },
                    )
                })
                .collect(),
        );
    }

    fn row_for_slot(&self, slot: usize) -> usize {
        self.rows
            .iter()
            .position(|(_, slots)| slots.contains(&slot))
            .unwrap_or(0)
    }

    fn measure(&mut self, width: Pixels) -> bool {
        let usable = f32::from(width - CONTENT_INSET * 2.).max(0.);
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
        let selected = self.library.read(cx).selected_slot();
        let mut ordered: Vec<_> = self
            .rows
            .iter()
            .flat_map(|(_, slots)| slots.iter().copied())
            .collect();
        // A multi-collection game is one keyboard stop, like bulk selection.
        let mut seen = std::collections::HashSet::new();
        ordered.retain(|slot| seen.insert(*slot));
        if ordered.is_empty() {
            return;
        }
        let position = selected.and_then(|slot| ordered.iter().position(|s| *s == slot));
        let next = position.map_or(0, |p| p.saturating_add_signed(delta).min(ordered.len() - 1));
        let slot = ordered[next];
        self.library.update(cx, |lib, cx| {
            lib.select_slot(slot);
            cx.notify();
        });
        self.scroll
            .scroll_to_item(self.row_for_slot(slot), ScrollStrategy::Top);
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
            .on_drag(drag_game(data), |game, offset, _, cx| {
                cx.new(|_| {
                    let mut preview = game.clone();
                    preview.offset = offset;
                    preview
                })
            })
            .on_click(cx.listener(move |this, _, window, cx| this.open(slot, window, cx)))
            .context_menu({
                let grid = cx.entity();
                let id = data.game.id;
                move |menu, window, cx| actions::menu(grid.clone(), id, menu, window, cx)
            })
            .into_any_element()
    }
    fn render_grid(
        &mut self,
        library: &Entity<Library>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
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
                                let rows_data: Vec<(Option<String>, Vec<GameCell>)> = {
                                    let library = library.read(cx);
                                    rows.clone()
                                        .map(|row| {
                                            let (header, slots) = &this.rows[row];
                                            (
                                                header.clone(),
                                                slots
                                                    .iter()
                                                    .copied()
                                                    .map(|slot| {
                                                        let idx = visible[slot];
                                                        GameCell::new(
                                                            slot,
                                                            &library.games[idx],
                                                            library
                                                                .source
                                                                .as_ref()
                                                                .map(|(_, m)| m.library_id),
                                                        )
                                                    })
                                                    .collect(),
                                            )
                                        })
                                        .collect()
                                };

                                rows_data
                                    .into_iter()
                                    .enumerate()
                                    .map(|(offset, (header, cells))| {
                                        if let Some(label) = header {
                                            return div()
                                                .h(px(44.))
                                                .px(CONTENT_INSET)
                                                .flex()
                                                .items_center()
                                                .text_sm()
                                                .font_medium()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(label)
                                                .into_any_element();
                                        }
                                        h_flex()
                                            .id(("game-row", rows.start + offset))
                                            .px(CONTENT_INSET)
                                            .gap(GAP)
                                            .pb(if this.view == LibraryView::Table {
                                                px(0.)
                                            } else {
                                                GAP
                                            })
                                            .children(cells.into_iter().map(|data| {
                                                let active = false;
                                                if this.view == LibraryView::Table {
                                                    return table_row(
                                                        &data,
                                                        cell_size,
                                                        this.selection.contains(&data.game.id),
                                                        this.saving,
                                                        cx,
                                                    );
                                                }
                                                if this.view == LibraryView::Cards {
                                                    return this.card_cell(
                                                        &data, cell_size, active, window, cx,
                                                    );
                                                }
                                                cell(&data, cell_size, active, cx)
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.restore_focus {
            self.restore_focus = false;
            window.focus(&self.focus);
        }
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
                if this.note.is_some() {
                    cx.propagate();
                    return;
                }
                if event.keystroke.key == "escape" && !this.saving {
                    this.selection.clear();
                    cx.notify();
                    return;
                }
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
                                "Try another search or clear the filters."
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
            .when(self.view == LibraryView::Table, |view| {
                view.child(self.table_header(cx))
            })
            .when(
                self.view == LibraryView::Table && !self.selection.is_empty(),
                |view| view.child(self.bulk_panel(cx)),
            )
            .when(self.note.is_some(), |view| view.child(self.note_dialog(cx)))
            .when(self.note.is_none() && !self.feedback.is_empty(), |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom(px(if self.selection.is_empty() { 16. } else { 70. }))
                        .left_4()
                        .right_4()
                        .p_3()
                        .rounded_lg()
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().popover_foreground)
                        .text_sm()
                        .child(self.feedback.clone()),
                )
            })
    }
}

/// Clone only the visible records, so rendering never holds a model borrow.
struct GameCell {
    slot: usize,
    game: Game,
    library_id: Option<uuid::Uuid>,
}
impl GameCell {
    fn new(slot: usize, game: &Game, library_id: Option<uuid::Uuid>) -> Self {
        Self {
            slot,
            game: game.clone(),
            library_id,
        }
    }
}

fn cell(
    data: &GameCell,
    width: Pixels,
    selected: bool,
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
        .on_drag(drag_game(data), |game, offset, _, cx| {
            cx.new(|_| {
                let mut preview = game.clone();
                preview.offset = offset;
                preview
            })
        })
        .on_click(cx.listener(move |this, _, window, cx| {
            window.focus(&this.focus);
            this.open(slot, window, cx);
        }))
        .child(
            div()
                .id("cover")
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .h(width * 1.5)
                .overflow_hidden()
                .rounded(px(8.))
                .child(
                    img(data
                        .game
                        .cover_path
                        .clone()
                        .map(gpui::ImageSource::from)
                        .unwrap_or_else(|| data.game.cover.clone().into()))
                    .size_full()
                    .rounded(px(8.))
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
                )
                .child(
                    div()
                        .id("cover-highlight")
                        .absolute()
                        .inset_0()
                        .rounded(px(8.))
                        .border_4()
                        .border_color(if selected {
                            cx.theme().ring
                        } else {
                            cx.theme().transparent
                        })
                        .hover(|style| style.border_color(cx.theme().ring.opacity(0.5)))
                        .active(|style| style.border_color(cx.theme().ring.opacity(0.5))),
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
        .context_menu({
            let grid = cx.entity();
            let id = data.game.id;
            move |menu, window, cx| actions::menu(grid.clone(), id, menu, window, cx)
        })
        .into_any_element()
}

fn table_row(
    data: &GameCell,
    width: Pixels,
    selected: bool,
    saving: bool,
    cx: &mut Context<GameGrid>,
) -> gpui::AnyElement {
    let slot = data.slot;
    let id = data.game.id;
    h_flex()
        .id(slot)
        .w(width)
        .h(px(66.))
        .px_3()
        .gap_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .bg(if selected {
            cx.theme().selection
        } else {
            cx.theme().background
        })
        .hover(|s| {
            s.bg(if selected {
                cx.theme().selection
            } else {
                cx.theme().secondary
            })
        })
        .cursor_pointer()
        .on_drag(drag_game(data), |game, offset, _, cx| {
            cx.new(|_| {
                let mut preview = game.clone();
                preview.offset = offset;
                preview
            })
        })
        .on_click(cx.listener(move |this, _, window, cx| this.open(slot, window, cx)))
        .child(
            gpui_component::checkbox::Checkbox::new(gpui::SharedString::from(format!(
                "select-game-{id}"
            )))
            .checked(selected)
            .disabled(saving)
            .on_click(cx.listener(move |this, checked, window, cx| {
                cx.stop_propagation();
                if *checked {
                    this.selection.insert(id);
                } else {
                    this.selection.remove(&id);
                }
                window.focus(&this.focus);
                this.feedback.clear();
                cx.notify();
            })),
        )
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
        .context_menu({
            let grid = cx.entity();
            let id = data.game.id;
            move |menu, window, cx| actions::menu(grid.clone(), id, menu, window, cx)
        })
        .into_any_element()
}
