# Kanban board

Branch: `kanban-board`, created from `omarchy-cards-3d-render`.

## Goal

Add a **Board** row under **Favorites** in the sidebar. The board shows one
column for each status, in library definition order. The user can move games
between columns and create, rename, and reorder statuses on the board.

The sidebar Status section was a transition feature. The board replaced it on
September 28. Status filtering remains in the toolbar Filter menu.

## Decisions

- **Board is a sidebar scope.** `Scope::Board` contains every game that is not
  hidden, like `All`. The sidebar row uses the existing active and count logic.
  Other scope rows leave the board. The toolbar view switcher stays; choosing
  Cards, Grid, or Table leaves the board and shows All games.
- **One status source.** Library definitions own statuses. After each definition
  save, `Library::apply_definitions` replaces the manifest, the status list,
  status labels, and collection labels in one step. The board, Filter menu,
  and card menus read only `Library.statuses`. The watcher refresh uses the
  same data. Board status edits and sidebar collection edits share one
  definitions save (`ui/definitions.rs`).
- **Moving a game changes only `personal.status`.** Moves use the existing
  guarded `edit_game_personal` write, one game per move. A failed move leaves the
  card in its old column and shows the error.
- **Input.** Drag a card onto a column or onto another card. The
  card context menu is the menu alternative: the shared Status submenu, plus
  Move up/down in column. Arrow keys move the selection; Enter opens the card.
  Alt with Left/Right moves the selected game to the next status; Alt with
  Up/Down moves it inside its column.
- **Create statuses** with the **Add status** column at the end of the board.
  It opens an inline name editor; Enter saves and Escape cancels. The key comes from the label (lowercase ASCII, digits, `-`). A label
  without ASCII letters or digits gets the key `status`. A taken key gets a
  number suffix (`status-2`). Duplicate labels are rejected. New statuses are
  not recommendation eligible, so Choose results do not change.
- **Rename and reorder** statuses from the column header menu: Rename (inline
  in the header), Move left, Move right. Keys do not change.
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

## Acceptance checks

- A new status appears at once on the board, in the card Status menu, the
  Filter menu, and the editor; it survives restart.
- Dragging a card to another column changes its status and its label in Cards,
  Grid, and Table.
- A rename or reorder changes the board, menus, and labels together.
- A failed write keeps the previous state and shows an error.
- Hidden games do not appear on the board.
