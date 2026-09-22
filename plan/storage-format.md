# Game records, version 1

This is the first storage format. The native app can load these files read-only.
Library manifests and status definitions are implemented. Custom field definitions
and AI write operations are pending.

## Identity and data

- `game_id` and `revision_id` are UUIDs. Steam App ID is a provider link, not game identity.
- `parents` lists the revisions an edit replaces. A new game has no parents.
- `game` separates title, Steam data, and personal values.
- Personal data contains status, rating, favorite, hidden, tags, notes, and optional description/cover overrides.
- Rating uses half-star units from 1 to 10. `null` means unrated.
- A `null` description uses Steam text. An empty string is an intentional blank.
- Status is a stable key, initially `backlog`. The folder reader checks game keys against the manifest.
- Cover paths use `/` within `media/`. Absolute paths and `..` are rejected.
- `deleted` is a reversible tombstone. Records and their history stay on disk.
- Unknown fields in version 1 survive edits. Unsupported versions block writes; their files stay unchanged.

## Saving and conflicts

Current records live at `games/<game-id>.json`. Immutable revisions live at
`history/<game-id>/<revision-id>.json`. A head is a revision with no known child.

| Scan result | Behavior |
| --- | --- |
| One head, valid complete history | Allow an edit only from that head |
| Several heads | Block ordinary edits; require an explicit branch choice |
| Missing parent, partial file, wrong identity, or unsupported version | Keep valid history available and block writes |
| Missing current file, valid history | Read the history head; do not infer deletion |
| Same revision ID with different content | Keep files intact and report an issue |

Resolution retains all known branches, chooses one head's values, and creates a
new revision with every reviewed head as a parent. If the heads change before
publication, reject the choice and request another review. No automatic merge.

An OS file lock prevents simultaneous local writers from accepting the same
parent. It cannot lock another computer or Dropbox. Late remote revisions become
conflicts when their files arrive. Every conforming writer must publish history
before its current file. Do not remove history or conflict copies by hand.

## Current limits

- `LibraryStore` creates a new offline library folder. Existing folders are never overwritten. `RecordStore` remains a lower-level service; it does not check status membership yet.
- Folder scans discover games from history folders and JSON record identity, including renamed copies. Game save checks use the same discovery. Temporary and lock files are ignored.
- Each JSON record is limited to 1 MiB. Covers remain separate files.
- A failed scan reports an error and retains last-valid data in a device-local SQLite index. File changes trigger a bounded, debounced refresh; a 10-second scan also retries missed events and unavailable folders.
- Conflict comparison and resolution are service operations; no conflict screen exists yet.
- macOS tests use temporary local folders. Live Dropbox synchronization and Linux behavior remain unverified.
- The index lives in the app cache directory, outside the library, and is bound to the resolved folder path. Deleting it rebuilds from valid source files; cached-only data cannot be reconstructed from missing source files. Corrupt-index errors are reported; an in-app rebuild control is pending.
- The last opened folder is stored in device-local settings. Appearance and selection are not persisted yet.

## Library definitions

`library.json` contains schema version, library UUID, revision UUID, parents, and
`definitions`. Its immutable revisions live in `history/library/`. Games and
library definitions share the same file publication and conflict checks.

Definitions contain the library name, default status key, and ordered statuses.
Each status has a stable key, label, and recommendation eligibility. Start with
Backlog, Want to play, Playing, Paused, Completed, and Dropped. The first three
are eligible for recommendations; ownership filtering comes later.

Edits can add statuses, change labels, reorder them, change eligibility, and choose
a defined default. Keys use 1–64 lowercase ASCII letters, digits, underscores,
or hyphens. Labels must not be blank. Duplicate keys and an undefined default
are rejected. Unknown supported fields survive edits.

Status removal is disabled until game reassignment is available. Conflict
resolution must retain all existing keys, including keys added on another
branch. The user supplies the reviewed definitions; the service does not merge
branches automatically. Changing labels leaves game files untouched.

Creation requires a new directory with an existing parent. It creates `games/`,
`media/`, and `history/`. Failed initialization leaves its partial directory for
inspection. Opening and inspecting do not repair or rewrite files. A missing
current manifest can be read from valid history and republished by a guarded edit.

## Collections and Steam additions

These additive schema-1 fields have defaults. Older records remain readable and
are not rewritten merely to add empty fields. Preserve extension properties.

- Library definitions: `collections` contains stable UUIDs, names, and an
  `archived` flag. Names are unique among active collections. Removed definitions
  stay archived; game membership is not rewritten.
- Personal game data: `collections` is a list of collection UUIDs. Membership
  uses the inspector's guarded personal-data write. Archived membership remains
  recoverable but is hidden from browsing.
- Library definitions: optional `steam_account` binds one Steam account. Device
  disconnect does not change this binding. Use another library for another account.
- Steam data: optional `metadata` stores genres, release year, review label and
  percent, provider cover path, and separate completion flags for details, reviews,
  and covers. Personal descriptions and covers take priority.
- New Steam records use a UUID derived from library UUID and Steam App ID.
  Existing matching game IDs remain unchanged. Ambiguous matches block that import.
- Provider writes keep the latest personal state and archived flag under the
  existing record lock. Missing Steam results do not remove or un-own games.
- A local sync lock prevents two processes from syncing the same folder at once.
  Remote Dropbox writers can still create branches; retain them for review.

Credentials, appearance, and last successful sync times stay on the device.
Keys use macOS Keychain or Linux Secret Service. They never enter library files.

## Hidden games

Personal `hidden` is an additive boolean with a default of false. It controls
browsing only; it is separate from the `deleted` tombstone. Steam sync preserves
it with other personal fields. Hidden games remain loaded so the Hidden games
scope can show and unhide them. The sidebar visibility preference is device-local.
