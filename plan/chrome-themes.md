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

## Native look

`Appearance.look` is Native (default) or Theme. Native resolves per platform:
MacOS on macOS, Windows on Windows, and Omarchy on Linux while Omarchy state
is available. Without Omarchy, Linux falls back to Theme. A missing `look`
loads as Native. Native hides the scheme, contrast and accent rows. Their
saved values stay unchanged and return with Theme.

- **Colors:**
  - macOS uses the Apple macOS 27 UI kit values; Windows uses WinUI Fluent
    tokens (not yet checked on Windows).
  - Both live in `native_palette.rs` as Baseline keys. Translucent text and
    borders are flattened over their surface.
  - The system accent (`controlAccentColor`, or the DWM `AccentColor`) replaces
    the scheme accent. It is read again when a window becomes active.
  - On macOS, labels on the accent are white.
- **Shape signal:** a `ShellStyle` global (Theme, MacOs, Windows, Omarchy)
  tells surfaces which design language is active. Do not infer it from colors
  or radii.
- **Window geometry (macOS, every look):** GPUI sets the traffic lights only
  when a window opens.
  - The main window places them at (18, 19) in a 52 pt row.
  - Settings and Collections place them at (13, 10) in a 34 pt title row.
- **Native macOS shell:**
  - The sidebar and the content are flush, with no inset panel.
  - The library toolbar and the detail header sit in the 52 pt row, after the
    window controls.
  - The Theme look keeps the inset panel described above.
- **macOS Settings, in both looks:** a centered title, icon tabs on top, and
  one centered form.
  - Labels are trailing-aligned.
  - Mode uses Light, Dark and Auto preview tiles.
  - Appearance is a segmented control.
  - The Theme rows are pop-ups with up/down chevrons and accent checkboxes.
  - Sidebar and Motion are checkboxes.
  - Other platforms keep the left navigation for now.

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

### Omarchy mode

On Linux, the **Omarchy** segment of Appearance (Native look) follows Omarchy.
It replaces the former **Sync with Omarchy theme** checkbox. GameSync reads
the active theme name and semantic colors from
`$XDG_STATE_HOME/omarchy/current`, or `~/.local/state/omarchy/current` when the
environment variable is unset. It does not install an Omarchy hook or change an
Omarchy file.

The active Omarchy mode selects light or dark appearance and maps its background,
foreground, accent, selection, muted, and semantic colors to native GameSync
tokens. GameSync watches the stable `current` directory because Omarchy replaces
the nested theme directory atomically. A periodic check covers unavailable file
watching. Invalid or incomplete colors keep the last valid theme.

Omarchy mode also changes the interface shape language. Native controls, panels,
menus, search, segmented controls, notices, status chips, cards, covers, and
thumbnails use square corners. The sidebar uses Omarchy's primary background,
matching its shell surfaces instead of using a separate dark shade. These changes
apply only while Omarchy mode is on; normal GameSync surfaces return when it is
turned off.

The Omarchy segment is the Native choice on Linux. It is shown only when
Omarchy state is available, or while Omarchy is already active, so the user
can still choose Theme if that state disappears. The legacy `omarchy_mode`
setting is no longer read. While Omarchy is active, the bundled palette rows
are hidden but their saved values stay unchanged. Theme restores those values.

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
