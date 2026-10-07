# Play now control motion

Continued `vova/play-now-native` with its existing local work.

- Moved Brain dead below energy and before activities in the shared setup
  form, including the preferences modal. Replaced the checkbox with a switch.
- Applied the improve-animations audit to the controls. The focused findings
  and implementation targets are in [the motion review](../motion-review.md).
- Added one sliding selection pill per preference group: time, energy,
  activity, device, and scope. The pill travels and resizes over 200 ms,
  including between wrapped activity rows. The switch thumb uses 120 ms.
  Rapid input retargets the current rectangle through the existing Motion
  primitive. Keyboard input, restored drafts, responsive reflow, and Reduce
  motion snap to the current selection.
- Used animation-vocabulary to identify the requested shared element
  transition. Labels and layout stay fixed; a rounded quad moves behind them.
  This replaces the earlier per-button fill blend.
- Matched the linked GPUI switch geometry using the existing Button for input
  and focus. The pinned gpui-component 0.5.1 Switch has no keyboard or
  reduced-motion path. No dependency upgrade or reference-project edit.
- Fixed off-state contrast against the Brain dead panel. The switch has a
  44 x 40 px target and keeps the supplied brain icon and description.

## Checks

- Formatting, strict all-target Clippy, and all 13 recommendation tests passed.
- Native macOS demo: setup and modal placement; both switch states; repeated
  changes; Tab focus, Space and Enter activation; selecting time and energy;
  and cancellation restoring 30 minutes, low energy, and Brain dead on.
- Enabled Reduce motion in the isolated demo, verified the saved setting,
  exercised the switch and time choices, then restored the setting to off.
- Checked the corrected native controls at normal speed in the isolated demo:
  all five groups, activity wrapping, scrolling, and both switch states.
- Built a temporary 2-second demo to inspect intermediate frames. Native
  captures show the energy and time pills between options, plus activity
  movement, width changes, and reversal across rows. Repeated in the modal.
  Keyboard Space selected immediately with a visible focus ring; modal
  cancellation restored the prior context. Reduce motion snapped to the
  target even in the slowed build. Restored Reduce motion to off.
- Restored the 200 ms source timing and rebuilt the demo and release package.
  No slow-motion override remains. Strict all-target Clippy, all 13
  recommendation tests, formatting, and diff checks passed again.
- Exact frame pacing, resize interaction, light-theme rendering, and Linux
  remain unverified.
- Personal game records were not used for interaction tests. No commit,
  push, PR, or merge was made.

## Test package

- Rebuilt with `python3 desktop/scripts/package_macos.py --output desktop/target/test-builds --name "GameSync Now"`.
- Apple Development signing, strict codesign verification, arm64 architecture,
  and ZIP integrity passed. The package is local and not notarized.
- Rebuilt the named package again after the sliding correction. Release build
  and strict codesign verification passed. Reopened it with the existing
  library and confirmed the updated Play now setup. Left that screen open
  for user testing.
