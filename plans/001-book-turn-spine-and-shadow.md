# 001 — Make the book turn one object with one table shadow

- **Status**: DONE (2026-10-07; the user checked the live motion)
- **Commit**: e6468d0 plus the uncommitted working tree of branch `native-ui-mode`
  (2026-10-07). `desktop/src/ui/detail.rs` is uncommitted there; match the
  excerpts below, not the commit.
- **Severity**: HIGH
- **Category**: Physicality & origin (spine corners) and Physicality (shadow)
- **Estimated scope**: 5 files. About 60 changed lines, most of them in `detail.rs`.

## Problem

The focused game card in the detail view opens like a book. A spring
(`desktop/src/ui/card_motion.rs`, OMEGA 26, about 400 ms, interruptible) drives
`angle` from 0 (closed card) to PI (open two-page spread). Timing,
interruption and reduced motion are correct. Do not change them. Two physical
faults make the turn look wrong.

### Fault A: the open book looks like two separate cards

The Metal card layer masks every face with one radius on all four corners. The
mask is in `desktop/vendor/gpui/src/platform/mac/card_shaders.metal:35-38` and
`:41`:

```metal
// card_shaders.metal:35 — current
float card_sdf(float2 uv, float2 dimensions, float radius) {
    float2 q = abs((uv - .5) * dimensions) - dimensions * .5 + radius;
    return length(max(q, 0.)) + min(max(q.x,q.y),0.) - radius;
}
...
// card_shaders.metal:43 — current
    float d = card_sdf(in.uv, u.rect.zw, u.shape.x);
```

So the turning leaf always has rounded corners at the spine. To hide the corner
pop at the end of the turn, an earlier change rounded every page. The settled
spread is now two separately bordered, separately shadowed cards. The spine
shows a double line and notches at the top and bottom:

```rust
// desktop/src/ui/detail.rs:555-572 — current
            // Every page keeps the card's four rounded corners in every state.
            // The turning leaf is a card layer with one uniform radius, so the
            // page beneath it and the settled spread match it: no square corner
            // shows past the closed card, and none appears when the turn settles.
            let page = |content| {
                div()
                    .w(dimensions.width)
                    .h(dimensions.height)
                    .rounded(radius)
                    .overflow_hidden()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow(super::card::card_shadow(false))
                    .child(content)
            };
            if spread {
                stage = stage.child(h_flex().size_full().child(page(left)).child(page(right)));
            } else {
```

### Fault B: three shadows that turn with the page

- Each hinged leaf face draws the compositor's projected shadow. The modes are
  set in `card_renderer.rs:252-256`:

  ```rust
  // desktop/vendor/gpui/src/platform/mac/card_renderer.rs:252 — current
          let modes: &[f32] = if card.pose.material && card.pose.frosted_top == 0. {
              &[2., 1., 0.]
          } else {
              &[0.]
          };
  ```

- Mode `2.` is the shadow pass (`card_shaders.metal:44-55`: 27% black, 20 pt
  down, sigma 20, spread -20). It runs through the same 3D vertex transform as
  the face. So the shadow rotates with the leaf, shrinks to a line at 90° and
  swings across the table like a sticker. It does not stay on the table.
- The right page under the leaf adds its own box shadow,
  `card_shadow(false)` (20%). The shadow gets darker the moment the turn
  starts.
- The settled spread then replaces the leaf shadow with a different box
  shadow, so the shadow jumps when the turn settles.

## Target

The book is one physical object, closed or open, with one shadow on the table.

1. **Spine corners animate with the angle.** Let
   `open = (1. - angle.cos()) / 2.` (0 closed, 1 open; this is the same term
   that moves the spine). Let `spine_round = 1. - open`.
   - The corners on the hinge side of each turning face use `radius * spine_round`.
   - The other two corners keep `radius`.
   - The right page under the leaf uses the same rule on its left corners.
   - Closed (angle 0): all corners are fully round, so the hidden right page
     matches the card exactly.
   - Open (angle PI): the spine corners are square, so the turn ends on exactly
     the settled spread.
2. **The settled spread is one container.** It has one rounded outline (outer
   corners `radius`, square inner corners), one 1 px border, one center divider
   on the left page (`border_r_1`), and one shadow, `card_shadow(true)`.
3. **One table shadow during the turn.** It is a box shadow on an empty div
   behind the pages. Its bounds are the projected footprint of the book on the
   table:
   - `left = spine + min(0., width * angle.cos())`
   - `right = spine + width`
   - `top = 0`, `height = height`
   - corner radius `radius`
   - the shadow is `card_shadow(true)`

   At angle 0 this is the closed card's footprint. At PI it is the full spread
   (`spine == width`, so left is 0 and right is `2 * width`). The width changes
   continuously because `cos` is continuous. The opacity never changes.
   `card_shadow(true)` (27% black, y 20, blur 20, spread -20) is the box-shadow
   equivalent of the compositor's "Figma hover shadow" (27% black, y 20, sigma
   20, spread -20). So the shadow looks the same before, during and after the turn.
4. **Hinged faces draw no projected shadow.** Any `CardPose` with
   `hinge != 0.` skips shader mode `2.`. Library cards, the closed focused
   card and `--card-proof` all use `hinge: 0.`, so they keep their shadow
   unchanged.

## Repo conventions to follow

- **The vendored GPUI patch** is documented in `desktop/vendor/gpui/GAMESYNC-PATCH.md`.
  The October 5 entry describes how `hinge` was added: one `CardPose` field,
  packed into the `hinge` `float4` uniform (`uniforms[20]` is `.x`).
  `uniforms[21..23]` are free. Follow the same pattern.
- **`CardPose` field docs** (`desktop/vendor/gpui/src/card_layer.rs:20-36`) say
  what each value means in one or two sentences. Imitate the `hinge` doc
  comment.
- **Exemplar for corner radii on GPUI divs:** `.rounded_l(..)`, `.rounded_r(..)`,
  `.rounded_tl(..)` and `.rounded_bl(..)` exist (`gpui::Styled`). `rounded_r` is
  already used in this file. `Pixels * f32` is supported.
- **Comments** explain intent and invariants in short, direct English. Do not
  narrate obvious code.

## Steps

1. **Add the pose field.** In `desktop/vendor/gpui/src/card_layer.rs`, add this
   field after `hinge`, inside `pub struct CardPose`:

   ```rust
       /// Fraction of the radius kept on the two corners at the hinge edge:
       /// 1 keeps them round, 0 makes them square. Ignored when `hinge` is 0.
       /// Hinged faces also skip the projected shadow; the caller draws one
       /// table shadow for the whole book.
       pub spine_round: f32,
   ```

   `Default` stays derived (0.0 is fine, because it is ignored when `hinge == 0`).

2. **Pass it to the shader.** In
   `desktop/vendor/gpui/src/platform/mac/card_renderer.rs`, inside `draw`, the
   uniform array currently ends:

   ```rust
               card.pose.hinge,
               0.,
               0.,
               0.,
           ];
   ```

   Change the first `0.` after `card.pose.hinge,` to `card.pose.spine_round,`.

3. **Skip the shadow pass for hinged faces.** In the same function, replace the
   `modes` block (`card_renderer.rs:252-256`) with:

   ```rust
           // A hinged face is one page of a book; the app draws a single table
           // shadow for the whole book, so the page casts no projected shadow.
           let modes: &[f32] = if card.pose.material && card.pose.frosted_top == 0. {
               if card.pose.hinge != 0. {
                   &[1., 0.]
               } else {
                   &[2., 1., 0.]
               }
           } else {
               &[0.]
           };
   ```

4. **Use the spine radius in the face mask.** In
   `desktop/vendor/gpui/src/platform/mac/card_shaders.metal`:
   - Add this function directly after `card_sdf` (after line 38):

     ```metal
     // Corners on the hinge edge use a fraction of the radius, so a turning page
     // can meet its neighbor at a square spine.
     float face_radius(float2 uv, constant CardUniforms &u) {
         bool spine = u.hinge.x != 0. && (uv.x - .5) * u.hinge.x > 0.;
         return spine ? u.shape.x * u.hinge.y : u.shape.x;
     }
     ```

   - Replace `float d = card_sdf(in.uv, u.rect.zw, u.shape.x);` (line 43) with
     `float d = card_sdf(in.uv, u.rect.zw, face_radius(in.uv, u));`.
   - Do not change the shadow branch (`u.shape.w == 2.`) or the edge branch
     (`u.shape.w == 1.`).
   - Why the side test works: `hinge = -1` puts the axis at the face's authored
     left edge (`uv.x < .5`), and `hinge = 1` at its authored right edge
     (`uv.x > .5`). The back face is authored upright, so the same test applies.

5. **Update the other `CardPose` literals,** which list every field. Add
   `spine_round: 1.,` after `hinge: 0.,` in:
   - `desktop/src/ui/grid.rs:341-348`
   - `desktop/src/ui/card_proof.rs:76-83`

   Do not touch `detail.rs:543` (it uses `..Default::default()` with
   `hinge: 0.`).

6. **Rebuild the book stage in `desktop/src/ui/detail.rs`.** Replace everything
   from the comment line `// Every page keeps the card's four rounded corners in every state.`
   (line 555) through the end of the `else { ... }` turning branch, which is the
   line before `if !spread {` (line 636). Keep `let (left, right) = self.pages(game, cx);`
   above it. The new code:

   ```rust
               // The book is one object. Its spine corners square off as it
               // opens, so the turn ends on exactly the settled spread, and one
               // table shadow follows its footprint instead of each page casting
               // its own.
               let open = (1. - angle.cos()) / 2.;
               let spine_radius = radius * (1. - open);
               let border = cx.theme().border;
               let paper = cx.theme().background;
               if spread {
                   stage = stage.child(
                       h_flex()
                           .size_full()
                           .rounded(radius)
                           .overflow_hidden()
                           .border_1()
                           .border_color(border)
                           .bg(paper)
                           .shadow(super::card::card_shadow(true))
                           .child(
                               div()
                                   .flex_1()
                                   .min_w_0()
                                   .h_full()
                                   .border_r_1()
                                   .border_color(border)
                                   .child(left),
                           )
                           .child(div().flex_1().min_w_0().h_full().child(right)),
                   );
               } else {
                   // While turning, the cover is a leaf hinged on the spine: its
                   // front is the card, its back is the left page. The right page
                   // lies flat beneath it.
                   let leaf = gpui::CardPose {
                       pitch,
                       yaw: yaw - angle,
                       material: true,
                       spine_round: 1. - open,
                       ..Default::default()
                   };
                   // The projected footprint on the table: the right page plus
                   // the part of the leaf that has swung past the spine.
                   let footprint = width * angle.cos().min(0.);
                   stage = stage
                       .child(
                           div()
                               .absolute()
                               .left(px(spine + footprint))
                               .top_0()
                               .w(px(width - footprint))
                               .h(dimensions.height)
                               .rounded(radius)
                               .shadow(super::card::card_shadow(true)),
                       )
                       .child(
                           // The leaf's back supplies the spine line when open,
                           // so this page has no left border.
                           div()
                               .absolute()
                               .left(px(spine))
                               .top_0()
                               .w(dimensions.width)
                               .h(dimensions.height)
                               .rounded_tl(spine_radius)
                               .rounded_bl(spine_radius)
                               .rounded_r(radius)
                               .overflow_hidden()
                               .border_t_1()
                               .border_r_1()
                               .border_b_1()
                               .border_color(border)
                               .bg(paper)
                               .child(right),
                       )
                       .child(
                           div()
                               .absolute()
                               .left(px(spine))
                               .top_0()
                               .child(gpui::card_layer(
                                   layer_id(1),
                                   dimensions,
                                   gpui::CardPose { hinge: -1., ..leaf },
                                   radius,
                                   div().children(front),
                               )),
                       )
                       .child(
                           div().absolute().left(px(spine - width)).top_0().child(
                               gpui::card_layer(
                                   layer_id(2),
                                   dimensions,
                                   gpui::CardPose {
                                       back: true,
                                       hinge: 1.,
                                       ..leaf
                                   },
                                   radius,
                                   // Square on the spine side; the shader
                                   // mask rounds it by `spine_round`.
                                   div()
                                       .w(dimensions.width)
                                       .h(dimensions.height)
                                       .rounded_l(radius)
                                       .overflow_hidden()
                                       .border_1()
                                       .border_color(border)
                                       .bg(paper)
                                       .child(left),
                               ),
                           ),
                       );
               }
   ```

   Notes for this step:
   - Child order is the paint order: table shadow, right page, front leaf, back
     leaf. Keep it.
   - `width` and `height` are `f32` in this function. `radius` is `Pixels`.
     `dimensions` is `Size<Pixels>`.
   - If `card_shadow` is not visible from `detail.rs`, it is
     `pub fn card_shadow` in `desktop/src/ui/card.rs:174`. The current code
     already calls it as `super::card::card_shadow`.

7. **Document the patch change.** Append this paragraph to
   `desktop/vendor/gpui/GAMESYNC-PATCH.md`:

   ```markdown
   October 7 spine: `CardPose::spine_round` scales the radius of the two
   corners on the hinge edge (uniform `hinge.y`), so a turning page meets its
   neighbor at a square spine. Hinged faces skip the projected shadow pass;
   the app draws one table shadow for the whole book.
   ```

## Boundaries

- Do NOT change the spring (`card_motion.rs`), the `spine` formula, `pitch`/`yaw`
  handling, the shield (`self.shield(...)`), the closed-card branch or the page
  builders (`self.pages`).
- Do NOT change the shader's shadow math, edge pass, clear coat or vertex
  transform. Only add `face_radius` and change the one `card_sdf` call.
- Do NOT touch the Blade/Linux renderer. Linux does not animate the book
  (`book_motion` is macOS only). The new field is ignored there.
- Do NOT add dependencies.
- If any excerpt above does not match the file (drift), STOP and report. Do not
  improvise.

## Verification

- **Mechanical** (from `desktop/`). Each of these must pass:
  - `cargo build`
  - `cargo clippy --all-targets`, with no new warnings
  - `cargo test`
  - `cargo fmt --check`. If it fails, run `cargo fmt` only on the files you changed.

- **Fixed-angle captures**, without input. Add a temporary probe, capture, then
  remove the probe completely. This was done before on this branch; see
  `plan/logs/2026-10-05.md`.
  1. **Probe in `GameSyncApp::render`** (`desktop/src/ui/app.rs`): when the
     environment variable `GAMESYNC_TURN_PROBE` is set and the detail view is
     not shown, select the first game with `lib.select_from_home(id)`, set
     `self.detail_shown = true`, and call `detail.present(cx)`.
  2. **Probe in `DetailPanel::render`:** when `GAMESYNC_TURN_PROBE` parses as
     degrees, set `self.open = degrees >= 90.`, use `turning = 0 < degrees < 180`
     and `angle = degrees.to_radians()`.
  3. **Run and capture:**
     ```sh
     GAMESYNC_TURN_PROBE=<deg> GAMESYNC_PREVIEW_DIR=<temp dir> target/debug/gamesync-desktop --demo
     ```
     Capture only the window with `screencapture -x -o -l<window id>`. Never
     send keystrokes or clicks; they reach the user's other apps.
  4. **Angles:** 0, 4, 20, 60, 90, 120, 160, 176 and 180.

- **Feel check** on the captures, and then by hand in the running app (Space
  turns the focused card):
  - 4° and 20°: no square corner and no second outline shows past the card's
    corners.
  - 160°, 176° and 180°: the inner (spine) corners are nearly square at 160°
    and square at 176°. 176° and 180° look the same apart from the small angle.
    There is no double line at the spine and no notch at its top or bottom.
  - 180°: the open book is one rounded outline with one divider at the spine.
  - 60°, 90° and 120°: there is exactly one shadow, lying flat on the table
    under the book's footprint, and its edges are soft. It does not rotate,
    shrink to a line, or appear on top of the right page.
  - Closed versus 4°: the shadow's darkness and offset do not visibly change.
    If the compositor shadow and `card_shadow(true)` differ, record the
    difference; do not tune values in this plan.
  - By hand: open and close a card 5 times in a row, and reverse it mid-turn.
    The shadow width tracks the page smoothly in both directions, with no frame
    where it jumps.
  - Turn on Reduce motion in GameSync Settings → Look and feel. Space must
    switch between closed and open at once, with the same settled visuals
    (single spread, single shadow).

- **Done when:**
  - All mechanical checks pass.
  - The captures meet every item above.
  - The probe code is gone (`grep -rn GAMESYNC_TURN_PROBE desktop/src` returns nothing).
  - `GAMESYNC-PATCH.md` has the October 7 paragraph.
