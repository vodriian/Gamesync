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
