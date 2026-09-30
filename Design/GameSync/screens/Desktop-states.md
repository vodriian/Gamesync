# Desktop design states

Editable reconstruction of the current Rust/GPUI app, using bundled sample content.
Open [Desktop.elyx](Desktop.elyx) for the main screen, or [AllStates.elyx](../flows/AllStates.elyx) for the full atlas. Each state has its own file. Shared layouts live in `../blocks/`; small controls live in `../controls/`. Colors live in `../tokens/desktop*.elyx`.

The project contains 90 states. Use the links below to open a state, or the
[project guide](../README.md) to find a focused overview. Edit a shared component to change all its instances; use a
screen override for a local experiment.

## Editor compatibility

The installed Elyx Preview is `0.0.1-preview.14918.b8a492b89`. It needs explicit
source paths in cross-file overrides, such as `gridTileSource.GridTile.title`. Keep these
paths: the newer global CLI accepts shorthand that this editor cannot resolve.
For compatible normalization and checks on this workstation, use
`/Applications/Elyx Preview.app/Contents/Resources/bin/elyx`.

Native visual references checked: Home, Grid, Cards, Table, Board, display menu, focused front/back, General Settings. Other states are reconstructed from UI source. Linux Omarchy is source-derived. Static frames do not implement native interactions, 3D perspective, or animation.

The light baseline matches the native sample shell. Dark is a token-based counterpart for design work, not a pixel-verified capture. Sample prices, counts, notes, and sync messages illustrate states. No personal account data is included.

## 01 · Home

- [Home](main/Home.elyx)
- [HomeDisconnected](main/HomeDisconnected.elyx)

## 02 · Library views

- [Grid](library/views/Grid.elyx)
- [GridCoversOnly](library/views/GridCoversOnly.elyx)
- [Cards](library/views/Cards.elyx)
- [Table](library/views/Table.elyx)
- [Board](library/views/Board.elyx)
- [Favorites](library/sections/Favorites.elyx)
- [Wishlist](library/sections/Wishlist.elyx)
- [Collection](library/sections/Collection.elyx)
- [SmartCollection](library/sections/SmartCollection.elyx)
- [HiddenGames](library/sections/HiddenGames.elyx)
- [SidebarCollapsed](library/states/SidebarCollapsed.elyx)

## 03 · Selection and editing

- [TableSelection](library/states/TableSelection.elyx)
- [SearchResults](library/states/SearchResults.elyx)
- [BulkPartialFailure](library/states/BulkPartialFailure.elyx)
- [EditNote](library/states/EditNote.elyx)
- [GridKeyboardFocus](library/states/GridKeyboardFocus.elyx)
- [FilteredLibrary](library/states/FilteredLibrary.elyx)
- [GroupedLibrary](library/states/GroupedLibrary.elyx)
- [TableInlineStatus](library/states/TableInlineStatus.elyx)
- [TableInlineRating](library/states/TableInlineRating.elyx)

## 04 · Empty, loading and errors

- [LibraryEmpty](library/states/LibraryEmpty.elyx)
- [SearchEmpty](library/states/SearchEmpty.elyx)
- [LibraryLoading](library/states/LibraryLoading.elyx)
- [LibraryError](library/states/LibraryError.elyx)
- [MissingCover](library/states/MissingCover.elyx)
- [LibraryWriteIssue](library/states/LibraryWriteIssue.elyx)
- [WishlistPriceLoading](library/states/WishlistPriceLoading.elyx)
- [WishlistPriceUnavailable](library/states/WishlistPriceUnavailable.elyx)

## 05 · Menus

- [DisplayMenu](library/menus/DisplayMenu.elyx)
- [FilterMenu](library/menus/FilterMenu.elyx)
- [SortMenu](library/menus/SortMenu.elyx)
- [GroupMenu](library/menus/GroupMenu.elyx)
- [GridViewMenu](library/menus/GridViewMenu.elyx)
- [GameContextMenu](library/menus/GameContextMenu.elyx)
- [StatusMenu](library/menus/StatusMenu.elyx)
- [CollectionMenu](library/menus/CollectionMenu.elyx)
- [WishlistSortMenu](library/menus/WishlistSortMenu.elyx)
- [WishlistContextMenu](library/menus/WishlistContextMenu.elyx)

## 06 · Board states

- [BoardColumnMenu](library/board/BoardColumnMenu.elyx)
- [BoardNewStatus](library/board/BoardNewStatus.elyx)
- [BoardRenameStatus](library/board/BoardRenameStatus.elyx)
- [BoardDragging](library/board/BoardDragging.elyx) — Static drag snapshot; no animation or drop behavior in Elyx.

## 07 · Collections

- [CollectionNew](library/collections/CollectionNew.elyx)
- [CollectionRename](library/collections/CollectionRename.elyx)
- [CollectionContextMenu](library/collections/CollectionContextMenu.elyx)
- [CollectionSaveFailed](library/collections/CollectionSaveFailed.elyx)

## 08 · Focused card

- [CardFront](details/CardFront.elyx)
- [CardFrontNoFilmstrip](details/CardFrontNoFilmstrip.elyx)
- [CardDetails](details/CardDetails.elyx)
- [DetailsUnsaved](details/DetailsUnsaved.elyx)
- [DetailsSaving](details/DetailsSaving.elyx)
- [DetailsSaved](details/DetailsSaved.elyx)
- [DetailsSaveFailed](details/DetailsSaveFailed.elyx)
- [DetailsConflict](details/DetailsConflict.elyx)
- [DetailsReadOnly](details/DetailsReadOnly.elyx)
- [DetailsStatusMenu](details/DetailsStatusMenu.elyx)
- [DetailsActionsMenu](details/DetailsActionsMenu.elyx)
- [DetailsTagPicker](details/DetailsTagPicker.elyx)

## 09 · Settings

- [SettingsGeneral](settings/general/SettingsGeneral.elyx)
- [SteamConnected](settings/general/SteamConnected.elyx)
- [SteamSyncing](settings/general/SteamSyncing.elyx)
- [SteamSyncFailed](settings/general/SteamSyncFailed.elyx)
- [SteamKeyTestFailed](settings/general/SteamKeyTestFailed.elyx)
- [SettingsAppearance](settings/appearance/SettingsAppearance.elyx)
- [SettingsOmarchy](settings/appearance/SettingsOmarchy.elyx) — Linux-only theme state reconstructed from source. Not visually verified on Linux.
- [AiLocal](settings/ai/AiLocal.elyx) — Existing AI Settings preview. Tests are simulated; no real AI integration is implied.
- [AiCloud](settings/ai/AiCloud.elyx) — Existing AI Settings preview. Tests are simulated; no real AI integration is implied.
- [AiTested](settings/ai/AiTested.elyx) — Existing AI Settings preview. Tests are simulated; no real AI integration is implied.
- [AiKeyAdded](settings/ai/AiKeyAdded.elyx) — Existing AI Settings preview. Tests are simulated; no real AI integration is implied.

## 10 · Device sync

- [SyncOff](settings/sync/SyncOff.elyx)
- [SyncSampleUnavailable](settings/sync/SyncSampleUnavailable.elyx)
- [SyncUpToDate](settings/sync/SyncUpToDate.elyx)
- [SyncChecking](settings/sync/SyncChecking.elyx)
- [SyncFolderUnavailable](settings/sync/SyncFolderUnavailable.elyx)
- [SyncPending](settings/sync/SyncPending.elyx)
- [SyncStopConfirmation](settings/sync/SyncStopConfirmation.elyx)
- [KeySyncOff](settings/keys/KeySyncOff.elyx)
- [KeySyncLocked](settings/keys/KeySyncLocked.elyx)
- [KeySyncUnlocked](settings/keys/KeySyncUnlocked.elyx)
- [KeySyncChanged](settings/keys/KeySyncChanged.elyx)
- [KeySyncError](settings/keys/KeySyncError.elyx)
- [SyncReviewChanges](settings/sync/SyncReviewChanges.elyx)
- [ReviewGameVersions](review/ReviewGameVersions.elyx) — Legacy revision review content. Source-derived; its dedicated native window is represented on a review canvas.
- [ReviewCollections](review/ReviewCollections.elyx) — Legacy revision review content. Source-derived; its dedicated native window is represented on a review canvas.

## 11 · Dark appearance

- [GridDark](appearance/GridDark.elyx)
- [CardsDark](appearance/CardsDark.elyx)
- [CardDetailsDark](appearance/CardDetailsDark.elyx)
- [SettingsAppearanceDark](appearance/SettingsAppearanceDark.elyx)
