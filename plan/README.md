# GameSync desktop

A cozy game library for people who like to play games.

Track games. Browse covers. Find something that fits your mood, time, and energy.

## Start here

| Document | Purpose |
| --- | --- |
| [App plan](app-plan.md) | Scope, architecture, data rules, milestones, and acceptance checks |
| [Guardrails](guardrails.md) | Code quality, tests, writing, and efficient agent work |
| [Design](design.md) | Eagle GUI reference and design rules |
| [Latest UI log](logs/2026-09-22.md) | Physical card views, save checks, and rendering limits |
| [Foundation log](logs/2026-09-11.md) | Initial decisions, changes, checks, and next task |
| [Desktop demo](../desktop/README.md) | Run commands and current native features |
| [Source notes](../desktop/SOURCE-NOTES.md) | Eagle code reuse and demo artwork sources |
| [Record format](storage-format.md) | Versioned game records and conflict rules |

## Current state

- Documentation foundation: added.
- Desktop implementation: native Cards, Grid, Table, and focused game details use app-managed Steam storage on macOS.
- Milestone 1: Linux runtime verification remains pending.
- Milestone 2: file records, definitions, folder loading, and a local index are in place. Inspector draft edits, guarded saves, and saved-game conflict review are implemented.
- Working branch: `codex/desktop-foundation`.
- Runtime targets: macOS and Linux. Windows follows later.
- Existing React/Express app: retained as a reference.

## Current UI direction

[Physical cards](physical-cards.md): framed covers, front/back details, and
three library views, restored left navigation, and native Metal perspective.
Linux keeps flat cards; Linux 3D remains pending.

## Next task

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
