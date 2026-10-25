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

### Windows first build

Install stable Rust (the MSVC toolchain), Git, and Visual Studio 2022 Build Tools.
Select **Desktop development with C++**, including MSVC x64 tools and a Windows
SDK. The SDK supplies `fxc.exe` for GPUI's release shaders.

From PowerShell at the repository root:

```powershell
& .\desktop\scripts\windows.ps1 -Command build
& .\desktop\target\debug\gamesync-desktop.exe
```

The script loads the installed MSVC and SDK environment for this process.
Use `-Release` to build `desktop/target/release/gamesync-desktop.exe`.
Use `-Command test`, `-Command fmt`, or `-Command clippy` for checks.
Builds use `Cargo.lock`. `-UpdateLock` is only for a deliberate dependency change.
The executable includes its artwork and UI assets. It is a local test build;
an installer and public release signing remain pending.
Other PCs need the Microsoft Visual C++ x64 runtime. The first-build copy is
`desktop/target/windows/GameSync.exe`.

### Steam sync logs

Each Steam sync builds one full library identity index for owned and wishlisted
games. It still checks filenames and rereads the target game's current revisions
before a write, so new conflict copies and personal edits remain protected.
It does not reread every game's contents for each match.

Progress shows new, updated, and unchanged games. Updated games have changed
ownership, playtime, activity, or wishlist data; personal edits do not affect
these counts. Missing details count only pending metadata steps (description,
reviews, covers, Steam Deck data, and tags), including retries. Completed steps
are skipped. The final message reports how many detail steps are still missing.

Steam sync writes one JSON Lines log per run, after it reads the saved key.
The final sync message includes its path. On Windows the folder is
`%LOCALAPPDATA%\GameSync\GameSync\data\logs\steam-sync\`.
Other platforms use the GameSync local data directory under `logs/steam-sync/`.
Preview runs use their isolated storage folder.

Logs include UTC Unix timestamps in milliseconds, app version and platform,
sync stages, Steam App IDs, request timings, HTTP status, content type,
Retry-After, errors, and the final result. Request queries, response bodies,
API keys, account IDs, and personal game fields are excluded. Errors can
include game titles and local file paths. Logs stay on this device.

The latest 10 logs are retained, with a 2 MiB limit per run. Active logs are
kept until a later cleanup. A size-limit marker and final result remain when
details reach the limit. On Windows, open the log after the run ends.
Logging failures do not stop sync; the final message reports them.

Windows builds embed `icon/app-icon.ico` as resource ID 1, which GPUI uses for
the window and taskbar icon. It is derived from `icon/app-icon.png` at seven
sizes from 16 to 256 px. After changing the PNG, run
`& .\desktop\scripts\make_windows_icon.ps1` before rebuilding.

Windows keeps keys in Credential Manager. Local library writes use flushed
staging files and write-through renames. Windows has no equivalent of the Unix
directory flush used here; power-loss durability depends on the filesystem and
device. Steam installation detection and game launching still need Windows support.

Connect Steam in Settings. GameSync prepares its local store automatically;
there is no folder picker, Open Library command, or `--library` option.
An existing readable store from earlier versions is retained internally so
personal edits and Steam credentials keep their identity.

Use `--demo` for a separate persistent sample store. You can edit sample cards
and create collections without affecting your Steam library.

Use `--play-now-demo` for the recommendation preview. On macOS it creates
`desktop/target/PlayNowDemo.app`, which always opens isolated sample data even
when opened without arguments. Settings, cache, and the editable demo library
live under the system temporary directory in `gamesync-play-now-demo/`.
No game is launched and no account sync runs from this preview.

## This milestone

- Normal startup prepares your Steam store. Sample games require `--demo`.
- **Play now**, below Home, deals up to three games using time, energy, activity,
  Brain dead, and optional device/scope filters. Hidden and unowned games are
  excluded. One reshuffle is available per app session. You can correct game
  profiles, save picks, exclude recommendations, and review recent choices.
  It works offline with estimates and personal corrections. AI enrichment is
  not connected yet. See [Play now](../plan/play-now.md).
- Refresh automatically after file changes, or use Command-R / Control-R.
- Filter by title, tag, status, or favorite.
- Use Grid (the default), Cards, Table, or Board. Click a game to open its card. Right-click for
  favorite, status, collection membership, Add/Edit note, and Hide/Unhide.
- Board shows the current sidebar scope as one column per status. Drag a card to
  another column to change its status, or onto a card to place it before that
  card. The order is saved per game and syncs. Alt with arrow keys moves the
  selected game. Use **Add status** and the column menu to create, rename, and
  reorder statuses. Status removal is not available yet.
- Hidden games stay saved and sync normally. Enable **Show hidden games in sidebar**
  under Settings → Look and feel to browse and unhide them.
- Use Space to turn the card, arrows or the thumbnail strip to browse, and Escape to return.
- Use Command-F on macOS or Control-F on Linux to focus search.
- Filter with the left sidebar. Use the Collections plus button to create inline;
  right-click a collection to rename or remove it. Enter saves; Escape cancels.
  Drag games from Cards, Grid, Table, or Board into a collection. The card-back chips
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
Table supports row selection and bulk favorite, status, collection, and hide actions. Select-all applies to the current filtered results. Sorting and column controls remain planned. AI remains future work.

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
Windows writes are enabled for the first Windows build. See the platform checks
in [the project log](../plan/logs/2026-10-11-windows-first-run.md).

## Try sample games

```sh
python3 desktop/scripts/dev.py --demo
```

Sample games and their collections persist separately in app storage. The
`demo_library` and `card_geometry_library` examples remain storage fixtures for
service-level checks, not user-facing folder import features.

## Best on

Real libraries get Best on from Steam evidence after a Steam sync: Valve's
Steam Deck rating, controller support, Steam Workshop, and where you played.
Set your rules in **Settings → Best on**: your PC, tags that prefer PC or
Steam Deck, and equipment that makes games PC only. ProtonDB Steam Deck
reports are optional; turning them on downloads about 70 MB once a month.
On a card, (i) shows how each fit is calculated.

```sh
python3 desktop/scripts/dev.py --best-on-demo
```

Opens a demo store with saved example values; it is recreated on each launch.
On macOS it has its own `BestOnDemo.app` bundle ID. See [Best on](../plan/best-on.md).

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
and sync; Look and feel contains all themes and Reduce motion; AI lists added providers (OpenAI, Claude, Grok, Gemini, and Ollama) with a model choice each; the info button replaces or deletes a key.
AI keys are saved in the OS credential store. AI tests and model lists are still simulated, and no AI requests are sent. For Steam, enter your masked profile and key, select
**Test key**, then **Save key** after success. Saved keys are reused on restart.
Changing either input requires another test. Use **Sync now** to update Steam. Cancel stops after the current
request; completed writes stay. Failed metadata stages retry on the next sync.

The refresh button before Settings in the sidebar starts the same Steam sync.
It rotates while the job runs. Reduce motion keeps the icon still and uses the
accent color to show activity. Open Settings to cancel. If Steam is not connected,
the button opens General settings. Sample libraries cannot sync with Steam.

When Steam sync ends, a toast appears at the bottom-right of the main window.
It shows completion, cancellation, retry/review items, or an error. Toasts can
be dismissed with the close button. Hover pauses their five-second timer; the
timer also pauses while the main window is inactive.

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
Grid and Table stay still. Linux retains flat cards outside Omarchy mode.
Omarchy mode has an experimental Blade/Vulkan implementation for Cards and
focused fronts. Reduce motion disables it.

The pinned GPUI 0.2.2 patch lives in `vendor/gpui`; see its `GAMESYNC-PATCH.md`.
Run `--card-proof` for the isolated real-card renderer check. Use
`GAMESYNC_CARD_TRACE=1` to measure GPU time and additional texture memory.
Add `GAMESYNC_FRAME_TRACE=1` to measure native layout and paint CPU time.
These opt-in debug traces are diagnostics, not a release benchmark.

Start GUI proofs from an Omarchy terminal. Agent or SSH shells can omit the
desktop environment. On this workstation, use:

```sh
XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 \
  ./desktop/target/release/gamesync-desktop --card-proof
```

For repeatable native geometry checks, launch the built executable directly:

```sh
desktop/target/GameSync.app/Contents/MacOS/gamesync-desktop --demo --small-window
```

`--small-window` starts at 960 × 600.

## macOS release build

Run `python3 desktop/scripts/package_macos.py` on macOS (Python 3.11 or newer).
It builds the locked release profile for the host architecture, copies the
executable and icon into a standalone bundle, signs it, and creates a ZIP under
`desktop/target/macos/`. It signs with the first Developer ID Application or
Apple Development certificate in the keychain, or with the identity in
`GAMESYNC_SIGN_IDENTITY`. Without a certificate it uses an ad-hoc signature.
Keychain then asks for its password again after each rebuild, because an
ad-hoc signature identifies only one exact build. With a certificate, choose
**Always Allow** once for each GameSync keychain entry. The app does not depend on the
checkout or development executable. Its minimum macOS version comes from the
linked binary. This is a local test build, not a universal or notarized release.
Developer ID signing and notarization remain required for public distribution.

To keep a named test build beside the existing app:

```sh
python3 desktop/scripts/package_macos.py --output desktop/target/test-builds --name "GameSync Now"
```

This creates `GameSync Now.app` and a `GameSync-Now` ZIP. It uses the normal
library, settings, and Keychain identity. It does not enable demo mode.

### Appearance

Settings → Look and feel provides Auto/Light/Dark appearance, independent light
and dark Baseline schemes, contrast variants, and two accent switches. Defaults
are Sanctum light and Notion dark. Preferences apply across open windows and
persist on restart. See [chrome and themes](../plan/chrome-themes.md).

On Omarchy, enable **Sync with Omarchy theme** in Look and feel to use the active
Omarchy colors and follow later theme changes. GameSync reads Omarchy's current
theme state. While enabled, application chrome also uses Omarchy's square-corner
design language everywhere, including game covers and physical cards. Its sidebar
uses the theme's primary background like Omarchy shell surfaces. GameSync does not
install hooks or modify Omarchy configuration. Turn the option off to restore the
saved GameSync schemes and interface radii.
