# Play now delivery

## Scope

- Deliver the native Play now feature on `vova/play-now-native`.
- Use `windows-test-build` as the PR base. The branch started at `fb19ea2`,
  which includes earlier Windows build and AI Settings work not yet on `main`.
  Keep those four existing commits outside the Play now review diff.
- Keep the early HTML prototype as a reference. Its README now makes the
  later native behavior and canonical Elyx references clear.
- Keep personal libraries, packaged apps, render output, and Elyx caches out
  of Git. Ignore generated `.cache` folders inside `Design`.

## Delivered behavior

- Add Play now below Home. Select time, energy, activity, and optional limits;
  deal up to three games with one optional reshuffle.
- Keep hidden, deleted, unowned, excluded, ineligible-status, and known blocked
  games out of the hand. Unknown evidence stays unknown. Local estimates and
  personal corrections work without AI; AI generation remains deferred.
- Reuse the game book in a shaded modal. Edit preferences and game profiles
  in modals. Use Play, Skip, and Save for later on cards.
- Persist a device-local active session before launch. Show the elapsed timer
  on the page and sidebar, block new picks, and finish with Done playing.
  Explicit session history does not alter imported Steam playtime or status.
- Slide one selection pill per control group in 200 ms. Move Brain dead
  below energy and slide its switch thumb in 120 ms. Keyboard input and
  Reduce motion snap. Keep labels and control layout fixed during motion.

## Elyx

- Add [PlayNow.elyx](../../Design/design/screens/play-now/PlayNow.elyx) with
  12 named states: setup, Brain dead on, More options, hand, preferences modal,
  game overlay, profile modal, active session, saved, recent, no matches,
  and save failure.
- Reuse LibraryShell, Sidebar, Button, IconLabel, IconButton, Checkbox,
  TextField, and the existing CardDetails book. Add shared Preferences,
  RecommendationCard, and Switch components. Update the canonical sidebar,
  entry canvas, workspace guide, and state index.
- Use unchanged supplied SVGs and bundled sample covers. All game values in
  the new references are sample data. Use shared light/dark theme tokens.
- Use stacks and content sizing. Intentional overlays occupy the window;
  modal bodies scroll. More options and save failure use taller canvases
  to expose content that normally needs scrolling.

## Checks

- `cargo test --manifest-path desktop/Cargo.toml --locked`: 187 passed,
  0 failed, 11 optional tests ignored. Includes all 13 recommendation tests.
- `cargo fmt --manifest-path desktop/Cargo.toml --check`: passed.
- `cargo clippy --manifest-path desktop/Cargo.toml --locked --all-targets -- -D warnings`:
  passed.
- Elyx Preview `0.0.1-preview.15345.gd9409cf06`: normalized changed designs;
  full `diagnostics Design` returned 0 errors and 0 warnings.
- Inspected resolved component structure and rendered all 12 states in light
  and dark. Checked setup, hand, and preferences at 960 x 600, plus the hand
  at 1600 x 1000. Small canvases retain scroll content; these are static
  layout checks, not native interaction proof. Render files remain outside Git.
- Native macOS checks and the signed local `GameSync Now` test package are
  recorded in the [session log](2026-10-07-play-now-sessions.md) and
  [control log](2026-10-07-play-now-controls.md). No Rust source changed during
  this delivery pass. The package is local and not notarized.
- Staged whitespace and artifact checks passed: 76 source, asset, design, and
  documentation files; no builds, render output, caches, or personal libraries.

## Remaining checks

- Linux and Windows runtime, cross-device sync acceptance, real Steam launch,
  exact frame pacing, and interactive resizing remain unverified.
- AI recommendation generation is not part of this change.
- Elyx settled-state renders do not prove native animation or accessibility.
  Native keyboard, reduced-motion, timer recovery, and retry checks remain
  the separate evidence described in the linked logs.
