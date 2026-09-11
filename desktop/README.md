# GameSync desktop demo

A native Rust/GPUI game grid, based on Eagle Companion.

## Run

Install stable Rust. On macOS, install Xcode and its command-line tools.
On Linux, use the system dependencies required by GPUI 0.2.2.

From the repository root:

```sh
python3 desktop/scripts/dev.py
```

The script builds the app. On macOS it also creates a development bundle at
`desktop/target/GameSync.app`. It is not a signed release package.
You can also run `cargo run --manifest-path desktop/Cargo.toml` directly.

## This milestone

- Browse 12 demo games with bundled covers. No network is needed at runtime.
- Filter by title, tag, status, or favorite.
- Click a cover, then use arrow keys to select games.
- Use Command-F on macOS or Control-F on Linux to focus search.
- Toggle the sidebar, inspector, cover density, and appearance from the toolbar.
- Quit with Command-Q on macOS or Control-Q on Linux.

All ratings, statuses, descriptions, and playtime are sample data. This build
does not connect to Steam, edit a personal library, or save preferences.
Table, kanban, durable edits, imports, and AI come in later milestones.

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
