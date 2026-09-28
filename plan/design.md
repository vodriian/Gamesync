# GUI design

## Intent

A cozy game library for geeks who like to play games. The app should help the
user browse, track, and choose. Keep common actions easy to find.

Use calm surfaces, clear text, and game covers as the main source of color.
Keep advanced options one level deeper. Use tactile edges, shadows, and responsive light where they make the cards
feel physical. Avoid motion that delays work.

## Current direction

The September 21 card references replace the inspector with a focused card.
Keep the useful left navigation sidebar while browsing. Treat games as physical cards: artwork and title on the front, personal
and provider details on the back. Use Cards, Grid, and Table presentations.
See [physical cards](physical-cards.md) for scope and native rendering limits.
Eagle remains a source for infrastructure, not the visual target for this UI.

## Foundation reference

The user's Eagle GUI template is the existing `eagle-linux` project at
`/Users/vova/Developer/Github/eagle-linux/`.

| Source in Eagle | Reuse |
| --- | --- |
| `src/ui/app.rs`, `src/ui/sidebar.rs` | Sidebar, toolbar, central content, and optional inspector |
| `src/ui/grid.rs`, `src/ui/thumb_cache.rs` | Virtualized browsing and bounded image memory |
| `src/ui/detail.rs`, `src/ui/setup.rs` | Detail layout and library selection patterns |
| `src/theme.rs`, `src/themes/` | Theme tokens and desktop-aware colors |
| `src/ui/motion.rs`, `plans/README.md` | Existing motion primitives and GPUI limits |

Use this source before inventing new components. Inspect its current rendered
layout before matching dimensions or claiming visual fidelity. Source inspection
alone does not prove how a screen looks or feels.

The source has local edits. Port needed code from the inspected working tree
without modifying it. Record the source commit and relevant local differences
when the port starts. Keep upstream attribution where code is reused.

## Skill sources

- [Apple Design](/Users/vova/.agents/skills/apple-design/SKILL.md)
- [Product Design index](/Users/vova/.codex/plugins/cache/openai-curated-remote/product-design/0.1.54/skills/index/SKILL.md)
- [Emil Design Engineering](/Users/vova/.agents/skills/emil-design-eng/SKILL.md)

These paths describe this workstation. If a path moves, locate the same skill
in the installed catalog. Use the principles below as the project baseline.
The user's Rust/GPUI target and Eagle reference take precedence over generic
web templates in a skill. No web prototype or image generation is required
for the documentation foundation.

## Apply the principles

| Source | Project rule |
| --- | --- |
| Apple | Immediate feedback, predictable actions, clear hierarchy, direct manipulation, and easy recovery |
| Emil | Fast repeated actions, careful spacing, useful defaults, and restrained motion |
| Product Design | Start from the supplied reference; verify working behavior and visual fidelity |

Use system fonts and platform conventions. Keep controls readable in light
and dark themes. Use a shared spacing and color scale from Eagle before adding
new values. Give icons accessible labels and visible focus states.

Keep view selection and search near the library. Open game details on the back of a focused card without losing library scroll position. Keep one selection model across
the grid, table, and board. For game behavior, use the app plan.

## Motion and input

- Give press feedback immediately. Commit the action on release when applicable.
- Keep keyboard navigation, selection, search results, and frequent actions immediate.
- Use short transitions only to clarify a state or panel change. Start with Eagle's 120 ms and 200 ms timing values.
- Do not copy the viewer's 70 ms keyboard fade into routine library navigation.
- Track functional drags directly. Keep the point where the user grabbed the card under the pointer.
- Allow cancellation and reversal. Do not block input while an animation runs.
- Use springs only where an interruptible gesture needs them. Do not add a general physics system in advance.
- Respect reduced motion; use static changes or brief fades. Provide an app setting if the platform signal is unavailable.
- Add no grid-entry stagger, looping decoration, or automatic sound.

## GPUI limits

Translate the skills' intent into GPUI. Do not copy CSS or browser performance
claims into Rust code.

Eagle records three limits in its pinned stack: subtree opacity can make
overlapping children bleed through; animated size changes can relayout virtual
lists each frame; `div` scaling does not work like CSS transforms. Verify these
limits against the version in use before writing an effect.

Use color or border changes for press feedback when scaling adds complexity.
Use opaque theme surfaces by default. Add translucency only when it improves
hierarchy and remains readable and fast on both target platforms.

## GUI checks

Compare the native screen with the supplied physical-card references.
Check alignment, cover crop, text truncation, contrast, focus, keyboard use,
resizing, and scroll stability. Test loading, missing covers, empty libraries,
and errors. Keep controls usable while background work runs.

When reviewing UI code under the Emil skill, use a `Before | After | Why`
table for findings. Log platform coverage and any missing visual checks.

## References
- themes: https://github.com/aaaaalexis/obsidian-baseline/tree/main/src/color-schemes

## Native chrome and themes

Use the [chrome and Baseline theme specification](chrome-themes.md) for shell
geometry and appearance. It replaces the old palette selection and KDE overrides.
Keep the sidebar on solid chrome and the toolbar inside the rounded content panel.
Use semantic native tokens for content and chrome; Vivid must retain separate
foreground colors for each surface.
