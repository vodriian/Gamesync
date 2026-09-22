# GameSync desktop

A native Rust/GPUI library of physical game cards, built on Eagle foundations.

## Run

Install stable Rust. On macOS, install Xcode and its command-line tools.
On Linux, use the system dependencies required by GPUI 0.2.2.
macOS uses GPUI's runtime Metal shaders, so the separate Metal Toolchain is not required.

From the repository root:

```sh
python3 desktop/scripts/dev.py
```

The script builds the app. On macOS it also creates a development bundle at
`desktop/target/GameSync.app`. It is not a signed release package.
You can also run `cargo run --manifest-path desktop/Cargo.toml` directly.

Connect Steam in Settings. GameSync prepares its local store automatically;
there is no folder picker, Open Library command, or `--library` option.
An existing readable store from earlier versions is retained internally so
personal edits and Steam credentials keep their identity.

Use `--demo` for a separate persistent sample store. You can edit sample cards
and create collections without affecting your Steam library.

## This milestone

- Normal startup prepares your Steam store. Sample games require `--demo`.
- Refresh automatically after file changes, or use Command-R / Control-R.
- Filter by title, tag, status, or favorite.
- Use Cards, Grid, or Table. Click a game to open its card. Right-click for
  favorite, status, collection membership, Add/Edit note, and Hide/Unhide.
- Hidden games stay saved and sync normally. Enable **Show hidden games in sidebar**
  under Settings → Look and feel to browse and unhide them.
- Use Space to turn the card, arrows or the thumbnail strip to browse, and Escape to return.
- Use Command-F on macOS or Control-F on Linux to focus search.
- Filter with the left sidebar. Use the Collections plus button to create inline;
  right-click a collection to rename or remove it. Enter saves; Escape cancels.
  Drag games from Cards, Grid, or Table into a collection. The card-back chips
  remain available for membership changes. Settings stays in the sidebar.
  The toolbar button animates navigation over 200 ms; Reduce motion makes it immediate.
- Quit with Command-Q on macOS or Control-Q on Linux.
- The footer shows the last completed Steam sync. Game cards open on their front.

Bundled data is available only in explicit demo mode. In a real library, select a
game and turn to Details. Status, stars, favorite, and collection chips save
on click. Tags and notes save after a 450 ms pause. Click the
selected star again to clear the rating. Descriptions are read-only; existing overrides remain preserved. Save feedback stays visible below the fields.

Revision checks protect each write. A failed save keeps the draft and offers
Retry. External edits require review or explicit discard. Closing or switching
libraries waits for unsaved work. A forced exit can still lose unsaved text.
Table supports row selection and bulk favorite, status, collection, and hide actions. Select-all applies to the current filtered results. Sorting and column controls remain planned. Kanban and AI remain future work.

## Storage foundation

`src/storage.rs` writes a complete immutable JSON revision, then atomically
replaces the current file. It flushes file data and directory entries. A failed
current-file save leaves the revision available for recovery by an exact retry.
It uses `tempfile`, already present in Eagle's dependency lock, for safe staging.

`RecordStore` adds versioned game records, personal edits, archive/restore, and
explicit conflict resolution. It checks revision parents under a local writer
lock. It retains both branches and blocks stale edits. See the
[record format](../plan/storage-format.md) for its contracts and limits.

`LibraryStore` creates a new library folder and manages its name and ordered
status definitions. Definitions use the same revision service as games, so
concurrent changes are preserved. Status removal is disabled until game
reassignment is available.

`LibraryReader` connects the file services to the grid and inspector. It checks
status keys and keeps last-valid records in a device-local SQLite index outside
the library. Missing/partial files do not remove games. An explicit tombstone hides
a game. File issues block inspector saves until the source is valid. This first
editor uses a conservative library-wide block; missing covers do not block edits.
A stale game or manifest also blocks Save and keeps the draft. Discard explicitly
returns to current details. Use **Conflicts** in the toolbar to compare differing fields and keep one complete
saved version. Both branches remain in history. A late version or changed library
definition rejects the choice. Unsaved inspector drafts remain separate.
Library-definition conflicts still require service-level resolution.
Local publication does not confirm Dropbox upload.
Only macOS has been tested; Windows writes are disabled.

## Try sample games

```sh
python3 desktop/scripts/dev.py --demo
```

Sample games and their collections persist separately in app storage. The
`demo_library` and `card_geometry_library` examples remain storage fixtures for
service-level checks, not user-facing folder import features.

## Checks

```sh
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo test --manifest-path desktop/Cargo.toml
cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
```

Use `--empty`, `--missing-covers`, or `--stress` with the development script
to check empty content, image failure, or 10,000 records. Stress records share
the 12 bundled covers; this checks list scale, not a large unique-image working set.

## Source reuse

See [source notes](SOURCE-NOTES.md) for reused Eagle modules and cover sources.
See [the project plan](../plan/README.md) for scope and progress.


## Collections and Steam

Use **+** beside Collections in the sidebar to create, rename, or remove a group.
Change a game's collection chips on the card back. Removing a
collection preserves its games and membership history.

Open **Settings** from the sidebar or Command/Control-comma. General contains Steam
and sync; Look and feel contains all themes and Reduce motion; AI contains an interactive preview for Ollama, OpenAI, Claude, Grok, and Gemini.
Models come before keys. AI tests are simulated; keys are not saved or sent. Enter your masked profile and key, select
**Test key**, then **Save key** after success. Saved keys are reused on restart.
Changing either input requires another test. Use **Sync now** to update Steam. Cancel stops after the current
request; completed writes stay. Failed metadata stages retry on the next sync.

Keys use macOS Keychain or Linux Secret Service and stay outside the library.
Unlock secure storage if it is unavailable. Remove key appears on the right only
when a key is saved, and keeps your games. Linux requires D-Bus development libraries and an unlocked Secret
Service in addition to GPUI's dependencies; Linux runtime validation is pending.

Steam updates provider data while preserving ratings, notes, status, collections,
archived games, and personal description/cover overrides. Missing games are never
deleted. Duplicate App IDs and conflicts require review.

Select a Steam game and click **Refresh cover** on the card back to fetch
artwork again. This needs no API key. The resolver tries current Steam artwork,
the legacy portrait, then a store header. Blank gray placeholders are rejected.
A successful refresh uses a new media path; failure keeps the previous cover.
Personal cover overrides keep priority.

Optional integration checks (never part of the offline test suite):

```sh
cargo test --manifest-path desktop/Cargo.toml --test credentials native_store_round_trip_and_removal -- --ignored
cargo test --manifest-path desktop/Cargo.toml --test steam_network -- --ignored
```

The credential test writes and removes a dummy value under a random library UUID.
The network test reads public Steam metadata and needs no account or key.

## Card materials

On macOS, Cards and focused fronts use GPU perspective, satin lighting and
pointer tilt. Mouse and Space animate a full 180-degree turn. The settled back
uses the existing native editor. Reduce motion disables tilt and spatial turns.
Grid and Table stay still. Linux retains flat cards; Linux 3D is pending.

The pinned GPUI 0.2.2 patch lives in `vendor/gpui`; see its `GAMESYNC-PATCH.md`.
Run `--card-proof` for the isolated real-card renderer check. Use
`GAMESYNC_CARD_TRACE=1` to measure GPU time and additional texture memory.
Add `GAMESYNC_FRAME_TRACE=1` to measure native layout and paint CPU time.
These opt-in debug traces are diagnostics, not a release benchmark.

For repeatable native geometry checks, launch the built executable directly:

```sh
desktop/target/GameSync.app/Contents/MacOS/gamesync-desktop --demo --small-window
```

`--small-window` starts at 960 × 600.

## macOS release build

Run `python3 desktop/scripts/package_macos.py` on macOS (Python 3.11 or newer).
It builds the locked release profile for the host architecture, copies the
executable and icon into a standalone bundle, applies an ad-hoc signature, and
creates a ZIP under `desktop/target/macos/`. The app does not depend on the
checkout or development executable. Its minimum macOS version comes from the
linked binary. This is a local test build, not a universal or notarized release.
Developer ID signing and notarization remain required for public distribution.

### Appearance

Settings → Look and feel provides Auto/Light/Dark appearance, independent light
and dark Baseline schemes, contrast variants, and two accent switches. Defaults
are Sanctum light and Notion dark. Preferences apply across open windows and
persist on restart. See [chrome and themes](../plan/chrome-themes.md).
