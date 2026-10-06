# GameSync design

This is the experimental Elyx project for GameSync. Keep the app's design
explorations, shared components, and screen designs together here. `.elyx`
files contain the designs; `elyx.json` contains project settings.

## About this project

GameSync is a cozy desktop game library for people who like to play games.
Help the user browse their library, track progress, and choose what to play.
Keep common actions clear and fast. Put advanced options one level deeper.

The app uses Rust and GPUI in `../desktop/`. Design for macOS and Linux
first. Windows follows data sync. Keep layouts practical for native desktop
input, resizable windows, and large libraries.

## Start here

- Read the repository [instructions](../AGENTS.md) and [plan index](../plan/README.md).
- Use the [app plan](../plan/app-plan.md) for scope and data rules.
- Use the [design guide](../plan/design.md), [physical cards](../plan/physical-cards.md), and [chrome and themes](../plan/chrome-themes.md) for the current direction.
- Read the [Elyx documentation](https://publish.obsidian.md/elyx/Elyx+Docs) and use the installed Elyx skill before editing `.elyx` files.
- Inspect the relevant existing designs and native UI before extending them.

## Design direction

- Use calm surfaces and clear text. Let game covers provide the main color.
- Treat games as physical cards: artwork and title on the front, details on the back.
- Keep useful left navigation and the Cards, Grid, Table, and Board views.
- Reuse semantic colors, spacing, and components. Support light and dark themes.
- Show focus and selection clearly. Account for keyboard input, resizing, long titles, and missing covers.
- Include loading, empty, and error states when designing a flow.
- Use motion to explain state and respond to input. Respect reduced motion.
- Keep personal edits, Steam data, and AI suggestions distinct.

These designs are experimental proposals. Record accepted product decisions
in `../plan/`; a mockup alone does not change the app's scope or prove
that a behavior works in GPUI.

## Structure

Read [README.md](README.md) and the [state index](state-index.md).

- `tokens/`: shared colors, context overrides, spacing, typography, and radii.
- `design/controls/`: reusable interface controls and their variants.
- `design/blocks/`: composed navigation, shells, library, Settings, and detail pieces.
- `design/screens/`: screen families with related state variants.
- `index.elyx`: a small entry canvas containing imported instances.
- `elyx.collections.json`: folder galleries; do not duplicate these as overview files.
- `resources/icons-new/`: the exclusive UI icon source, including solid variants.
- `resources/images/`: sample cover artwork.

## Working rules

- Imports flow from tokens/assets to controls, blocks, screens, then index.
- Use project-root imports and explicit source paths across instance boundaries.
- Each shared element has one source. Reuse it before adding another component.
- Group related states as variants; preserve their entries in the state index.
- Use stacks for all normal interface layout. Use content and parent-relative
  sizes. Keep positions only for intentional overlap, overlays, native window
  controls, and top-level canvas placement. Document each exception.
- Use semantic names. Do not keep alternate `_v2` components or compatibility copies.
- Never use Unicode symbols as substitute UI icons or copy SVGs into another folder.
  Preserve the supplied SVG bytes. Missing artwork requires an explicit decision.
- Keep the native app and sample-project read-only unless the user requests changes.
- Use sample content; never commit personal libraries, account IDs, or credentials.
- Preserve unrelated working-tree changes.
- Normalize with the installed Preview CLI, run full diagnostics, and inspect renders.
  Use `--editor-only` only to reproduce editor errors, not for final validation.
- Check dark contexts explicitly. Each standalone design imports the dark token file.
- Verify native behavior separately. A static Elyx design does not prove runtime behavior.
- Record accepted product decisions in `../plan/`; log outcomes and verification limits.
