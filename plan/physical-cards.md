# Physical card library

GameSync keeps the native Rust/GPUI app and the existing card artwork design.
The references establish a physical object: artwork and title on the front,
information and personal edits on the back. Their branding is not app content.

## Navigation and layout

- Show the left sidebar by default: All games, Favorites, status counts,
  collapsible sections, collection management and Settings. Keep its toggle.
- Combine scope, search and Cards/Grid/Table in one overlaid
  translucent toolbar. On macOS, blur the scrolling content beneath it.
- Open a game full-window. Retain library filters, selection and scroll state.
  Reuse Eagle's virtualized viewer filmstrip below the card for direct browsing.
- Cards have a fixed footer below the artwork. Composite the cropped cover,
  gradient and lighting before applying one rounded artwork mask.
- Keep virtual rows and the existing write safeguards. Let the library reach the
  bottom edge; show library help and sync status in a small corner control.
  Keep shortcuts in button tooltips; the thumbnail strip sits at the bottom edge.
- Use normal capitalization, soft shadows and hover-only card/table highlighting.

## Rendering

A repository-local patch of pinned GPUI 0.2.2 adds an opt-in Metal card surface.
Its native face paints into a private GPU texture, then a shader projects the
whole face in perspective. The front and back rotate together, with back-face
culling, a thin edge, a restrained satin highlight and a shadow. No foil,
particles or idle animation. See [patch notes](../desktop/vendor/gpui/GAMESYNC-PATCH.md).

Only the hovered library face and focused front/back retain card textures. The
blurred toolbar also captures the library viewport, limited to a 24 MiB pixel
estimate within the shared 64 MiB ceiling. Larger windows use plain translucency.
Static artwork
masks reuse a temporary target. Captured paint changes invalidate the pixels;
pose changes reuse them. Additional targets, including retired in-flight targets,
are capped at 64 MiB. No CPU image readback or per-pointer texture upload.

The native editor entity owns drafts, focus and its explicit scroll handle. Its
captured face appears during a turn; the live editor uses the same bounds when
the back settles. Rotating content is covered by an input shield. The library
is not painted or hit-tested while a card is focused.

## Motion

- Library Cards: up to 5 degrees pitch and 8 degrees yaw. Grid/Table stay still.
- Focused front: up to 8 degrees pitch and 12 degrees yaw. Editing stays flat.
- Mouse and Space turn around the vertical center by 180 degrees. A critically
  damped angular spring settles in about 400 ms. Reversal keeps angle and velocity.
- Pointer exit returns smoothly to neutral. Animation requests stop at rest,
  outside rendered virtual rows and while the window is inactive.
- Reduced motion uses immediate faces and no tilt. Space in an input stays text.
- Keep Escape and turn controls available. Preserve pending-save safeguards and
  keep a card open if editing removes it from the active filter.

## Platform boundary

macOS Metal is the first 3D implementation. Other renderers retain native flat
content and immediate face changes. Linux 3D is pending. A macOS build is not
proof of Linux compilation or native behavior.

## Validation

The isolated actual-game renderer proof precedes library integration. Inspect
perspective, an edge-on pose and a full animated turn. Check navigation, all
three views, masks, themes, minimum window, input controls, editing, restart,
filter changes, reduced motion and a 10,000-record library. Record measured GPU
and texture figures separately from visual checks in the latest UI log.

The optional cover-colored ambient backdrop is deferred. Review the simpler
floating card and softened shadows first.

## Steam-first follow-up

Folder opening is removed from the toolbar, menu, shortcut and CLI. Normal
startup prepares internal storage for Steam. Previously configured readable
stores remain in use without a picker. Sample cards have a separate persistent
store, including collections and edits. Front footers use status and collection
badges plus rating; they no longer repeat branding or favorite text.

Sidebar collapse/restore uses the existing 200 ms quintic ease-out token. Repeated
toggles retarget from the current position. Reduce motion changes it immediately.
The sidebar contents retain fixed width while sliding inside a clipped viewport;
the virtualized library resizes with it. Optional frosted capture is disabled
during this short layout transition to avoid texture allocation churn.

Toolbar view choices use icon buttons with tooltips. The rounded search field
includes a search icon and matches the view-control height. Descriptions display
read-only; removing their edit controls does not remove stored personal data.

The viewer strip toggle sits before Previous/Next, with a 200 ms reversible
clipped slide using the existing motion token. Reduce motion is immediate.
Visibility is retained while browsing in the current session; T toggles it when
the viewer, rather than an editor input, has focus.

## September 22 clear-coat refinement

The user requested a more visible trading-card shader, referencing
sixrobin/TradingCardShader. Keep the existing perspective and compact shadows;
replace the subtle satin band with an angle-driven clear-coat sweep, narrow glint,
and edge reflection on fronts. Attenuate it over title/footer content. Details
remain clear for editing. This is original Metal code over cached face pixels,
without additional textures or idle animation. Layer-separated artwork/parallax
from the reference remains a separate asset-dependent task.
