# Kanban board

Branch: `kanban-board`, created from `omarchy-cards-3d-render`.

## Goal

Add a **Board** row under **Favorites** in the sidebar. The board shows one
column for each status, in library definition order. The user can move games
between columns and create statuses. The sidebar Status list and the board
columns always show the same statuses, in the same order.

## Decisions

- **Board is a sidebar scope.** `Scope::Board` contains every game that is not
  hidden, like `All`. The sidebar row uses the existing active and count logic.
  Other scope rows leave the board. The toolbar view switcher stays; choosing
  Cards, Grid, or Table leaves the board and shows All games.
- **One status source.** Library definitions own statuses. After each definition
  save, `Library::apply_definitions` replaces the manifest, the status list,
  status labels, and collection labels in one step. The sidebar and board read
  only `Library.statuses`. The watcher refresh uses the same data.
- **Moving a game changes only `personal.status`.** Moves use the existing
  guarded `edit_game_personal` write, one game per move. A failed move leaves the
  card in its old column and shows the error.
- **Input.** Drag a card onto a column or onto another card. Status rows in the
  sidebar also accept a game; that drop works like a menu status change. The
  card context menu is the menu alternative: the shared Status submenu, plus
  Move up/down in column. Arrow keys move the selection; Enter opens the card.
  Alt with Left/Right moves the selected game to the next status; Alt with
  Up/Down moves it inside its column.
- **Create statuses.** Use the plus button at the Status sidebar header or the
  **Add status** column at the end of the board. Both use the same inline name
  editor. The key comes from the label (lowercase ASCII, digits, `-`). A label
  without ASCII letters or digits gets the key `status`. A taken key gets a
  number suffix (`status-2`). Duplicate labels are rejected. New statuses are
  not recommendation eligible, so Choose results do not change.
- **Rename and reorder** statuses from the context menu of a sidebar row or a
  column header: Rename, Move up/left, Move down/right. Keys do not change.
  The board header menu forwards to the sidebar, which owns the one name
  editor and all status definition writes.
- **Status removal stays deferred.** The store refuses to remove a key because
  games on other computers can still use it. Removal needs game reassignment.
- **Filters and search compose with the board**, like other views. Group by
  does not apply; the columns are the groups. Choosing Cards, Grid, or Table
  leaves the board and shows All games. A search with no results keeps the
  columns so the user can still drop games and add statuses.
- **Cross-platform.** Board cards are flat on every platform. They do not use
  the macOS Metal or Omarchy Blade card compositor. Drag and drop is in-app
  GPUI drag state, not OS drag and drop, so macOS and Linux behave the same.
  Storage has no new platform code. Windows remains deferred with the store.
- **Performance.** Each column is a virtual list with fixed card height, so a
  10,000-game column builds only visible cards. Covers use the shared image
  cache. The board scrolls horizontally when columns do not fit.

## Order inside a column

The user chose manual order. Each game stores an optional `board_rank` in
personal data. It is a fractional index: a string of `0-9a-z` digits compared
as text, with no trailing `0`. A new rank always fits between two neighbors, so
a move writes only the moved game and never renumbers a column.

- Ranked games come first, by rank. Games without a rank follow them in the
  Sort menu order. Ties use the Sort menu order.
- Drop on a ranked card: insert before it. Drop on an unranked card or on the
  empty part of a column: place after the last ranked game.
- A status change from a menu, the editor, or bulk edit clears the rank. The
  game then follows the Sort menu in its new column.
- An invalid rank from another writer counts as no rank. It is not an error.

The user also chose create, rename, and reorder for statuses in this branch.

## Steps

1. Model: `Scope::Board`, `apply_definitions`, board column query, and status
   key generation. Unit tests for columns, unknown statuses, key generation, and
   definition sync.
2. Status edits: generalize the sidebar name editor to collections and
   statuses. Add create, rename, and reorder with guarded definition writes.
3. Board view: columns, cards, drag/drop moves, drop highlight, context menu,
   keyboard selection, empty states, and the Add status column.
4. Sidebar: Board row, Status plus button, status row drops and menus.
5. Checks: `cargo fmt --check`, tests, Clippy, release build, and a native
   visual check on Linux (Omarchy and a standard theme). macOS check pending
   unless run on a Mac.

## Acceptance checks

- A new status appears at once in the sidebar, the board, the card Status menu,
  the Filter menu, and the editor; it survives restart.
- Dragging a card to another column changes its status, its sidebar counts, and
  its label in Cards, Grid, and Table.
- A rename or reorder changes the sidebar and the board together.
- A failed write keeps the previous state and shows an error.
- Hidden games do not appear on the board.
