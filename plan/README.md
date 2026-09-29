# GameSync desktop

A cozy game library for people who like to play games.

Track games. Browse covers. Find something that fits your mood, time, and energy.

## Start here

| Document | Purpose |
| --- | --- |
| [App plan](app-plan.md) | Scope, architecture, data rules, milestones, and acceptance checks |
| [Guardrails](guardrails.md) | Code quality, tests, writing, and efficient agent work |
| [Design](design.md) | Eagle GUI reference and design rules |
| [Latest log](logs/2026-09-28.md) | Omarchy theme integration, 3D card POC, and Kanban board |
| [Latest UI log](logs/2026-09-28.md) | Current macOS and cross-platform UI fixes and checks |
| [Foundation log](logs/2026-09-11.md) | Initial decisions, changes, checks, and next task |
| [Desktop demo](../desktop/README.md) | Run commands and current native features |
| [Source notes](../desktop/SOURCE-NOTES.md) | Eagle code reuse and demo artwork sources |
| [Record format](storage-format.md) | Versioned game records and conflict rules |
| [Kanban board](kanban-board.md) | Status board, manual order, and status editing |
| [Dashboard and smart collections](dashboard-tags.md) | Steam data, Smart collections, Home dashboard, and wishlist prices |

## Current state

- Documentation foundation: added.
- Desktop implementation: native Cards, Grid, Table, Board, and focused game details use app-managed Steam storage on macOS.
- Milestone 1: Linux runtime verification remains pending.
- Milestone 2: file records, definitions, folder loading, and a local index are in place. Inspector draft edits, guarded saves, and saved-game conflict review are implemented.
- Desktop v1 merged to `main` in PR #1 on September 28.
- Working branch: `codex/macos-view-board-wishlist-fixes` (from `main`) for the
  macOS build and cross-platform view, board, settings, and wishlist fixes.
- Runtime targets: macOS and Linux. Windows follows later.
- Existing React/Express app: retained as a reference.

## Current UI direction

[Physical cards](physical-cards.md): framed covers, front/back details, and
three library views, restored left navigation, and native Metal perspective.
Linux keeps flat cards outside Omarchy mode. The Omarchy branch has an
experimental Blade/Vulkan card projection for Cards and focused fronts.

## Next task

Verify these cross-platform fixes on Linux, then continue the deferred tuning
fields and offline feeling/time picker in [Dashboard and smart collections](dashboard-tags.md).

Collections, core Settings, and Steam sync are implemented. See
[Collections, Settings, and Steam](collections-settings-steam.md) for use and limits.
Run an account sync with a key entered in Settings. Verify Linux UI and secure
storage before cross-platform release. Then add tuning fields and the offline
feeling/time picker. The user moved collections and Steam ahead of that work.

## Maintain these documents

Keep each decision in one document. Link to it from other documents.
Update current state when a milestone changes. Add short dated logs under
`logs/YYYY-MM-DD.md`; use task headings for multiple entries on one day.
Keep screenshots and large test output outside Git unless they are needed
as a small, stable project reference. Do not commit personal game libraries.

- [Motion and interaction review](motion-review.md): Eagle reuse, scope, and native limits.
- [Chrome and Baseline themes](chrome-themes.md): inset native shell, palette catalog, contrast, and migration.
- [Chrome/theme implementation log](logs/2026-09-22-chrome-themes.md): validation and platform limits.
