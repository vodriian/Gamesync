# Play now native implementation

October 6, 2026. Branch: `vova/play-now-native`, from the prototype branch.

## Outcome

- Added Play now directly below Home. The native setup and card hand follow
  the reviewed prototype: supplied icons, 40 px activity controls, large brain
  icon, infinity time, sentence-case roles, prominent Edit, and no New session.
- Added a pure offline selector over actual library records. It applies
  ownership, hidden/deleted, status, device, personal exclusion, activity,
  energy, and time constraints before ranking. Hands contain at most three
  unique games. One reshuffle uses unseen eligible games; sparse hands stay
  sparse. Unknown required evidence has an explicit empty state.
- Added conservative local profiles, field source explanations, and manual
  corrections. A validated cached-analysis contract is ready for later AI
  enrichment. No AI service or dependency was added.
- Added persistent saved/excluded picks, Undo, and bounded recent choices.
  Personal writes use existing revisions and sync projection. Write failures
  retain the pending action for Retry or Discard, including a library already
  known to be unavailable. Profile save restores keyboard focus.
- Added local Steam manifest checks and guarded launch requests. Recheck
  installation before launch; distinguish OS acceptance from verified play.
  Remote, uninstalled, unknown, and demo choices only record the choice.
- Added `python3 desktop/scripts/dev.py --play-now-demo` for a persistent,
  isolated sample library, settings, and cache. Its bundle identifies itself
  as a demo before settings load, even when opened without arguments.

## Automated checks on macOS

- Full `cargo test --manifest-path desktop/Cargo.toml`: 185 passed, 0 failed,
  11 existing optional tests ignored. Includes 13 new tests for selection,
  profile precedence and validation, personal persistence and sync, Steam
  manifests, and the stored-demo ownership regression.
- `cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings`
  passed after the final recovery fix.
- `cargo fmt --manifest-path desktop/Cargo.toml --check` and `git diff --check`
  passed. Final demo build passed with
  `python3 desktop/scripts/dev.py --play-now-demo --build-only`.
- The full suite ran before the last two UI recovery guards. The final guards
  passed all-target Clippy and the native failure/retry check below.

## Native macOS checks

- Inspected setup, three-card and one-card hands, details, saved/excluded
  picks, recent choices, and profile editing in the running GPUI app.
- Exercised Save for later, Not now/Undo, exclusion/Undo, explicit choice,
  one reshuffle, and restart persistence. Demo choices did not launch games.
- Invalid profile minutes were rejected; corrected values persisted across
  restart. Keyboard text replacement, visible Tab focus, and Escape after
  saving worked.
- Infinity + Drive + Low produced no match. Try any activity relaxed only
  that filter and returned one eligible game.
- Inspected dark and light appearance, a 960 × 600 window, scrolling, and
  the isolated reduced-motion setting. Controls remained reachable.
- Temporarily moved only the isolated demo library. Last-valid records stayed
  visible with an unavailable notice and cover fallbacks. A save kept the
  saved count unchanged and showed Retry save / Discard change. Restored the
  directory, refreshed, and retried: the saved count advanced from 1 to 2.
  The demo directory was restored before finishing.

## Limits and next work

- No AI requests, live installed-game launch, Linux native acceptance,
  cross-device synchronization, or full screen-reader audit were performed.
  The native accessibility tree exposed little content; keyboard checks do
  not establish screen-reader support.
- Local estimates need calibration against a reviewed game set. Brain dead
  can require manual data until profile enrichment supplies missing evidence.
- An early preview launch read the normal library before demo startup was
  isolated. It made no personal edits and launched no game. Subsequent GUI
  checks used the dedicated demo bundle and temporary library.
- Existing prototype, supplied SVG files, Elyx caches, and unrelated work
  remain intact. No commit, push, PR, or merge was made.

Next: opt-in recommendation profile analysis using the existing provider
settings, followed by calibration and Linux/live-Steam acceptance.

## Native review: shared details and toolbar

- Replaced the recommendation details subpage with the existing open game
  card. Added a Play now section before Notes, with a brief reason, energy and
  session summary, and named icon actions. The normal library details view
  uses the same section. Return navigation keeps the recommendation hand.
- Profile edits now use a bounded, scrollable modal with fixed Save/Discard
  actions. Dirty drafts block incidental dismissal; a successful save returns
  to the same card. Preference edits use a modal and cancel restores the
  previous context. Existing revision and failure-retry behavior remains.
- Moved Reshuffle to the right of the preference strip. Edit is an icon-only
  Customize button directly after the setup chip. Removed the hand footer and
  the no-unseen-games message. Added the supplied Saved/Recent icons, renamed
  Recent, removed the decorative setup preview/subtitle, and changed the title
  to "Let's choose a game for you". Canonical SVG files remain unchanged.
- Changed demo bundle updates to replace the executable inode. macOS rejected
  an overwritten preview executable with an invalid cached signature page;
  atomic replacement restored native launch without changing security policy.
- Checks: recommendation integration tests (11 passed), strict all-target
  Clippy, formatting, and development build. Native macOS review confirmed
  setup copy, toolbar, context cancellation, shared card navigation, profile
  save and dirty-draft Escape protection, and the one-card reshuffle state.
  At 960 × 600, both modals scroll within the window; profile actions stay
  visible. A discarded draft retained the prior saved value after reopening.
  No AI request or actual Steam game launch was made.

## Personal-library test build: GameSync Now

- Added `--name` to the macOS package script. Named builds keep the normal
  bundle identifier, Keychain identity, settings, and library paths.
- Built the current `vova/play-now-native` working tree with
  `python3 desktop/scripts/package_macos.py --output desktop/target/test-builds --name "GameSync Now"`.
  The locked release build passed. Output: `desktop/target/test-builds/GameSync Now.app`
  and `GameSync-Now-0.1.0-macos-arm64.zip`. The existing `GameSync.app` remains.
- Verified Apple Development certificate signing, strict codesign validation,
  ZIP integrity, arm64 architecture, display name, and minimum macOS 11.0.
  The standalone executable's machine-code section matches the fresh release;
  certificate signing changes the executable's signature bytes.
- Opened the named app without demo flags. Native inspection confirmed that
  the personal library loaded and the updated Play now setup was visible.
  Left it open for user testing. No game records were edited and no game was
  launched during this smoke check.
- This is a local, non-notarized macOS test package. Linux acceptance and AI
  enrichment remain deferred. No Rust source changed during packaging; prior
  automated checks above were not repeated. No commit or push was made.
