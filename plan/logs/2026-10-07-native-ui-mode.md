# Native UI mode

Branch: `native-ui-mode`, created from `windows-test-build` (`e6468d0`).
Goal: on macOS the app looks native, with system colors, the system accent,
native-style controls and the kit's window geometry. Windows gets the model
and colors now; its visual checks come later. Rules are in
[chrome and themes](../chrome-themes.md#native-look).

## Decisions (user, 2026-10-07)

- **Appearance:** Look and feel has Mode (Auto, Light, Dark) and Appearance
  (`<Platform>` | Theme). The platform is MacOS, Windows or Omarchy.
- **Native is the default** for new and existing installs. On Linux, the
  Omarchy segment replaces the "Sync with Omarchy theme" checkbox.
- **Saved values stay:** the native looks hide the scheme, contrast and accent
  rows, and the saved values return with Theme.
- **Scope:** the shell and the controls change; physical cards stay.
- **macOS Settings:** toolbar tabs and one form in both looks, after the Bear
  and System Settings references. Mode uses preview tiles.

## Changes

- **Model:**
  - `Look` (Native, Theme) and `NativePlatform` in `appearance.rs`.
  - A missing `look` loads as Native. The legacy `omarchy_mode` field is no
    longer read.
- **Native palettes** (`native_palette.rs`):
  - macOS values come from the Apple macOS 27 UI kit (Sketch); Windows values
    come from WinUI Fluent tokens.
  - Translucent text and border tokens are flattened over their surface.
- **System accent** (`system_accent.rs`):
  - macOS reads `NSColor controlAccentColor`; Windows reads the DWM
    `AccentColor` registry value.
  - It is read again when a window becomes active.
  - The macOS look draws white labels on the accent.
- **Shell:**
  - The `ShellStyle` global replaces the old "radius 0 means Omarchy" signal.
  - Window geometry: the main window puts the traffic lights at (18, 19) in a
    52 pt row. Settings and Collections put them at (13, 10) in a 34 pt row.
  - In the native macOS shell, the sidebar and content are flush, and the
    library toolbar and the detail header sit in the 52 pt titlebar row.
- **Controls** (`ui/controls.rs`): segmented control, switch, pop-up with
  up/down chevrons, and form rows with trailing-aligned labels.
- **Play now:** the selected segments use the accent fill, and the off
  switches are visible.
- **Home:** cover images are rounded inside their frames.
- **Book turn** ([plan 001](../../plans/001-book-turn-spine-and-shadow.md)):
  - `CardPose::spine_round` squares the spine corners as the book opens.
  - Hinged faces skip the projected shadow, and one table shadow follows the
    book's footprint.
  - The open book is one object again.
- **Elyx:** `Appearance.elyx` has the Theme, MacOS, Windows and Omarchy
  variants. `SwitchRow.elyx` is new.

## Checks

- **Code checks:** `cargo fmt --check`, `cargo clippy --all-targets` (no
  warnings) and `cargo test` (187 passed, 0 failed) all passed.
- **Window captures:**
  - Main window: the traffic lights are at the kit position.
  - Settings General tab: tabs on top.
  - Book turn: fixed angles from 0° to 180°, through a temporary probe that
    was then removed. At 0° and 4° the shadow profile matches within 1/255.
    It has the same darkness from 90° to 180°.
- **User checks:** the user saw the live book turn and confirmed that it
  works.
- **Book turn trace:**
  - Turns run at 120 Hz, about 2 ms of CPU per frame.
  - The first opening turn has one 11 ms frame, while the left page is drawn
    for the first time.
- **Test build:** `package_macos.py --output target/test-builds --name "GameSync Native"`.
  - Optimized release build. It is signed with the certificate and
    `codesign --verify --strict` passed. It is not notarized.
  - It uses the normal app ID and real data. The settings were backed up to
    `settings.before-native-ui.json`.

## Not verified / next

- **Not checked by eye** (no screenshot of mine covers them):
  - The Look and feel form, Mode tiles and Theme rows.
  - Native dark mode.
  - The detail header.
  - Live change of the system accent.
  - A real-data run of the test build.
- **Other platforms:**
  - Windows visuals: no Windows target is installed here.
  - Linux/Omarchy segment flow.
- **Missing tests:** `Look` serde and the migration, and the native palette
  keys.
- **Not yet built:** the other Settings tabs as forms; dimmed minimize and zoom
  buttons in Settings; sidebar row metrics; Play now on the shared controls;
  the Mode tiles in Elyx.
- **Elyx:** the user's uncommitted `SettingsShell.elyx` and `Switch.elyx`
  edits did not parse. These files were restored to `HEAD`, and the edits are
  kept outside Git.
