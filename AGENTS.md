# GameSync

Build a cozy desktop game library. Keep the app simple, clear, and fast.

## Start here

- Read [plan/README.md](plan/README.md) and [plan/guardrails.md](plan/guardrails.md).
- Read [plan/app-plan.md](plan/app-plan.md) for scope and data rules.
- Before GUI work, read [plan/design.md](plan/design.md) and its relevant skill sources.
- Keep experimental Elyx designs in [Design](Design/). Read its [project instructions](Design/AGENTS.md) before design work there.
- Read only the latest relevant log. Do not load the full project history.

## Work rules

- Build the Rust/GPUI app in `desktop/` on a feature branch.
- Keep the existing web app as a reference until the user requests its removal.
- Treat Eagle and Picked as source references. Do not change those projects as part of this work.
- Use small, focused changes. Add no framework or dependency without a current need.
- Comment intent, invariants, and platform limits. Do not narrate obvious code.
- Keep user edits separate from Steam data and AI suggestions.
- Use short, direct English guided by ASD-STE100. Keep necessary technical terms.
- Update the plan when a decision changes. Log outcomes and checks, not transcripts.
- Never claim a build, test, or visual check passed without evidence.

## Cursor Cloud specific instructions

- `.cursor/cloud-install.sh` installs GPUI's Linux libraries, Postgres, and Rust 1.99.0. Vendored GPUI uses edition 2024, so the image's Rust 1.83 cannot build `desktop/`.
- `npm ci` fails: `package-lock.json` omits optional `bufferutil`. The install script uses `npm install --package-lock=false`.
- `.cursor/cloud-start.sh` starts Postgres, pushes the Drizzle schema, and runs `npm run dev`. The web app listens on `localhost:5001` (IPv6). Use that host, not `127.0.0.1`.
- Build and run the native app with `python3 desktop/scripts/dev.py --demo`, or run `desktop/target/debug/gamesync-desktop --demo` after `cargo build --manifest-path desktop/Cargo.toml`. On a display without a GPU, set `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`.
- Desktop checks: `cargo test --manifest-path desktop/Cargo.toml`. Web typecheck: `npm run check`.
