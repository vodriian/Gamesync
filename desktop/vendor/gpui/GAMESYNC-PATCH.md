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
- `src/platform/mac/shaders.metal`: use 32 vertical Gaussian samples for smooth
  UI shadows. Four samples produced visible bands around rounded popup menus.
- `src/platform/blade/blade_renderer.rs`: unwrap the existing video surface field.
  The Linux proof renders material card faces; see `plan/physical-cards.md`.
- `src/platform/windows/directx_renderer.rs`, `shaders.hlsl`, and `build.rs`:
  the DirectX card compositor. See "October 12 DirectX" below.

New files:

- `src/card_layer.rs`: fixed-size face content, identity, pose, radius and material.
  It also holds the capability checks, and the texture size, uniform block and
  pass list that the Metal and DirectX renderers share.
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

September 22 shadow refinement: card layers carry display scale to the compositor.
The projected shadow uses y 20, design blur 40 (sigma 20), spread -20 and 27%
black, with a Gaussian distance-field approximation instead of exponential decay.

October 5 hinge: `CardPose::hinge` moves the vertical rotation axis to a face
edge (-1 left, 0 center, 1 right). The Metal vertex shader rotates around that
axis, keeps it at rest position, and draws the thin edge on the free side.
The uniform block gains one `float4`. Blade ignores the field.

October 7 spine: `CardPose::spine_round` scales the radius of the two
corners on the hinge edge (uniform `hinge.y`), so a turning page meets its
neighbor at a square spine. Hinged faces skip the projected shadow pass;
the app draws one table shadow for the whole book.

October 12 DirectX: Windows composites material card faces.

- `DirectXCards` in `directx_renderer.rs` caches one texture per face with the
  Metal rules: 64-pixel size buckets, a 64 MiB ceiling, release after two
  unused frames, and content comparison on the captured paint operations.
  Direct3D keeps a released texture alive until its pending draws finish, so
  there is no retired list.
- A face pass switches the render target, viewport and global parameters to
  the face texture, draws the captured batches with the window pipelines
  (shadows, quads, underlines, sprites), and restores the window target.
  It uses the window blend states: alpha is exact for an opaque face.
- `card_vertex` and `card_fragment` in `shaders.hlsl` are a port of
  `card_shaders.metal`, including hinge and spine. `build.rs` compiles them
  with `fxc`; debug builds compile them at startup like the other shaders.
- Not ported: the toolbar blur (`frosted_top`) and mask-only layers. A
  mask-only layer returns the flat element, as on Linux, and the app rounds
  that artwork itself. A face skips vector paths and nested surfaces, with one
  warning; the current faces contain neither.
- `card_compositor_available`, `card_hinge_supported` and
  `card_mask_supported` are the capability source for the app.
  `GAMESYNC_FLAT_CARDS=1` keeps every card flat on every platform.
- `GAMESYNC_CARD_TRACE=1` prints texture count, bytes, rasterizations and
  cache hits on Windows. It has no GPU time there.

Checks: the Metal card proof is pixel-identical before and after the shared
uniform code. The Windows workflow compiled the DirectX code and the HLSL
with `fxc`, and its card proof drew and cached one face on the runner's
software renderer. No Windows visual check exists yet. See
`plan/logs/2026-10-12-fixes-optimization.md`.
