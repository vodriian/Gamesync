# Chrome layout and Baseline themes

## Shell

The sidebar remains 255 points wide on a solid chrome surface. The library
toolbar and browsing area share one content panel with an 8-point outer inset,
12-point corners, and a 1-point border. The panel edge replaces the sidebar
divider. Scrollbars and card-shadow gutters stay inside the panel.

The main window has no visible title. Its library name remains native window
metadata. Native window buttons and titlebar gestures remain available. The
sidebar reserves their height; a collapsed sidebar moves that reservation above
the panel. Focused details use the same panel without the sidebar and retain the
filmstrip. Dimensions come from the rendered content bounds.

Toolbars use solid surfaces. There is no canvas-wide frost capture or fixed
blue-gray tabletop. Physical-card rendering remains in place. GPUI clips overflow
to rectangles, so a native rounded frame and outline finish the rounded edge
without capturing the full library into a texture.

## Appearance

The bundled catalog ports all 25 named Baseline schemes from revision `8c56e831e1abb1d3841c4ffdecbe06b5182fbc68`. There are 45 supported
scheme/mode pairs. Unsupported modes are not offered in Settings.

Defaults: Auto mode, Sanctum light, Notion dark, Normal contrast in each mode,
Interface accent on, and Color scheme accent on. Light contrast offers Normal,
Tonal, and Vivid. Dark contrast offers Normal, Tonal, and Black. Tonal shares the
content and chrome background; Vivid keeps light content within dark chrome;
Black uses black dark-mode backgrounds. These are surface choices, not
accessibility guarantees.

Disabling Color scheme accent selects a shared blue. Disabling Interface accent
makes primary interface controls neutral; semantic warning/error colors remain.
Vivid chrome, sidebar controls, and menus have independently resolved colors.

`Appearance` is a typed settings value. Changes apply to all open windows and
persist through the atomic settings writer. Auto observes native OS appearance
without replacing the selected schemes. A save revision prevents an older queued
appearance update from overwriting a newer choice.

Legacy Auto/Light/Dark values select the new default pair with their old mode.
Named presets map to corresponding Baseline palettes and retain their mode;
Tokyo Night maps to Notion dark. The old `theme` string remains migration metadata.
Unknown scheme IDs fall back to the per-mode default at resolution time. Unrelated
settings and game data are retained.

## Sources and regeneration

The source is [Baseline](https://github.com/aaaaalexis/obsidian-baseline/tree/8c56e831e1abb1d3841c4ffdecbe06b5182fbc68/src/color-schemes).
Author credits and the upstream license are in `desktop/licenses/baseline/`.
`desktop/scripts/import_baseline.cjs` is a development-only importer. Run it with
an unpacked checkout at the pinned revision and a Node environment containing
Playwright and Chrome:

```sh
NODE_PATH=/path/to/node_modules node desktop/scripts/import_baseline.cjs /path/to/obsidian-baseline
```

It resolves inherited CSS colors against an explicit neutral Obsidian base and
writes `desktop/src/themes/baseline.json`: 270 content/chrome palettes covering
supported modes, contrast variants, and scheme accent choices. Runtime code uses
bundled native tokens, not CSS, browser downloads, or a new theme framework.
