# Desktop motion and interaction review

GameSync base: `f88ab2e`, with existing local work. Eagle reference: `d1fc38f`,
with local edits. Read `src/ui/motion.rs`, panel wrappers in `src/ui/app.rs`,
and category navigation in `src/ui/settings_panel.rs`. The source project was
not modified. User-requested fixes define the selected scope.

The improve-animations audit informs this implementation. Its generic read-only
workflow does not block the user's explicit implementation request. Keep these
records in the existing `plan/` folder. No additional agents or dependencies.

| Priority | Category | Surface | Finding | Implementation |
| --- | --- | --- | --- | --- |
| Medium | Interruptibility | Sidebars | Boolean visibility cuts panels instantly | 200 ms ease-out width clipping; reverse from current progress |
| Medium | Accessibility | Motion | No reduction preference | Device-local Reduce motion in Look and feel |
| Medium | Missed opportunity | Sidebar sections | Status and Collections cannot fold | Reversible 200 ms clipped section reveal |
| Low | Cohesion | Grid density | Density changes in one step | Reflow once, then resize visible artwork over 200 ms |

Targets use Eagle's `ease_out_quint` equivalent: `1 - (1 - t)^5`. This matches
Eagle's existing interpretation of the audit's strong ease-out. No spring,
subtree-opacity fade, entry stagger, keyboard selection fade, or animation on
search. Collapsed content remains mounted during its exit. New targets start
from the current value and cancel the previous frame task.

Native GPUI has no CSS transform substitute for these panel layouts. Panel widths change during the short transition, as in Eagle. For density, grid
columns change once; only the visible artwork resizes inside clipped cells over
200 ms. This avoids rebuilding virtual rows each frame for a density switch.
This is not a shared-element zoom viewer. Do not claim browser compositor performance
or measured frame rates. Large-library frame profiling remains future work.

Verification: toggle both panels, reverse a section quickly, change density,
and enable Reduce motion. Selection and search must stay immediate. At rest,
no motion task should request frames. Verify native rendering after build.

## Play now controls — October 7

Base: `fb19ea2`, with the existing uncommitted Play now feature. Scope is the
setup page and its shared preferences modal. The user requested implementation;
the improve-animations skill supplies the audit criteria, without an extra
approval or delegation step.

| Priority | Category | Before | After | Why |
| --- | --- | --- | --- | --- |
| Medium | Spatial continuity | Time, energy, activity, device, and scope blend separate button fills | One selection pill slides and resizes between measured choices over 200 ms | Keeps the selected surface continuous, including between activity rows |
| Medium | Accessibility | Brain dead uses a checkbox after activities; pinned Switch has no keyboard or reduced-motion handling | Switch below energy, built on the existing focusable Button | Keeps the requested switch appearance and keyboard support |
| Medium | Interruptibility | Pinned Switch restarts its thumb animation from an endpoint | Retarget from current progress; stop after 120 ms | Rapid reversal does not jump or lock input |

Implementation: `desktop/src/ui/play_now/controls.rs` uses the existing
`desktop/src/ui/motion.rs` curve, `1 - (1 - t)^5`: 200 ms STANDARD for
the shared selection pill, and 120 ms CONTROL for the switch thumb. Keep this
established native ease-out so each click responds immediately. Each group
measures its buttons before paint, then paints one rounded selection quad
behind them. Retarget from the current rectangle when another choice arrives.
Reuse semantic colors and a 36 x 20 px switch track, 16 px thumb,
and 44 x 40 px button target. Preserve immediate press feedback and action on
release. Do not add a new dependency or change recommendation rules.

Keyboard clicks and Reduce motion snap to the final value. Initial render and
cancelled modal drafts also snap. The preference layout does not animate;
only the painted selection rectangle and bounded switch thumb move. Resize
and responsive wrapping snap to the newly measured geometry. This is native
GPUI paint work,
not a claim of browser compositor acceleration. No continuous task remains
after the control settles.

Feel check: inspect the pill in transit, including its width and vertical
position between activity rows. Change preferences, reverse the switch, and
repeat in the modal.
Tab to the switch and activate it with Space and Enter. Cancel a changed modal
and check restored values. Repeat with Reduce motion. Inspect both switch
states against the native theme before packaging.
