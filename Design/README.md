# GameSync design workspace

Open [index.elyx](index.elyx) for the main references. Folder galleries provide
Controls, Blocks, Home, Library, Game details, Settings, Review, Tokens, Icons,
and Images. The [state index](state-index.md) maps the 90 original references and later additions.

## Where to edit

| Location | Owns |
| --- | --- |
| `tokens/` | Semantic colors, dark context, spacing, typography, radii |
| `design/controls/` | Buttons, icon labels, inputs, navigation rows, checkboxes, ratings, tags, menu items |
| `design/blocks/` | Sidebar, window shells, shared sample grid, cards, rows, filmstrip, Best on, messages, Settings groups |
| `design/screens/` | Nineteen screen families and their named state variants |
| `resources/icons-new/` | The only UI icon source; supplied SVG artwork is preserved |
| `resources/images/` | Bundled sample covers |

Imports flow from tokens/assets to controls, blocks, screens, then the index.
Edit a shared source to update its instances. Variants contain only intentional
state differences. Screen-local content remains in its family file. Menu and
dialog references reuse the shared library behind their overlay. No alternate
sidebar or copied overview atlas is retained. Add `flows/` only for a real
connected journey, not another gallery.

## Layout and component contracts

- Use stacks for rows, columns, forms, navigation, cards, lists, and toolbars.
- Use `.auto`/omitted sizes to hug content and `100%` for available stack space.
- Keep cover proportions, icon sizes, and native control heights explicit.
- Shell body and overlay slots are the insertion points for screen content.
- Use Checkbox, Rating, Tag, IconLabel, and MenuItem for their matching content.
- Keep normal labels inside their control. Do not use spaces or text glyphs as icons.
- Free positioning is limited to popup planes, physical artwork overlap, drag
  snapshots, native window controls, and top-level canvas placement.
- `colors.dark.elyx` is explicitly imported by design files. Context changes
  therefore apply when opening or rendering a file on its own.
- Hidden games uses the supplied EyeOff icon. Fractional ratings keep their
  numeric value beside whole-star SVGs.
- Supplied solid SVGs use `sizing: .fill` to avoid Preview's intrinsic percentage
  viewport sizing. This changes the icon fit, not the SVG artwork.

## Adding icons

1. Add the original SVG once to `Design/resources/icons-new/` from the repository
   root. Put filled variants in its `solid/` subfolder. Keep supplied filenames and bytes.
2. In Elyx, import `resources/icons-new/<filename>.svg` directly in the shared
   control or block that owns its use. The Icons gallery discovers files here.
3. For native use, add an entry to `ICONS` in `desktop/src/assets.rs` with
   `include_bytes!("../../Design/resources/icons-new/<filename>.svg")`. Reuse the
   existing component lookup key when replacing an icon; add an `IconNamed` role
   only when needed. Rebuild the app after adding or replacing embedded artwork.
4. Check Preview diagnostics and the affected light/dark screens; run native
   asset tests and inspect the affected app view.

Do not create another `desktop/icons` copy. Adding a file makes it available to
Elyx; mapping its intended role makes it appear in the app. Hidden games uses
`eye-off-stroke-rounded.svg`; discounts use `hot-price-stroke-rounded.svg`.

## Validate

Use the CLI bundled with the installed Elyx Preview. This cleanup was checked
with `0.0.1-preview.15345.gd9409cf06`. Recheck `--version` when updating the app;
the global CLI can use a different engine. Let Preview normalize resolved override paths.
Preview prunes unused imports during normalization: retain the explicit
`darkTheme` import afterward, or supply `--extra-file tokens/colors.dark.elyx`
when rendering. A context choice alone does not load its token file.

```sh
ELYX='/Applications/Elyx Preview.app/Contents/Resources/bin/elyx'
"$ELYX" normalize -w Design/design/screens/library/Grid.elyx
"$ELYX" diagnostics Design
"$ELYX" render Design/design/screens/library/Grid.elyx --symbol Grid -o /tmp/gamesync-grid.png
"$ELYX" render Design/design/screens/library/Grid.elyx --symbol Grid --context theme=dark -o /tmp/gamesync-grid-dark.png
```

Use the coverage table for per-symbol renders. Check main windows at 960×600,
1280×900, and 1600×1000; Settings at 700×480 and 820×780. Test long labels, large
counts, selection, hidden content, missing covers, feedback, and overlays.
A successful render is not a native interaction check. See the state index for
platform and source-derived limits. Keep screenshots and generated fixtures
outside Git. Use sample data only.
