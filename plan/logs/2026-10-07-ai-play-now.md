# AI profile analysis for Play now

## Scope

- Branch `vova/ai-play-now` from `windows-test-build` (`4fd731b`).
- Implement the first AI feature in [Play now](../play-now.md#ai-profile-analysis--october-7):
  real provider requests and cached, validated game profiles.
- User decisions: all five providers in the first pass; a batch action in
  Play now plus a one-game action in the profile modal.
- The batch action is in the Play now toolbar, not the hand's preference
  strip, because the strip shows only after a deal.

## Changes

- `desktop/src/ai.rs`: provider list, key/address connection, model list
  (also the key test), and one schema-constrained JSON request. Anthropic
  Messages for Claude; OpenAI-compatible chat for OpenAI, Grok, Gemini, and
  Ollama. Short errors per HTTP status; provider text bounded to 200 chars.
- `recommendations/analysis.rs`: system prompt, request built only from
  fingerprinted inputs, strict schema, and per-entry validation by game ID.
- `RecordStore::set_recommendation_analysis`: guarded write that refuses a
  result for changed inputs. `Analysis` gains an optional `reason`.
- Settings → AI: real tests, discovered model lists saved in device
  settings, tested key/address replacement, and Refresh models.
- Play now: Analyze in the toolbar (confirm dialog, progress, Cancel,
  Retry), and an AI section with Analyze / Analyze again at the end of the
  profile modal. Details label AI-sourced profiles "AI estimated".

## Checks (macOS, October 7)

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: 200 passed, 0 failed, 11 ignored. New: 6 provider
  parsing tests, 6 analysis validation tests, 1 record store write test.
- Native GUI, `--play-now-demo` with an isolated `GAMESYNC_PREVIEW_DIR`, a
  temporary env probe (removed), and window capture without input:
  - Toolbar shows Analyze beside Saved and Recent.
  - With every demo profile complete, Analyze reports "Every eligible game
    has a current profile." (correct empty state).
  - Confirm dialog with two configured providers: count, request count,
    provider choice, what is sent, and the charge note.
  - Profile modal opens at the top. The AI section first shifted the
    initial scroll to the bottom when placed above the effort rows; it now
    sits at the end of the modal, and the modal opens at the top.
- After `5abb49c` (1.75 px line weight): Analyze uses the supplied
  `ai-beautify-stroke-rounded.svg`. Toolbar capture shows it at the same
  weight as Saved and Recent. 201 tests passed, 0 failed, 11 ignored.
- Request shape, against a local fake HTTP server (standard library only):
  7 tests for URLs, auth headers (`x-api-key` + `anthropic-version`, or
  Bearer, or none for Ollama), JSON bodies, `fallbacks` only for supported
  Claude models, HTTP errors without the key, and a stopped local server.
  Final run: 208 passed, 0 failed, 11 ignored; fmt and Clippy passed.
- Settings → AI in the running app, light and dark: provider rows with
  model pickers, Add a provider choices, key field, and disabled Test key.
- Dark mode: Analyze confirm dialog and the profile modal's AI section.

## Commit and test build

- Commit `434e9ef` on `vova/ai-play-now`, on top of `5abb49c`.
- Push not done: every push with the new commits failed with GitHub
  "Internal Server Error" (HTTPS and SSH, 15:10–15:14 UTC); githubstatus.com
  reported no incident. Creating the branch at `5abb49c` through the API
  worked, so the remote branch exists but lacks these commits.
- Test build: `python3 desktop/scripts/package_macos.py --output
  desktop/target/test-builds --name "GameSync AI"` (release, `--locked`).
- Output: `desktop/target/test-builds/GameSync AI.app` and
  `GameSync-AI-0.1.0-macos-arm64.zip` (14 MB). Signed with the local
  certificate; `codesign --verify --strict` passed. Not notarized.
- The test build uses the normal bundle ID, Keychain entries, and library.
- Launch check: the packaged binary started with `--play-now-demo` in an
  isolated `GAMESYNC_PREVIEW_DIR` and showed Play now setup. The log has
  about 90 `usvg` "Failed to parse color value: 'currentColor'" warnings;
  debug runs before `5abb49c` logged a similar count, so they are not new.

## Review fixes after the first live run

The user ran a live OpenAI analysis (`gpt-5.6-luna`) and asked for:

- The AI section first, with controls set to the suggested values. Chosen
  model: show suggestions, not copies. Controls select the effective value
  with a source tag; choosing a value makes it yours; Reset returns to the
  suggestion. "Automatic" buttons are removed. Minute fields show the
  suggestion as a placeholder.
- Activities directly after the AI section. Effort, session, and stopping
  controls collapsed under **Energy and session**, with a summary line.
- Clearer naming: **Get suggestion** / **Refresh suggestion**.
- The AI reason in game details in place of the generic Play now line.

Checks (macOS): fmt, Clippy, and 208 tests passed. Captured with a temporary
probe (in-memory sample suggestion, removed): collapsed and expanded modal
in light mode, minute placeholders in dark mode, and game details with the
AI reason. The modal opens at the top with the AI section first.

## Not verified

- No live provider request was made. No key was used, and the local Ollama
  install has no downloaded model. Request shapes follow current Anthropic
  documentation and the OpenAI-compatible format; Gemini and Ollama schema
  support (`anyOf` with null) is unconfirmed.
- Progress, Cancel, Retry, and error states were not seen with a real job.
- Narrow windows and keyboard focus for the new controls.
- The selected provider in the confirm dialog has low contrast against the
  unselected one (shared outline Button style).
- The Settings section icon for AI is still the stock `Bot` icon.
- Cached analysis does not sync yet (see the plan's remaining items).
