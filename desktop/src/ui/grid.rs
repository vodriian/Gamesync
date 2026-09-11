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
    h_flex, scroll::ScrollableElement as _, v_flex, v_virtual_list, ActiveTheme as _,
    StyledExt as _, VirtualListScrollHandle,
};
use std::{rc::Rc, sync::Arc};

const GAP: Pixels = px(8.);
const CAPTION: Pixels = px(52.);

pub struct GameGrid {
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
            if !Arc::ptr_eq(&this.last_visible, &visible) {
                this.last_visible = visible;
                this.rebuild_rows();
                this.scroll.scroll_to_item(0, ScrollStrategy::Top);
            }
            cx.notify();
        })
        .detach();
        Self {
            last_visible: library.read(cx).visible.clone(),
            library,
            cache,
            focus: cx.focus_handle(),
            scroll: VirtualListScrollHandle::new(),
            width: px(0.),
            columns: 1,
            cell: px(150.),
            minimum: px(150.),
            row_sizes: Rc::new(Vec::new()),
        }
    }

    pub fn set_density(&mut self, compact: bool, cx: &mut Context<Self>) {
        self.minimum = px(if compact { 120. } else { 150. });
        self.measure(self.width);
        cx.notify();
    }

    fn rebuild_rows(&mut self) {
        self.row_sizes = Rc::new(vec![
            size(self.width, self.cell * 1.5 + CAPTION + GAP);
            self.last_visible.len().div_ceil(self.columns)
        ]);
    }

    fn measure(&mut self, width: Pixels) -> bool {
        let usable = f32::from(width).max(0.);
        let columns = ((usable + f32::from(GAP)) / (f32::from(self.minimum) + f32::from(GAP)))
            .floor()
            .max(1.) as usize;
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
    fn render_grid(
        &mut self,
        library: &Entity<Library>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let columns = self.columns;
        let cell_size = self.cell;
        let selected = library.read(cx).selected;
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
                            move |this, rows, _, cx| {
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
                                            .pb(GAP)
                                            .children(cells.into_iter().map(|data| {
                                                let active = selected == Some(data.game.id);
                                                cell(&data, cell_size, active, cx)
                                            }))
                                            .into_any_element()
                                    })
                                    .collect()
                            },
                        )
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
                                "This demo library is empty."
                            } else {
                                "Try another search or status."
                            }),
                    )
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
            this.library.update(cx, |lib, cx| {
                lib.select_slot(slot);
                cx.notify();
            });
        }))
        .child(
            div()
                .id("cover")
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
                    img(data.game.cover.clone())
                        .size_full()
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
                .h(CAPTION - GAP)
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
                        .child(data.game.status.label())
                        .child(data.game.rating.map_or(String::new(), |rating| {
                            format!("{:.1}", f32::from(rating) / 2.)
                        })),
                ),
        )
        .into_any_element()
}
