//! Status board. Columns come from `Library::board_columns`, so they follow the
//! library status definitions. Moves write one game through the grid's guarded
//! save; status edits are in `board_status`. Cards stay flat on every platform: the board does not use
//! the macOS Metal or Linux Blade card compositor.
use super::board_status::StatusTarget;
use super::*;
use crate::model::BoardColumn;
use gamesync_desktop::board::rank_for_drop;
use gpui::{uniform_list, ScrollHandle, UniformListScrollHandle};
use gpui_component::{
    menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    Icon, IconName, Sizable as _,
};
use uuid::Uuid;

const COLUMN_WIDTH: Pixels = px(280.);
const CARD_HEIGHT: Pixels = px(84.);
const CARD_GAP: Pixels = px(8.);
const COLUMN_GAP: Pixels = px(16.);

/// Per-column scroll handles and the horizontal board scroll.
#[derive(Default)]
pub(super) struct BoardState {
    pub columns: Vec<BoardColumn>,
    scrolls: Vec<UniformListScrollHandle>,
    horizontal: ScrollHandle,
}

impl BoardState {
    pub fn refresh(&mut self, library: &Library) {
        self.columns = library.board_columns();
        self.scrolls
            .resize_with(self.columns.len(), UniformListScrollHandle::new);
    }

    /// Column and row of a game.
    fn find(&self, id: Uuid, library: &Library) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(c, column)| {
            column
                .games
                .iter()
                .position(|&i| library.games[i].id == id)
                .map(|row| (c, row))
        })
    }
}

/// Where a move puts a game inside its column.
#[derive(Clone, Copy)]
enum Place {
    /// Before this game. An unranked target means after the ranked games.
    Before(Uuid),
    AfterRanked,
}

impl GameGrid {
    /// Arrow keys move the selection. Alt with arrows moves the selected game:
    /// left and right change its status, up and down change its position.
    pub(super) fn board_key(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();
        if !matches!(key, "left" | "right" | "up" | "down") {
            return false;
        }
        let lib = self.library.read(cx);
        let Some((column, row)) = lib.selected.and_then(|id| self.board.find(id, lib)) else {
            // Nothing selected: start at the first card on the board.
            if let Some(column) = self.board.columns.iter().position(|c| !c.games.is_empty()) {
                self.select_card(column, 0, cx);
            }
            return true;
        };
        if event.keystroke.modifiers.alt {
            let Some(id) = self.library.read(cx).selected else {
                return true;
            };
            match key {
                "left" | "right" => {
                    let step = if key == "left" { -1 } else { 1 };
                    let target = self.board.columns[..]
                        .get(column.wrapping_add_signed(step))
                        .and_then(|c| c.key.clone());
                    if let Some(target) = target {
                        self.move_card(id, target, Place::AfterRanked, cx);
                    }
                }
                "up" => {
                    if let Some(place) = self.place_up(column, row, cx) {
                        let key = self.board.columns[column].key.clone();
                        if let Some(key) = key {
                            self.move_card(id, key, place, cx);
                        }
                    }
                }
                _ => {
                    if let Some(place) = self.place_down(column, row, cx) {
                        let key = self.board.columns[column].key.clone();
                        if let Some(key) = key {
                            self.move_card(id, key, place, cx);
                        }
                    }
                }
            }
            return true;
        }
        match key {
            "up" => self.select_card(column, row.saturating_sub(1), cx),
            "down" => self.select_card(column, row + 1, cx),
            _ => {
                // Skip empty columns and keep the row where possible.
                let columns = &self.board.columns;
                let next = if key == "left" {
                    (0..column).rev().find(|&c| !columns[c].games.is_empty())
                } else {
                    (column + 1..columns.len()).find(|&c| !columns[c].games.is_empty())
                };
                if let Some(next) = next {
                    self.select_card(next, row, cx);
                }
            }
        }
        true
    }

    fn select_card(&mut self, column: usize, row: usize, cx: &mut Context<Self>) {
        let Some(games) = self.board.columns.get(column).map(|c| &c.games) else {
            return;
        };
        let Some(&index) = games.get(row.min(games.len().saturating_sub(1))) else {
            return;
        };
        let row = row.min(games.len() - 1);
        self.board.scrolls[column].scroll_to_item(row, ScrollStrategy::Top);
        self.board.horizontal.scroll_to_item(column);
        self.library.update(cx, |lib, cx| {
            lib.selected = Some(lib.games[index].id);
            cx.notify();
        });
    }

    /// One place up. `None` at the top of the column, and when the board
    /// follows a sort: rows are then not in manual order.
    fn place_up(&self, column: usize, row: usize, cx: &gpui::App) -> Option<Place> {
        let games = &self.board.columns[column].games;
        let lib = self.library.read(cx);
        (lib.display.board_manual && row > 0).then(|| Place::Before(lib.games[games[row - 1]].id))
    }

    /// One place down. Unranked games follow the Sort menu, so a game cannot
    /// move below an unranked game; `None` there, at the bottom, and when the
    /// board follows a sort.
    fn place_down(&self, column: usize, row: usize, cx: &gpui::App) -> Option<Place> {
        let games = &self.board.columns[column].games;
        let lib = self.library.read(cx);
        if !lib.display.board_manual {
            return None;
        }
        lib.games[*games.get(row + 1)?].board_rank()?;
        Some(match games.get(row + 2) {
            Some(&next) if lib.games[next].board_rank().is_some() => {
                Place::Before(lib.games[next].id)
            }
            _ => Place::AfterRanked,
        })
    }

    /// Save a new status and position for one game.
    fn move_card(&mut self, id: Uuid, key: String, place: Place, cx: &mut Context<Self>) {
        if let Place::Before(target) = place {
            if target == id {
                return;
            }
        }
        let lib = self.library.read(cx);
        let Some(column) = self
            .board
            .columns
            .iter()
            .find(|c| c.key.as_deref() == Some(key.as_str()))
        else {
            return;
        };
        let others: Vec<&Game> = column
            .games
            .iter()
            .map(|&i| &lib.games[i])
            .filter(|g| g.id != id)
            .collect();
        // A sorted board shows the column in Sort menu order, not rank order.
        let mut ranked: Vec<&str> = others.iter().filter_map(|g| g.board_rank()).collect();
        ranked.sort_unstable();
        let before = match place {
            Place::Before(target) => others
                .iter()
                .find(|g| g.id == target)
                .and_then(|g| g.board_rank())
                .and_then(|rank| ranked.iter().position(|r| *r == rank)),
            Place::AfterRanked => None,
        };
        let Some(rank) = rank_for_drop(&ranked, before) else {
            self.feedback = "Could not place the game here. Try another position.".into();
            cx.notify();
            return;
        };
        let target = match self.target_for(id, cx) {
            Ok(target) => target,
            Err(message) => {
                self.feedback = message;
                cx.notify();
                return;
            }
        };
        let mut personal = target.base.game.personal.clone();
        personal.status = key;
        personal.board_rank = Some(rank);
        self.save_personal(target, personal, false, cx);
    }

    fn on_board_drop(
        &mut self,
        dragged: &DraggedGame,
        key: String,
        place: Place,
        cx: &mut Context<Self>,
    ) {
        let library_id = self
            .library
            .read(cx)
            .source
            .as_ref()
            .map(|(_, m)| m.library_id);
        if dragged.library_id.is_none() || dragged.library_id != library_id {
            return;
        }
        let same_column = {
            let lib = self.library.read(cx);
            self.board
                .find(dragged.id, lib)
                .is_some_and(|(column, _)| self.board.columns[column].key.as_deref() == Some(&key))
        };
        self.library
            .update(cx, |lib, _| lib.selected = Some(dragged.id));
        // A drag inside a column asks for manual order. A drag to another
        // column only changes the status, so a sorted board stays sorted.
        if same_column {
            self.use_manual_order(cx);
        }
        self.move_card(dragged.id, key, place, cx);
    }

    /// Turn on manual order for Board and save it with the display settings.
    fn use_manual_order(&mut self, cx: &mut Context<Self>) {
        if self.library.read(cx).display.board_manual {
            return;
        }
        self.library.update(cx, |lib, cx| {
            lib.display.board_manual = true;
            lib.recompute();
            cx.notify();
        });
        cx.spawn(async move |grid, cx| {
            let result = cx
                .background_spawn(async {
                    crate::settings::update(|s| s.library_display.board_manual = true)
                })
                .await;
            if let Err(error) = result {
                let _ = grid.update(cx, |grid, cx| {
                    grid.feedback = format!("Could not save the board order setting: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn render_board(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let columns = self.board.columns.clone();
        let writable = {
            let lib = self.library.read(cx);
            lib.source.is_some() && lib.write_issue.is_none()
        };
        // Drops and menus wait while a save runs or an editor is open.
        let editable = writable && !self.busy();
        let count = columns.iter().filter(|c| c.key.is_some()).count();
        image_cache(self.cache.clone())
            .size_full()
            .child(
                div()
                    .relative()
                    .size_full()
                    .child(
                        h_flex()
                            .id("board")
                            .size_full()
                            .overflow_x_scroll()
                            // Keep vertical wheel input for the column under the pointer.
                            .map(|mut board| {
                                board.style().restrict_scroll_to_axis = Some(true);
                                board
                            })
                            .track_scroll(&self.board.horizontal)
                            .items_start()
                            .gap(COLUMN_GAP)
                            .pt(px(80.))
                            .pb(px(20.))
                            .px(CONTENT_INSET)
                            .children(columns.into_iter().enumerate().map(|(index, column)| {
                                self.board_column(index, column, count, editable, cx)
                            }))
                            .when(writable, |board| board.child(self.add_status_column(cx))),
                    )
                    .horizontal_scrollbar(&self.board.horizontal),
            )
            .into_any_element()
    }

    fn board_column(
        &mut self,
        index: usize,
        column: BoardColumn,
        count: usize,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let scroll = self.board.scrolls[index].clone();
        let key = column.key.clone();
        let droppable = editable && key.is_some();
        let games = column.games.clone();
        let list_key = key.clone();
        v_flex()
            .id(("board-column", index))
            .w(COLUMN_WIDTH)
            .h_full()
            .flex_shrink_0()
            .rounded(crate::theme::interface_radius(cx, px(12.)))
            .bg(cx.theme().secondary.opacity(0.55))
            .border_1()
            .border_color(cx.theme().border.opacity(0.4))
            .when_some(key.clone().filter(|_| droppable), |col, key| {
                col.drag_over::<DraggedGame>(|style, _, _, cx| {
                    style
                        .bg(cx.theme().primary.opacity(0.12))
                        .border_color(cx.theme().primary.opacity(0.6))
                })
                .on_drop(cx.listener(move |this, game: &DraggedGame, _, cx| {
                    this.on_board_drop(game, key.clone(), Place::AfterRanked, cx)
                }))
            })
            .child(self.column_header(index, &column, count, editable, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .when(games.is_empty(), |body| {
                        body.child(
                            div()
                                .m_2()
                                .p_4()
                                .rounded(crate::theme::interface_radius(cx, px(8.)))
                                .border_1()
                                .border_dashed()
                                .border_color(cx.theme().border)
                                .text_xs()
                                .text_center()
                                .text_color(cx.theme().muted_foreground)
                                .child(if droppable {
                                    "Drop a game here"
                                } else {
                                    "No games"
                                }),
                        )
                    })
                    .when(!games.is_empty(), |body| {
                        body.child(
                            uniform_list(
                                ("board-list", index),
                                games.len(),
                                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .map(|row| {
                                            this.board_card(
                                                games[row],
                                                list_key.clone(),
                                                droppable,
                                                cx,
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .size_full()
                            .px_2()
                            .pb_2()
                            .track_scroll(scroll.clone()),
                        )
                        .vertical_scrollbar(&scroll)
                    }),
            )
            .into_any_element()
    }

    fn column_header(
        &self,
        index: usize,
        column: &BoardColumn,
        count: usize,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let key = column.key.clone();
        if key.is_some()
            && self.status_edit.as_ref().is_some_and(|edit| {
                edit.target == StatusTarget::Rename(key.clone().unwrap_or_default())
            })
        {
            return self.status_editor(cx);
        }
        let grid = cx.entity();
        h_flex()
            .h(px(44.))
            .flex_shrink_0()
            .pl_3()
            .pr_1()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_semibold()
                    .child(column.label.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(column.games.len().to_string()),
            )
            .when_some(key.filter(|_| editable), |header, key| {
                header.child(
                    Button::new(("column-menu", index))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Ellipsis)
                        .tooltip("Status options")
                        .dropdown_menu(move |menu, _, _| {
                            column_menu(menu, &grid, &key, index, count)
                        }),
                )
            })
            .into_any_element()
    }

    fn board_card(
        &mut self,
        index: usize,
        key: Option<String>,
        droppable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lib = self.library.read(cx);
        let game = lib.games[index].clone();
        let data = GameCell::new(0, &game, lib.source.as_ref().map(|(_, m)| m.library_id));
        let selected = lib.selected == Some(game.id);
        let id = game.id;
        let muted = cx.theme().muted_foreground;
        let radius = crate::theme::interface_radius(cx, px(8.));
        let hours = format!("{:.1} h", game.playtime_minutes as f32 / 60.);
        div()
            .h(CARD_HEIGHT + CARD_GAP)
            .pt(CARD_GAP)
            .child(
                h_flex()
                    .id(gpui::SharedString::from(format!("board-card-{id}")))
                    .h(CARD_HEIGHT)
                    .p_2()
                    .gap_3()
                    .rounded(radius)
                    .border_1()
                    .border_color(if selected {
                        cx.theme().ring
                    } else {
                        cx.theme().border.opacity(0.6)
                    })
                    .bg(cx.theme().background)
                    .shadow_xs()
                    .cursor_pointer()
                    .hover(|style| style.border_color(cx.theme().ring.opacity(0.5)))
                    .when_some(key.filter(|_| droppable), |card, key| {
                        // A top edge shows that the dropped game goes before this one.
                        card.drag_over::<DraggedGame>(|style, _, _, cx| {
                            style.border_t_4().border_color(cx.theme().primary)
                        })
                        .on_drop(cx.listener(
                            move |this, dragged: &DraggedGame, _, cx| {
                                this.on_board_drop(dragged, key.clone(), Place::Before(id), cx)
                            },
                        ))
                    })
                    .on_drag(drag_game(&data), |game, offset, _, cx| {
                        cx.new(|_| {
                            let mut preview = game.clone();
                            preview.offset = offset;
                            preview
                        })
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        window.focus(&this.focus);
                        this.library.update(cx, |lib, cx| {
                            lib.selected = Some(id);
                            cx.notify();
                        });
                        cx.emit(OpenGame);
                    }))
                    .context_menu({
                        let grid = cx.entity();
                        move |menu, window, cx| {
                            let menu = actions::menu(grid.clone(), id, menu, window, cx);
                            board_order_menu(menu, &grid, id, cx)
                        }
                    })
                    .child(
                        div()
                            .w(px(44.))
                            .h(px(66.))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .rounded(crate::theme::interface_radius(cx, px(4.)))
                            .bg(cx.theme().muted)
                            .child(
                                img(game
                                    .cover_path
                                    .clone()
                                    .map(gpui::ImageSource::from)
                                    .unwrap_or_else(|| game.cover.clone().into()))
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .with_fallback(move || {
                                    div()
                                        .size_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(muted)
                                        .child(Icon::new(IconName::File).size_4())
                                        .into_any_element()
                                }),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .truncate()
                                    .child(game.title.clone()),
                            )
                            .when(!game.collections.is_empty(), |details| {
                                details.child(
                                    h_flex()
                                        .min_w_0()
                                        .gap_1()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .min_w_0()
                                                .max_w(px(120.))
                                                .px_1()
                                                .rounded(crate::theme::pill_radius(cx))
                                                .bg(cx.theme().muted_foreground.opacity(0.10))
                                                .text_xs()
                                                .text_color(cx.theme().foreground.opacity(0.75))
                                                .truncate()
                                                .child(game.collections[0].clone()),
                                        )
                                        .when(game.collections.len() > 1, |row| {
                                            row.child(
                                                div().flex_shrink_0().text_xs().child(format!(
                                                    "+{}",
                                                    game.collections.len() - 1
                                                )),
                                            )
                                        }),
                                )
                            })
                            .child(
                                h_flex()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(hours)
                                    .when_some(game.rating, |row, rating| {
                                        row.child(
                                            h_flex()
                                                .gap_1()
                                                .child(
                                                    Icon::new(crate::assets::RatingIcon).size_3(),
                                                )
                                                .child(format!("{:.1}", f32::from(rating) / 2.)),
                                        )
                                    })
                                    .when(game.favorite, |row| {
                                        row.child(Icon::new(crate::assets::FavoriteIcon).size_3())
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

/// Keyboard and menu alternative to dragging inside a column.
fn board_order_menu(
    menu: PopupMenu,
    grid: &Entity<GameGrid>,
    id: Uuid,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let view = grid.read(cx);
    let lib = view.library.read(cx);
    if view.view != LibraryView::Board || lib.source.is_none() || view.busy() {
        return menu;
    }
    let Some((column, row)) = view.board.find(id, lib) else {
        return menu;
    };
    let Some(key) = view.board.columns[column].key.clone() else {
        return menu;
    };
    let up = view.place_up(column, row, cx);
    let down = view.place_down(column, row, cx);
    let (up_grid, down_grid, up_key) = (grid.clone(), grid.clone(), key.clone());
    menu.separator()
        .item(
            PopupMenuItem::new("Move up in column")
                .disabled(up.is_none())
                .on_click(move |_, _, cx| {
                    if let Some(place) = up {
                        up_grid.update(cx, |g, cx| g.move_card(id, up_key.clone(), place, cx));
                    }
                }),
        )
        .item(
            PopupMenuItem::new("Move down in column")
                .disabled(down.is_none())
                .on_click(move |_, _, cx| {
                    if let Some(place) = down {
                        down_grid.update(cx, |g, cx| g.move_card(id, key.clone(), place, cx));
                    }
                }),
        )
}

fn column_menu(
    menu: PopupMenu,
    grid: &Entity<GameGrid>,
    key: &str,
    index: usize,
    count: usize,
) -> PopupMenu {
    let (rename, left, right) = (grid.clone(), grid.clone(), grid.clone());
    let (rename_key, left_key, right_key) = (key.to_owned(), key.to_owned(), key.to_owned());
    menu.item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
        rename.update(cx, |grid, cx| {
            grid.begin_status(StatusTarget::Rename(rename_key.clone()), window, cx)
        })
    }))
    .separator()
    .item(
        PopupMenuItem::new("Move left")
            .disabled(index == 0)
            .on_click(move |_, _, cx| {
                left.update(cx, |grid, cx| grid.move_status(&left_key, false, cx))
            }),
    )
    .item(
        PopupMenuItem::new("Move right")
            .disabled(index + 1 >= count)
            .on_click(move |_, _, cx| {
                right.update(cx, |grid, cx| grid.move_status(&right_key, true, cx))
            }),
    )
}

impl GameGrid {
    fn add_status_column(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let editing = self
            .status_edit
            .as_ref()
            .is_some_and(|edit| edit.target == StatusTarget::New);
        div()
            .w(COLUMN_WIDTH)
            .flex_shrink_0()
            .when(editing, |column| {
                column
                    .rounded(crate::theme::interface_radius(cx, px(12.)))
                    .bg(cx.theme().secondary.opacity(0.55))
                    .child(self.status_editor(cx))
            })
            .when(!editing, |column| {
                column.child(
                    Button::new("board-add-status")
                        .ghost()
                        .w_full()
                        .icon(IconName::Plus)
                        .label("Add status")
                        .disabled(self.busy())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_status(StatusTarget::New, window, cx)
                        })),
                )
            })
            .into_any_element()
    }
}
