# Chrome and Baseline themes — 2026-09-22

Implemented on `codex/desktop-foundation`. Commit and push authorized after review.
No release packaging.
Existing uncommitted work, including concurrent library display work, was retained.

## Outcome

- Inset rounded content panel, solid shared chrome/sidebar, no sidebar divider.
- Hidden visible main-window title with library-name metadata and native controls.
- Focused details use panel bounds; card shadows and scrollbars remain inside.
- Removed canvas-wide frost capture and fixed tabletop tint; retained card effects.
- All 25 Baseline named schemes plus neutral Obsidian, pinned source and attribution.
- Typed appearance preferences, supported-mode selectors, per-mode contrast,
  independent accents, live window refresh, and legacy migration.
- Explicit content/chrome token resolution, including readable Vivid sidebar,
  menus, card backs, filmstrip, and auxiliary titlebars.
- Atomic persistence preserves unrelated settings and legacy theme metadata.

See [the specification](../chrome-themes.md) for defaults, provenance, and regeneration.

## Checks

- `cargo fmt --check`: passed.
- `cargo test`: 94 passed, 4 ignored, no failures. Ignored checks are not counted as passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo build`: passed on macOS.
- Tests cover catalog completeness, supported modes, valid/resolved tokens,
  contrast rules, both accent toggles, legacy mappings, unknown IDs, and settings
  round-trip preservation. All 282 palette combinations resolve to native tokens.
- Native macOS palette sweep: all 47 supported scheme/mode pairs inspected in
  Settings. Sanctum/Notion and Tonal/Vivid/Black inspected in detail; accent toggles
  and readable Vivid chrome/menu/card-back surfaces checked.
- Native 1280×820 and 960×600: Cards, Grid, Table, focused details, filmstrip,
  scrolling, panel corners, and card-shadow gutters inspected. Search and empty
  results checked. Settings menus and reduced-motion operation checked.
- Sidebar collapse/restore checked. Native close controls, double-click zoom and
  restore, and fullscreen entry checked. Exact mid-animation reversal timing and
  full keyboard focus traversal were not instrumented.
- Restart retained selected palettes/contrasts. Final preferences restored to Auto,
  Sanctum/Notion, Normal contrasts, both accents on, reduced motion off.
- Switching macOS to Dark updated both the already-open Settings window and main
  library in Auto mode without resetting schemes. Restored original macOS Auto
  and observed the library return to light.
- Linux build/runtime and native appearance notifications remain unverified.

The isolated local QA bundle and compiler logs are under ignored build/tmp paths.
No game records, Steam imports, personal edits, credentials, or reference web app
were changed for this theme work.

## Corner correction and catalog trim

The path masks left square content visible outside the rounded outline. Replaced
them with a native rounded quad frame: its inner curve matches the panel radius,
while the panel's rectangular clip hides the outer frame. Checked all four corners
in the native macOS library and focused details at 960×600.

Removed the standalone Obsidian preset from both the bundled data and importer.
The catalog now contains 25 schemes, 45 supported mode pairs, and 270 palettes.
Neutral inherited source values remain an internal color-resolution input.
Existing saved Obsidian IDs safely resolve to the per-mode default. Current user
palette preferences were not changed. Native Settings menu checked.

Targeted appearance tests and native build passed. Formatting and strict Clippy
passed. Linux visual verification remains pending.

## Publication checks

Prepared a theme/layout-only commit, leaving unrelated working-tree changes in
place. Reconstructed the staged source over HEAD in a temporary checkout and
validated it independently: `cargo check`, 75 passing tests (4 ignored),
formatting, and strict all-target Clippy. The larger test counts above describe
the shared development checkout. Daily log updated with this work and its limits.
