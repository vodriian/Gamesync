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

## Current state

- Documentation foundation: added.
- Desktop implementation: native demo built and checked on macOS.
- Milestone 1: Linux runtime verification remains pending.
- Milestone 2: revision file publication and recovery tests added. Editor integration is pending.
- Working branch: `codex/desktop-foundation`.
- Runtime targets: macOS and Linux. Windows follows later.
- Existing React/Express app: retained as a reference.

## Next task

Verify the native demo on Linux before adding more screens. Storage work
can proceed separately: add versioned records and revision conflict checks
before connecting the file layer to personal edits. The current demo uses
read-only sample data; it does not save game edits or preferences.

## Maintain these documents

Keep each decision in one document. Link to it from other documents.
Update current state when a milestone changes. Add short dated logs under
`logs/YYYY-MM-DD.md`; use task headings for multiple entries on one day.
Keep screenshots and large test output outside Git unless they are needed
as a small, stable project reference. Do not commit personal game libraries.
