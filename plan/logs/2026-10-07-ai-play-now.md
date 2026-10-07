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
