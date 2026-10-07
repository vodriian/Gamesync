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
- Push: first attempts failed with GitHub "Internal Server Error" (HTTPS and
  SSH, 15:10–15:14 UTC; no incident listed). A later push succeeded.
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

Commit `8f098db`, pushed. Test build `GameSync AI` rebuilt from it (same
command and output paths as above); the packaged app launched in an isolated
preview folder.

## Second review round

- Game details no longer show "Your profile was saved." after the profile
  modal closes.
- With a current analysis, the energy/session line in game details shows the
  activity icons (tooltip: activity name).
- Settings → General uses the AI provider pattern for the Steam key: an add
  form without a key; with a key, a row with an info button and a dialog
  (profile, key, Remove key, Save). Save tests first. A test that cannot start
  clears the save request, so a later manual test never saves by itself.

Checks (macOS): fmt, Clippy, and 208 tests passed. Captured with temporary
probes (removed): add form and saved row (light), dialog (light and dark),
and game details with three activity icons. A real Steam test and save
through the new dialog were not run.

## Third review round

- Settings scroll fix: the content column used `w_full().max_w(640)`. Wrapped
  text was measured narrower than drawn, so the scroll range was short and the
  end of Best on → Enrichment was cut off. A definite 640 px width fixes it
  (window minimum is 700 px). Probe measured the range at 292 px before and
  389 px after; the capture shows the full Enrichment group and its padding.
- Analyze games in Settings → AI (user request): one shared `AnalysisJob`
  entity now runs analysis for Play now and Settings. Library scope covers
  every game except hidden, removed, and unowned ones. Bin unit test
  `library_scope_skips_hidden_but_not_finished_or_excluded_games` added.
- `settings::age_label` now formats both the Steam sync time and the last
  analysis time.

Checks (macOS): fmt, Clippy, and 209 tests passed. Captured with temporary
probes (removed): Settings → AI with two providers, light and dark; Best on
scrolled to the bottom. A real library run from Settings was not done.

## Fourth review round

- Game details: the Estimated / AI estimated label is gone; activity icons sit
  at the right end of the energy/session line. A wand button before Edit game
  profile runs a one-game analysis. It shows only with a ready provider
  (`AnalysisJob::ready`, kept current by Settings → AI) outside demos.
- Settings title row reads "Settings" instead of the section name.
- Best on tag rules and equipment now match your tags as well as Steam tags
  (`suitability::assessment_in` joins both, read-only). Typing in Prefer PC or
  Prefer Steam Deck suggests matching library tags from both sources
  (`editor::suggest_tags`, now crate-visible), with a source tooltip; an
  unknown tag shows a short hint. Lib test
  `tag_rules_match_your_tags_as_well_as_steam_tags` added.

Checks (macOS): fmt, Clippy, and 210 tests passed. Captured with temporary
probes (removed): game details with the wand and right-aligned icons; Best on
with "ca" (your "Cards" and Steam "Card Game" listed), with an unknown tag, and
empty. Tooltips and a real one-game run from the wand were not checked.

Test build rebuilt from this round's working tree: `GameSync AI.app` and
`GameSync-AI-0.1.0-macos-arm64.zip` (14 MB). `codesign --verify --strict`
passed; not notarized. The packaged binary opened a window with
`--play-now-demo` in an isolated preview folder and logged no errors besides
the known `usvg` currentColor warnings.

Gap fix: with activity icons, "energy" ran into the clock icon. The row was one
wrapping flex row with `ml_auto` on the icons, and its items could shrink, so
the energy text overflowed into the gap. Now the facts sit in their own
wrapping group (`flex_1`, items `flex_shrink_0`) and the icons follow as a
fixed group. Probe capture with three icons shows the full gap. Checks: fmt,
Clippy, 210 tests passed; test build rebuilt, `codesign --verify --strict`
passed, packaged binary opened its window with no new errors.

## Pull request

- Commit `9457ec8` (fourth round and gap fix), pushed to `vova/ai-play-now`.
- PR #9 opened against `windows-test-build`, the branch this work started from
  (`origin/main` does not have PR #8 yet), so the PR shows only these commits.

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
