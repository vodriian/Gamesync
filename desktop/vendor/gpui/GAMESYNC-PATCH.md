# GameSync card compositor patch

Base: the published `gpui` 0.2.2 crate, copyright 2022–2025 Zed Industries.
The upstream Apache 2.0 license is retained in `LICENSE-APACHE`. The original
package manifest is retained in `Cargo.toml.orig`. Cargo selects this copy through
`[patch.crates-io]` in `desktop/Cargo.toml`; the shared Cargo registry is unchanged.

Changed upstream files:

- `src/gpui.rs`: export the opt-in card surface.
- `src/window.rs`: capture native paint operations in local card coordinates;
  opt-in CPU frame diagnostics.
- `src/scene.rs`: compare captured content and translate its primitives.
- `src/platform/mac/metal_renderer.rs`: render nested card scenes to private GPU
  textures and resume the parent pass. Offscreen alpha uses source-over blending.
- `src/platform/blade/blade_renderer.rs`: unwrap the existing video surface field.
  Blade does not render card textures.

New files:

- `src/card_layer.rs`: fixed-size face content, identity, pose, radius and material.
  Non-Metal platforms return the original native element.
- `src/platform/mac/card_renderer.rs`: texture ownership, content invalidation,
  bounded memory accounting and Metal draw commands.
- `src/platform/mac/card_shaders.metal`: perspective projection, back-face culling,
  rounded composite mask, satin highlight, thin edge and shadow.

Face scenes are compared independently of their outer pose. A changed cover,
primitive, theme color, scale, size or editor content invalidates the face.
Material faces are cached only while used; the app supplies one hovered library
card or the focused front/back. A flat `frosted_top` surface additionally caches
the library viewport and blurs only its top strip beneath the toolbar. The app
limits this optional capture to a 24 MiB pixel estimate; it shares the existing
64 MiB allocation ceiling. Large windows and non-Metal platforms retain plain
translucency. Static artwork masks share one temporary target.
Targets use 64-pixel allocation buckets to avoid churn from fractional columns.
Retired, in-flight targets count toward the additional 64 MiB ceiling. Allocation
checks use Metal's aligned size estimate and track actual `allocatedSize`, not
only pixel dimensions. The trace reports the lifetime peak. Metal owns
command resources until completion. There is no readback or per-pointer image
upload. The pre-existing image atlas and path targets are outside this ceiling.

A failed command invalidates cached pixels before retry. Application code owns
all game records, drafts and persistence; none enter the renderer.

## Verification

`gamesync-desktop --card-proof` renders the real Hades card before library use.
Space animates the turn; 1 sets an oblique pose, 2 shows the edge, 3 shows the rear.
`GAMESYNC_CARD_TRACE=1` reports encoding time, GPU duration, texture bytes,
face rasterizations and cache hits. Timings include the full window render;
encoding time excludes drawable acquisition. `GAMESYNC_FRAME_TRACE=1` reports
native layout/prepaint/paint CPU time separately. Neither trace is on by default.

See `plan/logs/2026-09-21.md` (from the repository root) for the native checks and
remaining platform limits. Linux keeps flat cards; Linux 3D remains pending.
