# GameSync design workspace

Editable references for UI improvements to the Rust/GPUI desktop app.
Use bundled sample content. See the [state index](screens/Desktop-states.md)
for all 90 states and the limits of the reconstruction.

## Open a canvas

- [Project overview](index.elyx): main screen, Settings, and component examples.
- [Main screen](screens/Desktop.elyx): the default Grid window.
- [Home](flows/Main.elyx) and [library views](flows/LibraryViews.elyx).
- [Settings](flows/Settings.elyx), [device sync](flows/DeviceSync.elyx), and [key sync](flows/KeySync.elyx).
- [Game details](flows/GameDetails.elyx) and [conflict review](flows/ConflictReview.elyx).
- [Components](flows/Components.elyx): shared controls, cards, menus, and shells.
- [All states](flows/AllStates.elyx): the complete original atlas.

Elyx folder galleries group Settings, views, states, components, and assets.
Overview canvases use instances of the canonical files; edit those source files
to change every overview that shows them.

## Where to edit

| Folder | Purpose |
| --- | --- |
| `tokens/` | Semantic light/dark colors, font values, and corner radii |
| `controls/` | Buttons, text fields, and window controls |
| `blocks/` | Navigation, window shells, library cards, rows, and menus |
| `screens/main/` | Home and its disconnected state |
| `screens/library/` | Views, sections, menus, board, collections, and other states |
| `screens/details/` | Focused card front/back and editing states |
| `screens/settings/` | General, appearance, AI, device sync, and key sync |
| `screens/review/` | Conflict review windows |
| `screens/appearance/` | Dark instances of the light sources |
| `flows/` | Focused comparison canvases and the full atlas |
| `resources/` | Local sample images and SVG icons |

Imports follow `tokens/assets → controls → blocks → screens → flows`.
Screens can derive from other screens. Keep shared elements below screens and
avoid circular imports. Use project-root import paths and explicit source paths
in cross-file overrides. See [AGENTS.md](AGENTS.md) for editor compatibility.

The organization follows the read-only reference at
`/Users/vova/Developer/sample-project`: focused files, shared controls, variants,
type-specific tokens, and folder galleries. GameSync keeps its own UI and assets.
