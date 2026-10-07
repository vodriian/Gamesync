# Play now sessions and card overlay

Branch: `vova/play-now-native`. Continued the existing uncommitted feature.

## Changes

- Recommendation actions now use Play, icon-only Skip, and Save for later.
  Skip uses the supplied fast-forward SVG and retains Undo. Removed the
  separate Why this game and Not now controls.
- Recommendation details open over the current hand with a shaded backdrop,
  shadow, and close control. They reuse the book pages and nested profile
  editor. No thumbnail strip, Back button, or adjacent-card navigation appears
  in this overlay. Escape and backdrop clicks return to the same hand.
- Play starts one explicit timed session. A centered Now playing screen shows
  artwork, title, elapsed time, and Done playing. The sidebar shows the supplied
  hourglass icon and timer, including while browsing other library pages.
- A device-local start time is saved before requesting a Steam launch. Active
  sessions block new picks and context changes until Done playing. Navigation
  and restart retain elapsed time. Restored sessions retain their context.
- Recent choices accept an optional explicit end time. Launch retries and
  completion update one session UUID. These values never replace Steam
  playtime or change personal game status. A removed or unavailable game does
  not prevent ending the local timer.

## Checks

- Full locked Cargo test suite passed, including 13 recommendation tests.
  Added coverage for timestamp recovery, backwards clock handling, old choice
  records, end-time validation, retry deduplication, and provider-data safety.
- Strict all-target Clippy, formatting, and `git diff --check` passed. The full
  suite preceded the final focus-cycle and restored-context refinements; those
  passed strict Clippy and build checks.
- Native macOS checks used the isolated PlayNowDemo library: Skip/Undo, overlay
  layout, Tab/Shift-Tab focus, Escape, starting a session, sidebar timer while
  browsing, restart recovery, and Done playing. Fixed a discovered Tab leak
  by binding the overlay's focus cycle ahead of the shared Root bindings.
- Temporarily made only the isolated demo settings unavailable. Done playing
  retained the active screen and a useful retry message. Restored the original
  settings and retried successfully. Verified the active timer was cleared and
  exactly one completed choice remained, with no launch request.
- Backdrop uses shading and the existing card shadow. No background-blur
  implementation or performance claim was added. Minimum-size layout, Linux
  native behavior, and real Steam launch acceptance remain unverified for
  this update.

## Test package

- Fresh locked release build passed with
  `python3 desktop/scripts/package_macos.py --output desktop/target/test-builds --name "GameSync Now"`.
- Output: `desktop/target/test-builds/GameSync Now.app` and
  `GameSync-Now-0.1.0-macos-arm64.zip`. Apple Development signing, strict
  codesign validation, arm64 architecture, and ZIP integrity checks passed.
  This local package is not notarized. The separate `GameSync.app` remains.
- Reopened the packaged app without demo flags. Home loaded the existing
  225-game library, and Play now opened its setup screen. Left it ready for
  the user's test without starting a personal session.
- No commit, push, PR, or merge was made. Personal game records were not used
  for automated or mutation checks.
