# Sidebar Steam sync

Branch: `vova/sidebar-steam-sync`.

- Added the supplied refresh icon before Settings, at the same 18 px size.
- Moved the existing Steam job from Settings to a shared app entity. Both
  buttons use it. Duplicate starts are blocked. Cancellation and saved timestamps
  keep their existing behavior. Completion refreshes the library and shows feedback.
- The icon turns clockwise once per second only while sync runs. Reduce motion
  uses a static accent icon. A disconnected account opens General settings.
- The demo button is disabled. No dependency was added.

## Checks on macOS arm64

- Formatting, scoped Clippy with `-D warnings`, and the development build passed.
- Binary tests: 38 passed. Steam/collection integration tests: 9 passed.
- Native demo: placement and icon size checked. Two captures showed different
  rotation angles during a simulated job. Settings showed the same job and an
  enabled Cancel sync button. Cancellation returned the icon to its idle position.
- Reduce motion: two captures showed the same static accent icon while the
  simulated job ran. Temporary probe code was removed before the final build.
- Native inspection caught an invisible raw SVG; using the existing Icon
  component fixed inherited color handling.
- Real Steam requests, Linux/Windows visuals, and screen-reader behavior were
  not checked in this task.

## Delivery checks

- Refreshed `origin` before delivery. The feature branch starts from current
  `origin/main` (`a0300fb`).
- Formatting and locked binary compile passed.
- Full deterministic suite: 210 passed, 0 failed, 11 optional OS/network tests
  ignored. The sandbox blocked localhost mock servers on the first attempt;
  the suite passed outside the sandbox.
- Strict all-target Clippy passed with `-D warnings`.
- The release smoke test found that focus-local command routing ignored a
  sidebar click in an empty library. Steam sync now uses the same global command
  registration as Settings. Binary compile, all 38 binary tests, and strict
  all-target Clippy passed again after this fix.
- Fresh optimized package built on macOS 27.0.1 arm64 with
  `python3 desktop/scripts/package_macos.py`:
  - `desktop/target/macos/GameSync.app`
  - `desktop/target/macos/GameSync-0.1.0-macos-arm64.zip`
- Signed with the existing Apple Development certificate. Strict codesign
  verification and ZIP integrity passed. The executable is Mach-O arm64.
  This is a local test package, not notarized.
- Final packaged app launched without demo flags, using an isolated empty
  profile. The refresh icon rendered before Settings. Clicking it opened General
  with the Steam connection form. The test instance was closed afterwards.
- Build outputs remain ignored by Git. Source, usage notes, design decisions,
  and this log form the delivery commit.

## Completion toasts

- Replaced sidebar-only feedback with one main-window toast host. It remains
  mounted with the sidebar hidden and on the game details render paths. Existing
  collection and bulk feedback uses the same host.
- Sonner is the visual reference: <https://sonner.emilkowal.ski/>. The native
  implementation adds no dependency. The kit's notification component has fixed
  top-right placement and no app reduced-motion support, so it was not used.
- Toasts have 24 px right/bottom padding, a 356 px width, a status icon, title,
  optional two-line description, and a close button. At most three cards appear.
- Entry, exit, and stack movement reuse the app's 200 ms reversible motion.
  Individual paints fade; no subtree opacity is used. Reduced motion is static.
- Five-second timers pause on hover and while the main window is inactive.
  Sync completion distinguishes success, cancellation, partial failure/review,
  and fatal failure from typed results.
- Native macOS checks used simulated results and an isolated sample profile:
  entry/fade, settled stack, light/dark contrast, two-line text, reduced motion,
  auto-dismiss, and closing one card were observed. Returning from Settings kept
  pending feedback available. Entry was also inspected at five times its normal
  duration. The temporary trigger and slowed duration were removed.
- Pointer-hover timing and a real Steam sync were not verified. Linux/Windows
  visuals and screen-reader behavior remain unverified.
- Final source checks: 38 binary tests passed, strict all-target Clippy passed,
  formatting/diff checks passed, and a fresh development build passed.
- Refreshed the standard app and ZIP under `desktop/target/macos` after the
  toast change. Strict codesign, ZIP integrity, and arm64 checks passed. The
  signed package launched without demo flags in an isolated empty profile.
  It remains a local Apple Development build, not notarized.
