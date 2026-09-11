# GameSync desktop

A cozy game library for people who like to play games.

Track games. Browse covers. Find something that fits your mood, time, and energy.

## Start here

| Document | Purpose |
| --- | --- |
| [App plan](app-plan.md) | Scope, architecture, data rules, milestones, and acceptance checks |
| [Guardrails](guardrails.md) | Code quality, tests, writing, and efficient agent work |
| [Design](design.md) | Eagle GUI reference and design rules |
| [Foundation log](logs/2026-09-11.md) | Initial decisions, changes, checks, and next task |
| [Desktop demo](../desktop/README.md) | Run commands and current native features |
| [Source notes](../desktop/SOURCE-NOTES.md) | Eagle code reuse and demo artwork sources |
| [Record format](storage-format.md) | Versioned game records and conflict rules |

## Current state

- Documentation foundation: added.
- Desktop implementation: native grid and inspector load real library folders on macOS.
- Milestone 1: Linux runtime verification remains pending.
- Milestone 2: file records, definitions, folder loading, and a local index are in place. Inspector draft edits and guarded saves are implemented.
- Working branch: `codex/desktop-foundation`.
- Runtime targets: macOS and Linux. Windows follows later.
- Existing React/Express app: retained as a reference.

## Next task

Verify the native demo on Linux before adding more screens. Storage work
can proceed separately. Next, add a small conflict review inside the inspector. It must show the saved alternatives and keep
both branches in history. No automatic merge or overwrite.

## Maintain these documents

Keep each decision in one document. Link to it from other documents.
Update current state when a milestone changes. Add short dated logs under
`logs/YYYY-MM-DD.md`; use task headings for multiple entries on one day.
Keep screenshots and large test output outside Git unless they are needed
as a small, stable project reference. Do not commit personal game libraries.
