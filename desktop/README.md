# GameSync desktop

A native Rust/GPUI game grid, based on Eagle Companion.

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

Use the folder button or Command-O / Control-O to open a GameSync library.
The app remembers the last successfully opened folder. To choose one at launch:

```sh
python3 desktop/scripts/dev.py --library /path/to/MyGames.library
```

Use `--demo` to open the bundled sample games instead of the remembered library.

## This milestone

- Browse 12 demo games with bundled covers. No network is needed at runtime.
- Open a real library folder with its own status labels and local covers.
- Refresh automatically after file changes, or use Command-R / Control-R.
- Filter by title, tag, status, or favorite.
- Click a cover, then use arrow keys to select games.
- Use Command-F on macOS or Control-F on Linux to focus search.
- Toggle the sidebar, inspector, cover density, and appearance from the toolbar.
- Quit with Command-Q on macOS or Control-Q on Linux.
- Use Refresh in the toolbar or Command-R / Control-R. The footer shows progress and completion.

Bundled ratings, statuses, descriptions, and playtime are sample data. In a real
folder, select a game and choose **Edit details**. Set status, rating, favorite,
tags (one per line), notes, or your description. Click one of five stars to rate;
click the selected star again to clear it. Save with the button or Command-S /
Control-S. Turn off **Use my description** to use the retained Steam text.

The inspector stays on one draft until Done or Discard. Refresh does not replace
it. Ordinary window close, the Quit action, and folder changes block while a draft
is dirty or saving. Drafts live in memory; forced exit or an OS shutdown can lose
unsaved work. A successful save creates a recoverable revision on disk.

This build remembers the library location. Steam connection, table, kanban,
imports, AI, and saved appearance preferences come later.

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
returns to current details. Branch comparison and resolution controls come next.
Local publication does not confirm Dropbox upload.
Only macOS has been tested; Windows writes are disabled.

## Try a disk library

Create a disposable library with bundled games and copied covers. The destination
must not exist. This command never reads your Steam account or Obsidian vault.

```sh
cargo run --manifest-path desktop/Cargo.toml --example demo_library -- /tmp/GameSync-Demo.library
python3 desktop/scripts/dev.py --library /tmp/GameSync-Demo.library
```

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
