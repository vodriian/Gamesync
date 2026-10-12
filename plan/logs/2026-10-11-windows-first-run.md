# First Windows build

Branch: `windows-first-run`, from `main` at `a0300fb`.
Scope: compile the native desktop app and check first startup on Windows.

## Changes

- Installed stable Rust 1.99.0 and Visual Studio 2022 Build Tools 17.14.41.
  Git was installed but absent from the task's PATH.
- Added `desktop/scripts/windows.ps1`. It loads MSVC x64 and the Windows SDK,
  finds `fxc.exe`, and runs locked builds, tests, formatting checks, or Clippy.
- Enabled Windows library and record storage. Staged files are flushed before
  same-volume `MoveFileExW` publication with `MOVEFILE_WRITE_THROUGH`.
  Immutable history never uses replacement.
- Opened existing history with write access for Windows flushes. Bounded
  rename retries handle brief reader/scanner locks. Lasting failures retain
  the revision and report recovery paths.
- Routed Steam cover publication through the same platform file operations.
  Its unconditional Unix directory flush had blocked Windows cover saves.
- Enabled six existing storage/editor/conflict test files on Windows.
  Kept the symlink-escape check Unix-only because Windows symlinks need
  developer mode or elevated privileges. The cache-location check runs on both.
- Added a Windows locked-file recovery test.
- Fixed the development script's Windows executable suffix. Release builds
  use the GUI subsystem, so opening the app does not open a console.
- Added Windows instructions and updated the platform plan.

## Checks

Host: Windows 11 Home, version `10.0.26300`, x64.
Compiler: Rust 1.99.0, MSVC 14.44.35229, Windows SDK `10.0.26100.0`.

- Development build passed.
- Final locked release build passed after the storage and cover fixes.
- Full Windows test suite passed: 211 passed, 0 failed, 11 ignored.
  Ignored checks use OS credentials, public Steam endpoints, or ProtonDB exports.
- Final formatting check passed.
- Final Clippy check passed with `--all-targets -- -D warnings`.
  The vendored GPUI dependency still reports two float-literal compatibility
  warnings. Cargo also reports a future compatibility warning in
  `proc-macro-error2`; dependencies were not upgraded.
- Git whitespace check passed.
- Launched the copied release from its own output directory with isolated
  storage, first normally and then with `--demo`.
  Windows reported visible main windows titled `My games` and `Sample games`.
  The empty store contained zero games; the separate sample store contained 12.
  Both processes ended within 10 seconds of a window close request.
  The PowerShell process object did not provide their exit codes.
- PE headers confirm x64 (`0x8664`) and GUI subsystem (`2`). DLL inspection
  confirms the Microsoft Visual C++ runtime dependency.

## Local artifact

- `desktop/target/windows/GameSync.exe`
- 37,460,992 bytes; bundled artwork and UI assets.
- SHA-256: `e5160aa009d81d2593d218eab9b1766b5becb6b14db8df440b862784e841e371`.
- The original build remains at `desktop/target/release/gamesync-desktop.exe`.
- Unsigned local test executable. No installer was made.

## Remaining checks

- This was a process/startup check, not a screenshot or visual review.
  Keyboard, resizing, card rendering, Settings, and accessibility need review.
- Release startup logs contain existing SVG `currentColor` warnings.
  Window shutdown logs contain GPUI window-not-found/invalid-handle errors,
  even though both processes ended after the close request.
  Debug startup also reports the missing optional DXGI debug interface.
- Live Steam sync, Credential Manager persistence, and cross-device sync
  remain unverified on this host.
- Steam installation detection and game launching still only support macOS/Linux.
- Windows has no supported equivalent of the Unix directory flush used by
  storage. Power-loss durability depends on the filesystem and device.
- Other PCs need the Microsoft Visual C++ x64 runtime.

Next: review the running Windows UI, then test Steam credentials and sync with
real data. Add Windows Steam detection and launching before release.

## Windows app icon

- The first executable had no Windows icon resource. The macOS bundle script
  used `app-icon.icns`, but the Windows build did not use the supplied artwork.
- Derived `desktop/icon/app-icon.ico` from the existing 512 px
  `desktop/icon/app-icon.png`. It has 16, 24, 32, 48, 64, 128, and 256 px frames.
  The PNG and macOS icon stay unchanged.
- Added a resource file and Cargo build script. They embed icon resource ID 1,
  which GPUI loads for its window class. Resource compilation must succeed.
  The direct `embed-resource` build dependency reuses locked version 3.0.11.
- Added `make_windows_icon.ps1` to regenerate the ICO after artwork changes.
- The release rebuild, formatting check, and Git whitespace check passed.
  No domain code changed, so the full test suite was not repeated.
- Windows resource inspection found ID 1 with all seven frames. Windows shell
  icon extraction returned the supplied controller artwork; the extracted
  image was inspected beside the source PNG. This is an executable icon check,
  not a taskbar screenshot check.
- The previous app was running. It was kept open, and the updated build was
  copied to `desktop/target/windows-icon/GameSync.exe` for the next launch.
  The original release path also contains the updated executable.
- Updated executable: 37,601,792 bytes.
  SHA-256: `2c24c02458c4d3f1e37642a9453383491fad5e26c7ca3ba52b82ea1b8e9d7a32`.

## Source delivery

- The user requested a commit and push of the Windows build and icon work.
- The delivery includes source fixes, existing tests enabled on Windows, the
  generated ICO, resource setup, build scripts, documentation, and this log.
- Executables, isolated libraries, extracted icon previews, and raw check logs
  stay under ignored `desktop/target/`. They are not source-control artifacts.
- The feature branch is `windows-first-run`; the destination is
  `origin/windows-first-run`. Main is not changed by this delivery.

## Integration with main — 2026-10-12

- Merged current `origin/main` (`577cf50`) into this branch. It includes the
  Steam sync button and completion toasts from merged PR #11.
- Code merged without conflicts. The log-index conflict in `plan/README.md`
  was resolved by keeping the Windows log as latest and retaining a separate
  sidebar-sync log link.
- Fresh checks on macOS arm64: formatting and locked binary compile passed;
  full deterministic suite passed (211 tests, 11 optional tests ignored);
  strict all-target Clippy and whitespace checks passed.
- The Windows checks above apply to the original Windows commit. The existing
  Windows build workflow will check the integrated branch separately. Its test
  step is non-blocking; inspect that step as well as the build result.
- Windows visual review, real Steam credentials/sync, cross-device sync, and
  Windows Steam launching remain outside this integration check.
