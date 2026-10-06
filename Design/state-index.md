# GameSync design states

This is the single coverage map for the 90 original references. Related states
share one source file. Select the named variant in Elyx or render its symbol.
The four dark references use the canonical screen with `theme=dark`.
`index.elyx` opens the default Grid, Settings, and the canonical Sidebar.

## Coverage and limits

These are static design references with bundled sample content. The migration
aligns shared components with current native source: sidebar hierarchy and icons,
toolbar icons, Settings navigation, rating controls, and thumbnail geometry.
The native Home/sidebar was observed during planning. The isolated sample-app
launch timed out during implementation. Other native comparisons are not newly
verified. Linux/Omarchy, live sync, errors, animation, and 3D remain source-derived
or illustrative. AI Settings remain a simulated preview.

Card details now show the open book from the `details-card-v2` branch: the
Steam summary on the left page and personal fields on the right. The Best on
block lives in [BestOn.elyx](design/blocks/details/BestOn.elyx): fit is its
only metric, shown on one scale (Steam Deck left, PC right). Its states are
Steam Deck, PC, Both, PC only (equipment), and Needs review; (i) shows
`FitTooltip` on hover and opens the `FitInfo` modal on click. Best on rules
have their own Settings section, with optional ProtonDB enrichment. Six states were added after the original
90. The fold animation and the 3D turn are native only and are not drawn here.

Use the [workspace guide](README.md) for editing and validation commands.
The [October 1 verification log](../plan/logs/2026-10-01.md#elyx-workspace-cleanup)
records the completed static checks and remaining native comparison limit.

| Original state | Canonical file | Symbol | Theme |
| --- | --- | --- | --- |
| Home | [Home.elyx](design/screens/home/Home.elyx) | `Home` | light |
| HomeDisconnected | [Home.elyx](design/screens/home/Home.elyx) | `HomeDisconnected` | light |
| Grid | [Grid.elyx](design/screens/library/Grid.elyx) | `Grid` | light |
| GridCoversOnly | [Grid.elyx](design/screens/library/Grid.elyx) | `GridCoversOnly` | light |
| SearchResults | [Grid.elyx](design/screens/library/Grid.elyx) | `SearchResults` | light |
| GridKeyboardFocus | [Grid.elyx](design/screens/library/Grid.elyx) | `GridKeyboardFocus` | light |
| FilteredLibrary | [Grid.elyx](design/screens/library/Grid.elyx) | `FilteredLibrary` | light |
| GroupedLibrary | [Grid.elyx](design/screens/library/Grid.elyx) | `GroupedLibrary` | light |
| MissingCover | [Grid.elyx](design/screens/library/Grid.elyx) | `MissingCover` | light |
| SidebarCollapsed | [Grid.elyx](design/screens/library/Grid.elyx) | `SidebarCollapsed` | light |
| Cards | [Cards.elyx](design/screens/library/Cards.elyx) | `Cards` | light |
| Table | [Table.elyx](design/screens/library/Table.elyx) | `Table` | light |
| TableSelection | [Table.elyx](design/screens/library/Table.elyx) | `TableSelection` | light |
| BulkPartialFailure | [Table.elyx](design/screens/library/Table.elyx) | `BulkPartialFailure` | light |
| TableInlineStatus | [Table.elyx](design/screens/library/Table.elyx) | `TableInlineStatus` | light |
| TableInlineRating | [Table.elyx](design/screens/library/Table.elyx) | `TableInlineRating` | light |
| Board | [Board.elyx](design/screens/library/Board.elyx) | `Board` | light |
| BoardDragging | [Board.elyx](design/screens/library/Board.elyx) | `BoardDragging` | light |
| BoardColumnMenu | [Board.elyx](design/screens/library/Board.elyx) | `BoardColumnMenu` | light |
| BoardNewStatus | [Board.elyx](design/screens/library/Board.elyx) | `BoardNewStatus` | light |
| BoardRenameStatus | [Board.elyx](design/screens/library/Board.elyx) | `BoardRenameStatus` | light |
| Favorites | [Sections.elyx](design/screens/library/Sections.elyx) | `Favorites` | light |
| Collection | [Sections.elyx](design/screens/library/Sections.elyx) | `Collection` | light |
| SmartCollection | [Sections.elyx](design/screens/library/Sections.elyx) | `SmartCollection` | light |
| HiddenGames | [Sections.elyx](design/screens/library/Sections.elyx) | `HiddenGames` | light |
| Wishlist | [Wishlist.elyx](design/screens/library/Wishlist.elyx) | `Wishlist` | light |
| WishlistPriceLoading | [Wishlist.elyx](design/screens/library/Wishlist.elyx) | `WishlistPriceLoading` | light |
| WishlistPriceUnavailable | [Wishlist.elyx](design/screens/library/Wishlist.elyx) | `WishlistPriceUnavailable` | light |
| LibraryEmpty | [Feedback.elyx](design/screens/library/Feedback.elyx) | `LibraryEmpty` | light |
| SearchEmpty | [Feedback.elyx](design/screens/library/Feedback.elyx) | `SearchEmpty` | light |
| LibraryLoading | [Feedback.elyx](design/screens/library/Feedback.elyx) | `LibraryLoading` | light |
| LibraryError | [Feedback.elyx](design/screens/library/Feedback.elyx) | `LibraryError` | light |
| LibraryWriteIssue | [Feedback.elyx](design/screens/library/Feedback.elyx) | `LibraryWriteIssue` | light |
| DisplayMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `DisplayMenu` | light |
| FilterMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `FilterMenu` | light |
| SortMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `SortMenu` | light |
| GroupMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `GroupMenu` | light |
| GridViewMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `GridViewMenu` | light |
| GameContextMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `GameContextMenu` | light |
| StatusMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `StatusMenu` | light |
| CollectionMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `CollectionMenu` | light |
| WishlistSortMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `WishlistSortMenu` | light |
| WishlistContextMenu | [Menus.elyx](design/screens/library/Menus.elyx) | `WishlistContextMenu` | light |
| CollectionNew | [Collections.elyx](design/screens/library/Collections.elyx) | `CollectionNew` | light |
| CollectionRename | [Collections.elyx](design/screens/library/Collections.elyx) | `CollectionRename` | light |
| CollectionContextMenu | [Collections.elyx](design/screens/library/Collections.elyx) | `CollectionContextMenu` | light |
| CollectionSaveFailed | [Collections.elyx](design/screens/library/Collections.elyx) | `CollectionSaveFailed` | light |
| EditNote | [Collections.elyx](design/screens/library/Collections.elyx) | `EditNote` | light |
| CardFront | [CardFront.elyx](design/screens/details/CardFront.elyx) | `CardFront` | light |
| CardFrontNoFilmstrip | [CardFront.elyx](design/screens/details/CardFront.elyx) | `CardFrontNoFilmstrip` | light |
| CardDetails | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `CardDetails` | light |
| DetailsUnsaved | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsUnsaved` | light |
| DetailsSaving | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsSaving` | light |
| DetailsSaved | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsSaved` | light |
| DetailsSaveFailed | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsSaveFailed` | light |
| DetailsConflict | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsConflict` | light |
| DetailsReadOnly | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsReadOnly` | light |
| DetailsStatusMenu | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsStatusMenu` | light |
| DetailsActionsMenu | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsActionsMenu` | light |
| DetailsTagPicker | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsTagPicker` | light |
| DetailsFitInfo | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsFitInfo` | light |
| DetailsNoBestOn | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `DetailsNoBestOn` | light |
| SettingsGeneral | [General.elyx](design/screens/settings/General.elyx) | `SettingsGeneral` | light |
| SteamConnected | [General.elyx](design/screens/settings/General.elyx) | `SteamConnected` | light |
| SteamSyncing | [General.elyx](design/screens/settings/General.elyx) | `SteamSyncing` | light |
| SteamSyncFailed | [General.elyx](design/screens/settings/General.elyx) | `SteamSyncFailed` | light |
| SteamKeyTestFailed | [General.elyx](design/screens/settings/General.elyx) | `SteamKeyTestFailed` | light |
| SettingsBestOn | [BestOn.elyx](design/screens/settings/BestOn.elyx) | `SettingsBestOn` | light |
| SettingsBestOnEmpty | [BestOn.elyx](design/screens/settings/BestOn.elyx) | `SettingsBestOnEmpty` | light |
| SettingsProtonDbUpdating | [BestOn.elyx](design/screens/settings/BestOn.elyx) | `SettingsProtonDbUpdating` | light |
| SettingsProtonDbFailed | [BestOn.elyx](design/screens/settings/BestOn.elyx) | `SettingsProtonDbFailed` | light |
| SettingsAppearance | [Appearance.elyx](design/screens/settings/Appearance.elyx) | `SettingsAppearance` | light |
| SettingsOmarchy | [Appearance.elyx](design/screens/settings/Appearance.elyx) | `SettingsOmarchy` | light |
| AiLocal | [AI.elyx](design/screens/settings/AI.elyx) | `AiLocal` | light |
| AiCloud | [AI.elyx](design/screens/settings/AI.elyx) | `AiCloud` | light |
| AiTested | [AI.elyx](design/screens/settings/AI.elyx) | `AiTested` | light |
| AiKeyAdded | [AI.elyx](design/screens/settings/AI.elyx) | `AiKeyAdded` | light |
| SyncOff | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncOff` | light |
| SyncSampleUnavailable | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncSampleUnavailable` | light |
| SyncUpToDate | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncUpToDate` | light |
| SyncChecking | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncChecking` | light |
| SyncFolderUnavailable | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncFolderUnavailable` | light |
| SyncPending | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncPending` | light |
| SyncStopConfirmation | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncStopConfirmation` | light |
| SyncReviewChanges | [DeviceSync.elyx](design/screens/settings/DeviceSync.elyx) | `SyncReviewChanges` | light |
| KeySyncOff | [KeySync.elyx](design/screens/settings/KeySync.elyx) | `KeySyncOff` | light |
| KeySyncLocked | [KeySync.elyx](design/screens/settings/KeySync.elyx) | `KeySyncLocked` | light |
| KeySyncUnlocked | [KeySync.elyx](design/screens/settings/KeySync.elyx) | `KeySyncUnlocked` | light |
| KeySyncChanged | [KeySync.elyx](design/screens/settings/KeySync.elyx) | `KeySyncChanged` | light |
| KeySyncError | [KeySync.elyx](design/screens/settings/KeySync.elyx) | `KeySyncError` | light |
| ReviewGameVersions | [GameVersions.elyx](design/screens/review/GameVersions.elyx) | `ReviewGameVersions` | light |
| ReviewCollections | [Collections.elyx](design/screens/review/Collections.elyx) | `ReviewCollections` | light |
| GridDark | [Grid.elyx](design/screens/library/Grid.elyx) | `Grid` | dark |
| CardsDark | [Cards.elyx](design/screens/library/Cards.elyx) | `Cards` | dark |
| CardDetailsDark | [CardDetails.elyx](design/screens/details/CardDetails.elyx) | `CardDetails` | dark |
| SettingsAppearanceDark | [Appearance.elyx](design/screens/settings/Appearance.elyx) | `SettingsAppearance` | dark |
