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
