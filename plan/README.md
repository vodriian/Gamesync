# GameSync desktop

A cozy game library for people who like to play games.

Track games. Browse covers. Find something that fits your mood, time, and energy.

## Start here

| Document | Purpose |
| --- | --- |
| [App plan](app-plan.md) | Scope, architecture, data rules, milestones, and acceptance checks |
| [Guardrails](guardrails.md) | Code quality, tests, writing, and efficient agent work |
| [Design](design.md) | Eagle GUI reference and design rules |
| [Latest log](logs/2026-10-07-native-ui-mode.md) | Native UI mode: macOS look, system accent, Settings form, book turn |
| [Play now delivery log](logs/2026-10-07-play-now-delivery.md) | Play now delivery, Elyx states, and final checks |
| [Book details log](logs/2026-10-05.md) | Best on demo prototype and book details prototype |
| [Sync log](logs/2026-09-29.md) | Sync implementation, initial live check, and library UI fixes |
| [UI log](logs/2026-09-28.md) | Omarchy theme, 3D card POC, Kanban board, and macOS UI fixes |
| [Foundation log](logs/2026-09-11.md) | Initial decisions, changes, checks, and next task |
| [Desktop demo](../desktop/README.md) | Run commands and current native features |
| [Source notes](../desktop/SOURCE-NOTES.md) | Eagle code reuse and demo artwork sources |
| [Record format](storage-format.md) | Versioned game records and conflict rules |
| [Kanban board](kanban-board.md) | Status board, manual order, and status editing |
| [Dashboard and smart collections](dashboard-tags.md) | Steam data, Smart collections, Home dashboard, and wishlist prices |
| [Best on](best-on.md) | Fit from Steam evidence, your rules, and optional ProtonDB data |
| [Play now](play-now.md) | Recommendation requirements, native offline implementation, and HTML reference |
| [Play now native log](logs/2026-10-06-play-now-native.md) | Native implementation, persistence, and validation |
| [Play now session log](logs/2026-10-07-play-now-sessions.md) | Timed sessions, card overlay, and updated test package |
| [Play now controls log](logs/2026-10-07-play-now-controls.md) | Preference motion and the Brain dead switch |
| [Play now prototype log](logs/2026-10-06-play-now.md) | Source findings, browser checks, and prototype limits |
| [Data sync](data-sync.md) | Personal data, settings, and encrypted keys across devices |

## Current state

- Documentation foundation: added.
- Desktop implementation: native Cards, Grid, Table, Board, and focused game details use app-managed Steam storage on macOS.
- Milestone 1: Linux runtime verification remains pending.
- Milestone 2: file records, definitions, folder loading, and a local index are in place. Inspector draft edits, guarded saves, and saved-game conflict review are implemented.
- Desktop v1 merged to `main` in PR #1 on September 28.
- Cross-platform view, board, settings, and wishlist fixes merged in PR #3.
- Prior branch: `data-sync-mode` (merged into `main` in PR #4) includes [data sync](data-sync.md) steps 1–5, library UI fixes, and the Elyx design workspace.
- Initial macOS/Omarchy Dropbox check is logged; full cross-device acceptance remains pending.
- Hugeicons UI merged in PR #5. Book details, Best on, and the shared Elyx workspace merged in PR #6.
- Use the [Elyx workspace](../Design/README.md) as the design source for the next UI improvements.
- Native offline Play now is implemented on `vova/play-now-native`; AI enrichment remains deferred.
- Native UI mode is on `native-ui-mode` (PR to `windows-test-build`): a native macOS look by default, with system colors and accent, kit window geometry, and a tabbed Settings form. See [native look](chrome-themes.md#native-look).
- Runtime targets: macOS and Linux. Windows follows after sync works between them.
- Existing React/Express app: retained as a reference.

## Current UI direction

[Physical cards](physical-cards.md): framed covers, front/back details, and
three library views, restored left navigation, and native Metal perspective.
Linux keeps flat cards outside Omarchy mode. The Omarchy branch has an
experimental Blade/Vulkan card projection for Cards and focused fronts.

## Next task

Test `native-ui-mode` with real data (test build `GameSync Native`), then add
the missing tests (`Look` migration, native palette keys) and the remaining
items in the [native UI log](logs/2026-10-07-native-ui-mode.md#not-verified--next).

Review and test `vova/play-now-native`. It adds offline recommendations,
editable game profiles, modal details, timed sessions, and sliding preference
controls. The PR targets `windows-test-build`, which supplies the earlier
Windows and AI Settings work. AI recommendation enrichment is a later step.
See [Play now](play-now.md) and the
[Elyx states](../Design/design/screens/play-now/PlayNow.elyx).

Complete [data sync](data-sync.md) step 6 acceptance checks across macOS and
Omarchy, including Dropbox and Google Drive. Windows and compaction remain
later steps. The offline picker now follows [Play now](play-now.md). AI enrichment remains deferred.

Collections, core Settings, and Steam sync are implemented. See
[Collections, Settings, and Steam](collections-settings-steam.md) for use and limits.
Run an account sync with a key entered in Settings. Verify Linux UI and secure
storage before cross-platform release. Then validate the offline Play now profiles and add opt-in AI enrichment.

## Maintain these documents

Keep each decision in one document. Link to it from other documents.
Update current state when a milestone changes. Add short dated logs under
`logs/YYYY-MM-DD.md`; use task headings for multiple entries on one day.
Keep screenshots and large test output outside Git unless they are needed
as a small, stable project reference. Do not commit personal game libraries.

- [Motion and interaction review](motion-review.md): Eagle reuse, scope, and native limits.
- [Chrome and Baseline themes](chrome-themes.md): inset native shell, palette catalog, contrast, and migration.
- [Chrome/theme implementation log](logs/2026-09-22-chrome-themes.md): validation and platform limits.
