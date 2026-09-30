# GameSync design

This is the experimental Elyx project for GameSync. Keep the app's design
explorations, shared components, and screen designs together here. `.elyx`
files contain the designs; `elyx.json` contains project settings.

## About this project

GameSync is a cozy desktop game library for people who like to play games.
Help the user browse their library, track progress, and choose what to play.
Keep common actions clear and fast. Put advanced options one level deeper.

The app uses Rust and GPUI in `../../desktop/`. Design for macOS and Linux
first. Windows follows data sync. Keep layouts practical for native desktop
input, resizable windows, and large libraries.

## Start here

- Read the repository [instructions](../../AGENTS.md) and [plan index](../../plan/README.md).
- Use the [app plan](../../plan/app-plan.md) for scope and data rules.
- Use the [design guide](../../plan/design.md), [physical cards](../../plan/physical-cards.md), and [chrome and themes](../../plan/chrome-themes.md) for the current direction.
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
in `../../plan/`; a mockup alone does not change the app's scope or prove
that a behavior works in GPUI.

## Structure

Start with the existing `tokens/colors.elyx`. Add folders only when needed:

- `tokens/`: shared design values.
- `controls/`: buttons, toggles, text fields, and other small controls.
- `blocks/`: reusable cards, navigation, toolbars, dialogs, and panels.
- `screens/`: complete windows and app views.
- `flows/`: related screens and states that explain a user journey.
- `resources/icons/` and `resources/images/`: local design assets, as configured in `elyx.json`.

## Working in this project

- Follow the patterns in existing `.elyx` files.
- Preserve the project's established design language and conventions.
- Keep changes focused. Reuse tokens and components before creating new ones.
- Use sample content. Do not copy personal libraries, account IDs, or keys into designs.
- Run Elyx diagnostics and inspect a render after design edits. Report which checks ran and any limits.
- Verify implemented UI in the native app separately. An Elyx render is a design check.
- Keep this file current as the project evolves.
