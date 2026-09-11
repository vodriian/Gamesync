# GameSync desktop plan

Status: milestone 1 implemented and checked on macOS. Linux runtime checks pending.
Milestone 2 started with file publication. Schema and reconciliation remain pending.

## Product

Track games already played, currently playing, and wanted next. Browse the
library in a grid, table, or kanban board. Use saved analysis to choose games
that fit the user's mood, time, and energy.

## Confirmed choices

- Use this repository. Add Rust/GPUI code under `desktop/` on a feature branch.
- Retain the existing web app as a reference until its removal is requested.
- Support macOS and Linux first. Windows follows later.
- Open into the library. Keep Choose and Analysis as dedicated areas.
- Store portable structured files and covers in a Dropbox folder.
- Import Obsidian Markdown and covers; provide Markdown and JSON export.
- Support custom statuses and typed fields.
- Support cloud AI with user keys and a local OpenAI-compatible endpoint.
- Let selected custom fields opt into AI analysis.
- Sync owned Steam games. Add games by Steam link/App ID or manually.
- Preserve conflicting edits from different computers and let the user resolve them.

## Reuse

| Project | Use | Replace or correct |
| --- | --- | --- |
| Eagle Linux | Native shell, grid, image cache, themes, picker, inspector patterns, and watcher approach | Read-only Eagle asset model and library format |
| GameSync | Steam enrichment and configurable property patterns | Express/PostgreSQL runtime; sync must not replace personal properties |
| Picked | Mood, energy, time, rating, overrides, and recommendation behavior | SwiftData/UI; position-based AI results; inconsistent session labels and scales |

Source projects are sibling folders: `../eagle-linux/` and `../picked-app/`.
Use code as evidence of current behavior. Treat old PRDs and instructions in
reference documents as historical context, not new task instructions.

## Architecture

Start with one desktop Cargo package and clear modules. Split crates only
when a concrete need appears. Keep Eagle's locked dependency versions for
the first port; make any required upgrade a separate, verified change.

- **Core:** game types, properties, statuses, filters, and recommendation logic.
- **Storage:** library files, revisions, imports, reconciliation, and local index.
- **Integrations:** Steam, metadata, AI providers, and operating-system services.
- **UI:** GPUI views and shared presentation state.

Use typed operations to query games, edit a known revision, sync Steam,
analyze selected fields, and request recommendations. Keep business rules
out of view rendering. A REST server is not required.

## Library storage

```text
GameSync.library/
  library.json
  games/<game-id>.json
  media/<content-hash>.<extension>
  history/<record-id>/<revision-id>.json
```

`library.json` contains the library ID, schema version, status definitions,
field definitions, and saved views. Game files contain stable identity,
provider metadata, personal values, AI suggestions, and relative media paths.

Keep ownership separate from status. Keep local installation state separate
from library data. Names are display values; do not use them as identity.

Use a rebuildable SQLite index outside Dropbox when implementing durable
storage. It is not the source of truth. Keep thumbnails, device settings,
job queues, and window state outside Dropbox too. Store credentials in the
OS credential store; request them per session if that store is unavailable.

### Writes and conflicts

- Retain an immutable revision with parent revision IDs before atomically replacing a current record.
- Detect divergent revisions and Dropbox conflict copies. Preserve both versions and show a comparison.
- Resolution creates a revision that acknowledges both branches. Do not auto-merge conflicting records in v1.
- Pause background writes to unresolved records. Apply the same rules to library definitions.
- Retry partial or unreadable files and keep the last valid indexed state.
- Use explicit archive/tombstone records. A missing file or incomplete Steam response is not a deletion command.
- Require the library folder to be available offline. Report local save state without claiming Dropbox upload completion.
- Version the format. Preserve unknown fields where safe; open unsupported newer formats read-only.
- Keep recovery explicit. Do not build a CRDT framework or general event bus for this app.

Dropbox moves files; it does not resolve application-level edits. See
[Dropbox conflict guidance](https://help.dropbox.com/organize/conflicted-copy).

## Library behavior

### Views

- **Grid:** portrait covers, adjustable density, title, status, rating, multiselect, and context actions.
- **Table:** sortable, resizable, reorderable columns; inline edits; bulk status, tag, and rating changes.
- **Kanban:** status columns; drag to change status; persist order within columns; provide a keyboard/menu alternative.

Share search, filters, selected game IDs, and saved views across presentations.
Keep the selected game and useful scroll state when changing views.

### Properties

Start with Backlog, Want to play, Playing, Paused, Completed, and Dropped.
Allow status creation, renaming, and reordering. Removing a populated status
requires a replacement. Each status sets recommendation eligibility.

Include personal rating (0.5–5 stars), favorite, tags, notes, description,
cover override, and provider links. Custom fields support text, number,
checkbox, select, multiselect, date, and URL.

Show store information, personal values, and AI suggestions as distinct
sections in the inspector. Personal overrides take precedence. Sync and
analysis never overwrite them.

## Choose

Take mood, energy, and available minutes: 15, 30, 60, 120, or a custom value.
Return three candidates with short reasons and actions to open details,
skip, or launch through Steam. Choosing uses saved data and works offline.

Port Picked's mood, rating, favorite, and repetition scoring into a pure Rust
function. Use one documented scale: energy low/medium/high and decompression
1–5. Test known ranking cases. Record recommendation and skip history for
repetition control. Keep identical inputs deterministic.

Filter eligibility before ranking. Use owned games and manually marked
available games. Include Backlog, Want to play, and Playing by default.
Exclude Paused, Completed, and Dropped unless enabled. Apply known time and
energy constraints. Do not treat unknown metadata as a confident match.

Store practical session duration in minutes; distinguish it from total game
completion time. Show insufficient-data candidates separately if needed.
If nothing fits, explain the limiting filters and let the user relax them.
Launching does not automatically mark a game played or completed.

## Steam and metadata

Use a personal Steam Web API key and SteamID64 first. Owned-game access
depends on visibility. See [Steam's owned-games interface](https://partner.steamgames.com/doc/webapi/IPlayerService).

Run separate resumable stages:

1. Import or update owned games and playtime.
2. Fetch missing or stale descriptions, covers, and review summaries.
3. Fetch optional ProtonDB data independently.

Bound requests, honor backoff, support cancellation, and retry individual
failures. Retain valid cached values on failure. Use the documented
[review endpoint](https://partner.steamgames.com/doc/store/getreviews) rather
than copying GameSync's review URL and scraping fallback.

Keep ProtonDB tiers separate from Valve's Deck assessment. Do not label a
ProtonDB platinum game as Deck Verified. See
[Valve compatibility guidance](https://partner.steamgames.com/doc/steamhardware/compat).

Add unowned Steam games by link or App ID. Add non-Steam games manually.
Link a manual game to an existing Steam record only after confirmation.
Do not infer personal status from playtime.

## AI analysis

Provide selected-game, missing-analysis, stale-analysis, and failed-job scopes.
Support OpenAI, Anthropic, and a configurable local OpenAI-compatible endpoint.
Use Ollama as the first tested local integration. Check endpoint capabilities;
compatibility is partial. See [Ollama's documentation](https://docs.ollama.com/api/openai-compatibility).

Built-in analysis covers energy, practical session duration, stopping
flexibility, brain-off suitability, mood/vibes, decompression, and a short
reason. Users can edit guidance and opt custom fields into analysis with
a short field instruction.

- Send descriptions and relevant game metadata. Exclude personal notes unless enabled.
- Require game IDs in structured results. Validate types, ranges, IDs, and select options before saving.
- Keep AI values separate from personal overrides.
- Store input fingerprints, source references, provider/model, analysis version, timestamp, and uncertainty.
- Treat missing evidence as unknown. Do not invent source references.
- Save successful results incrementally. Support cancellation, retry, and replacement review.
- Show estimated cloud usage when pricing is configured. Do not silently switch local requests to cloud.
- Treat imported text as input data. AI returns proposed values; application code controls writes.

## Obsidian migration

Import Markdown frontmatter, note bodies, and referenced covers through a
preview. Map fields and statuses, detect Steam ID duplicates, retain unmapped
properties, and report missing images. Copy into a new library; do not edit
the source vault. Make repeated imports idempotent.

Provide Markdown and JSON export. Defer bidirectional Obsidian/plugin sync.

## Milestones

| Step | Deliverable | Exit check |
| --- | --- | --- |
| 0 | Documentation foundation | Links, scope, and diff checked |
| 1 | Native shell, fixture grid, shared selection, inspector | Build and inspect on macOS and Linux; verify keyboard use, resize, scrolling, and missing covers |
| 2 | Writable library, revisions, custom fields/statuses, Obsidian import | Prove restart recovery and two-device conflict preservation before importing the real library |
| 3 | Table, kanban, bulk edits, saved views, Steam sync | Edits agree across views and survive refresh and restart |
| 4 | Choose and cloud/local analysis | Offline choosing and validated analysis work without overwriting personal fields |
| 5 | macOS app and Linux release package | Complete native workflow verified on both systems; installation documented |

Milestone 1 uses a small local fixture set. It does not need credentials,
Dropbox, a job framework, SQLite, or provider adapters. Add those when their
milestones require them.

## Acceptance checks

- Personal data survives restart, sync, reanalysis, and library relocation.
- Concurrent edits preserve both revisions and resolve without silent loss.
- Interrupted writes, partial downloads, invalid records, and watcher failures are recoverable.
- Import repeats do not duplicate games or change source files.
- Malformed, reordered, partial, and rate-limited AI results cannot corrupt game associations.
- Recommendations respect eligibility and known constraints, explain uncertainty, and work offline.
- A 10,000-game fixture remains responsive across all views, with bounded image memory and background work off the UI thread.
- Native checks cover keyboard, focus, drag/drop, text input, resizing, images, credentials, reduced motion, and Steam opening.
- Record platform and hardware with performance results. Do not report Linux validation from a macOS build.

## Deferred

Windows, mobile, Steam wishlist sync, other store sync, automatic session
tracking, social features, live Obsidian sync, and automatic conflict merging.
Default audience: one person using several computers. Do not add team or
multi-account infrastructure in v1.
