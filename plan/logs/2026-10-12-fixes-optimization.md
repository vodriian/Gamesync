# Fixes and optimization

Branch: `fixes-optimization`, from `main` at `8903379` (after PR #12).
Scope: startup speed, idle cost, motion pacing, Windows 3D cards, Windows Steam.
The branch also holds `b299e43`, the macOS popup shadow fix from another
session; it is not part of this work.

## Changes

- **Timing trace.** `GAMESYNC_STARTUP_TRACE=1` prints `[startup]` phase marks
  and one `[idle]` line for each background refresh. An ignored test,
  `scan_timing_probe`, builds a library with history and prints scan times.
- **Indexed library first.** The storage worker starts with the window. It
  sends the library that the local index last recorded, then scans the folder.
  The view no longer waits for the scan. A scan that confirms the indexed
  library does not replace the model. A store created at launch has no
  indexed contents, so the first launch is unchanged.
- **Cheaper scans.** A scan parses each record once. Discovery reads a current
  file only when its name does not give the game ID.
- **No idle scans.** `LoadedLibrary::digest` identifies what a view shows. A
  refresh with the same digest leaves the model and the view alone. While
  the file watcher works and reports no change, the 10-second sync round
  reuses the last read. The folder is scanned on a watcher event, a manual
  refresh, a watcher failure, and every five minutes. See
  [data sync](../data-sync.md). The first sync round at startup uses the
  startup read instead of a second scan.
- **Motion.** `Motion` has no timer. Each view requests the next frame while
  its motion is active, so panels, toasts and switches follow the display rate.
- **Windows 3D.** DirectX composites material card faces: Cards hover tilt, the
  focused front, and the book turn. See the
  [patch notes](../../desktop/vendor/gpui/GAMESYNC-PATCH.md).
  `GAMESYNC_FLAT_CARDS=1` turns the compositor off on any platform.
- **Flat artwork.** Where no mask layer composites the cover (Windows, Linux),
  the cover and its gradients take the corner radius themselves.
- **Windows Steam.** Install detection reads the Steam folder from the
  registry, then the default folders. Launch asks the shell to open
  `steam://rungameid/<id>` without a console window.

## Measurements

Host: macOS 27.0, arm64, 120 Hz display. Release builds. The library is a copy
of a real one in an isolated profile: 288 games, 2,495 history files, sync off.
Other builds ran on this Mac during the session, so absolute times vary.
Before and after runs alternate under the same load.

| Measure | Before | After |
| --- | --- | --- |
| Games shown after launch, median of 6 | 454 ms | 297 ms |
| Games shown after launch, range | 410–1,362 ms | 267–339 ms |
| Idle work, sync off | 126 ms scan every 10 s, model replaced | none in 24 s |
| Idle work, sync folder set | scan and round every 10 s | round only, 2.5–4.6 ms |
| Frames in a 200 ms sidebar slide | about 13 (16 ms timer) | 24–27 (display rate) |
| Scan, 1,000 games × 8 revisions | 289–297 ms | 261–263 ms |
| Index open, same probe library | 3 ms | 3 ms |

- The sync-folder figure used an empty folder, so each round failed to open
  it. It shows that a round no longer scans; it is not the cost of a real round.
- "Before" for the sidebar slide is the timer period, not a measured run.

Measured and left as they are:

- Metal shader and pipeline setup at launch: about 5 ms, also for a new binary.
- A full-window frame: 2–3 ms CPU in Grid, Cards and Table, with 288 and with
  10,000 games. The budget at 120 Hz is 8.3 ms.
- Filter and sort of 10,000 games: 0.3–1.5 ms.
- Thin LTO with one codegen unit: 33.3 MB to 28.5 MB, no measurable startup
  change, an 11-minute release build under load. Not adopted.

## Checks

macOS 27.0 arm64, Rust 1.99.0:

- Formatting, Clippy with `--all-targets -- -D warnings`, and the full
  deterministic suite passed at each phase. Final: 219 passed, 12 ignored.
- Trace order on the library copy: `cached_applied` comes before `scan_done`.
- Idle: 24 seconds produced no scan and no frame. A touched record caused one
  scan and no redraw. A changed cover caused one scan and a redraw.
- Motion: a temporary probe moved the sidebar and showed a toast. Each motion
  drew 24–30 frames and then stopped, also with the window inactive. The probe
  was removed.
- Card proof: window captures before and after the shared uniform code are
  pixel-identical. With `GAMESYNC_FLAT_CARDS=1` the card is flat and its
  artwork has rounded corners.
- HLSL: `card_vertex` and `card_fragment` pass a `glslc -x hlsl` parse and
  type check. A broken copy fails it. This is not `fxc`.

## Not verified

- **Windows build.** The DirectX code was not compiled on macOS: a C dependency
  needs the Windows SDK. The Windows workflow must compile it. Record the run
  here.
- **Windows visuals.** Card tilt, clear coat, shadow, book turn, flat artwork,
  and motion smoothness on Windows need a check on a Windows PC.
- **Windows Steam.** Registry lookup and launch need a PC with Steam installed.
- **A real sync round** with the reused library read. Check that a remote edit
  still appears within 10 seconds.
- **Linux.** Flat artwork rounding and frame-paced motion changed Linux paths.
  Not run on Linux.
- **Motion feel** on macOS. Frame counts are measured; the feel is the user's.

## Found, not changed

- Each game save reads and flushes every earlier revision of that game
  (`RevisionFiles::publish_child`). The cost grows with history. It is a
  durability decision, so it stays until it is discussed.
- Each launch logs about 34 `usvg` warnings for `currentColor` in the icons.
- A sync round notifies the sidebar every 10 seconds, also with no change.
- The first frame rasterizes about 22 icons on the main thread (about 24 ms).

## Next

1. Run the Windows workflow on this branch. Fix compile errors.
2. Check `GameSync.exe --card-proof`, Cards hover, the book turn, and Play now
   install state and launch on Windows.
3. Send Windows screenshots of Home, Grid, Cards, an open game and Settings in
   light and dark. Fix the concrete differences from macOS.
