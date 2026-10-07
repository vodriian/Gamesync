# Play now — requirements and native implementation

Status: the user accepted the MVP direction on October 6, 2026. The revised
HTML remains a demo. The offline native implementation merged in PR #7.
Opt-in AI profile analysis is on `vova/ai-play-now`; see
[AI profile analysis](#ai-profile-analysis--october-7).

## Goal

Help someone with a large library choose a game for the time and attention
they have now. Reduce the library to **at most three games**. Keep collection
management and choosing together; neither replaces the other.

Add **Play now** directly below Home in the sidebar's top navigation group.
Keep the existing library navigation and footer.

## Source analysis

Reviewed the supplied Obsidian `Recommendations.md`, the supplied Picked
screenshots, and `/Users/vova/Glaze/Picked/sources/` on October 6.
The document is a discussion, not an approved implementation specification.

| Source | What it establishes | Treatment |
| --- | --- | --- |
| User passages in the document | Limited time and energy; a box of owned games reduced to three; collection and choosing both matter | Core problem and three-card ceiling |
| Advice in the document | Play now name, separate mood/energy, behavioral profiles, weighted ranking, one reshuffle, learning | Proposals to evaluate; numeric examples are not measured evidence |
| Picked `renderer/main/home-view.tsx` | Feeling/time form, genre and scope filters, history, one displayed pick, skip/play and feedback | Reuse useful behaviors, redesign the flow |
| Picked `main/services/recommender.ts` | Hidden/finished/time/energy exclusions; favorites, rating, affinity, backlog and review boosts; random jitter; one pick plus four alternatives | Reuse the filter-then-rank structure; replace output size and undocumented tradeoffs |
| Picked `main/services/scoring.ts` | Genre heuristics, nine feeling mappings, four coarse duration bands | Bootstrap evidence only; do not promote guesses to facts |
| Picked `renderer/lib/use-reroll-limit.ts` | Three rerolls, then a ten-minute cooldown; choosing resets it | Replace with one optional reshuffle in this prototype; no timer lockout |
| Picked `main/services/library-store.ts` | Manual scoring values protected against heuristic/AI replacement | Preserve this precedence in GameSync |

### What needs to change

| Before | Proposed after | Why |
| --- | --- | --- |
| Nine feelings combine emotion, energy and intent | Time, energy, one optional activity, independent Brain dead toggle | Action can fit low attention without being mechanically easy |
| Long form followed by an expanding history feed | Compact setup, then a focused hand; history behind a separate action | Keep the decision visible |
| One pick with up to four alternatives in the service | Zero to three cards, with reasons for each | Respect the decision ceiling |
| Random score jitter | Deterministic ranking for the same snapshot and explicit variation on reshuffle | Make behavior explainable and testable |
| Generic genre-derived claims | Field evidence, confidence, and explicit unknowns | A genre does not establish stopping flexibility or cognitive load |
| Skip coupled to another recommendation | Not now removes a card from this hand; Undo restores it | Avoid an endless replacement loop and false dislike signals |

## First version

1. Open Play now. Start with 30 minutes, low energy, Anything, Brain dead off.
   Place Brain dead directly below the energy levels, before activities. Use
   a switch with the supplied brain icon and the existing short description.
   Preference groups use one selection pill that slides and resizes over
   200 ms. The switch thumb moves over 120 ms. Both use reversible ease-out
   motion. Keyboard input and Reduce motion apply each state immediately.
   These are editable prototype defaults, not inferred personal preferences.
2. Set available time: 15, 30, 45, 60, 120 minutes, or **∞**. Infinity means
   no time limit, not a very large session length. It removes the time filter
   and time-fit preference, but leaves energy and all other constraints intact.
   Store this as an explicit no-limit value; do not serialize numeric infinity.
3. Choose energy: Low, Medium, High, each with the supplied battery icon.
   Under **What do you feel like doing?**, choose one optional activity:
   Shoot, Fly, Drive, Explore, Ride a horse, Play cards, Multiplayer,
   Never-ending, or Weird. Anything removes this filter.
   A game can support several activities. Match their actual mechanics;
   do not infer horse riding from the RPG genre alone. Never-ending means
   repeatable runs, including roguelikes, not that a session cannot end.
4. Brain dead favors low cognitive load, little story recall, easy stopping,
   and low startup/onboarding effort. It does **not** silently change energy
   or disallow all action games. Mechanical effort remains a separate field.
   Place the supplied brain icon at 44 px before the text.
5. Put optional controls in More options: target setup (PC/Steam Deck),
   installed only, favorites only, and Any eligible/Unplayed/Playing scope.
   Installation is device-specific; unknown does not count as installed.
6. **Deal me 3** produces up to three unique games. If only one or two qualify,
   show those. Never pad with weak or ineligible choices.
7. Each card shows cover, title, a short reason, practical session length,
   setup information, and Play, Not now, Save for later, and Details actions.
   Do not show a fabricated match percentage.
   Save for later uses `clock-fading-stroke-rounded.svg`.
8. Allow one **Reshuffle** for a different eligible hand. Exclude already
   dealt games. Explain when fewer than three remain. Context edits retain
   the reshuffle budget and temporary dismissals; repeated identical edits
   return the same eligible hand. Restarting the app starts a new recommendation session.
   This is a helpful boundary, not enforcement against the user.
9. Keep saved picks and recent actions off the main decision surface.
10. The results heading has no extra Play now label. Use sentence case for
    card roles. Put an icon-only Customize button directly after PC in the
    preference strip; it opens a modal. Put Reshuffle at the right of that
    strip, with no hand footer or no-unseen-games message. Keep New session out.
11. Open recommendation cards over the current view with a shaded backdrop
    and close button. Omit the back button, thumbnail strip, and adjacent-game
    navigation. Preserve the hand and keyboard focus on close. Reuse the book
    layout and its Front/Details controls. Add a
    brief Play now section before Notes, with energy/session facts and icon
    actions for profile editing, saving, and exclusion. Edit profiles in a
    modal, retaining the open game and pending changes until Save or Discard.
12. Saved uses `bookmark-02-stroke-rounded.svg`; Recent uses
    `transaction-history-stroke-rounded.svg`. Use
    `customize-stroke-rounded.svg` for both edit controls. The setup title is
    "Let's choose a game for you". Remove the setup subtitle and decorative
    three-card preview until the user supplies a replacement state.
13. Each recommendation has Play, an icon-only Skip (`fast-forward`), and
    Save for later in that order. Skip uses the same outline style as Save.
    Remove the separate Why this game and Not now text controls.
14. Play opens a centered Now playing screen with cover, title, elapsed timer,
    and Done playing. Add the `hourglass` icon and elapsed timer to the Play now
    sidebar row. Keep library browsing available. Active sessions prevent new
    hands, reshuffles, preference edits, and additional Play actions.
    Persist the device-local start time before requesting Steam launch. Restore
    elapsed time after navigation, sleep, or app restart. Done playing clears
    the session only after saving; failed writes keep Retry/Discard recovery.
    A removed game must not prevent ending the timer. This is user-tracked
    session time, separate from Steam playtime and game status.

## Eligibility before ranking

- Use owned games or games explicitly marked available. **Hidden games are
  always excluded before ranking.** Apply this to the first hand, reshuffles,
  estimated/offline fallback, and saved-pick resurfacing. AI, high affinity,
  and filter relaxation cannot override it. Recheck current eligibility when
  showing a saved hand and before launch, in case another view hid a game.
  Also exclude deleted, unavailable, and explicitly excluded recommendations.
- Use existing status eligibility settings. Default to Backlog, Want to
  play, and Playing; exclude Paused, Completed, and Dropped unless enabled.
- Apply selected activity, scope, and installation filters before ranking.
  If no eligible game supports the activity, offer Anything explicitly.
  Do not silently substitute card games when the user asked to Drive.
- Exclude known target-device blockers. Reuse Best on's separate evidence
  and personal preference. Do not equate ProtonDB tiers with Valve support.
- With a finite budget, require a usable minimum practical session plus
  setup time to fit it. A rule-derived estimate is usable but must be marked
  Estimated. Confirmed contrary evidence always wins. Completion time is
  never session time. Ideal duration is a soft preference. With ∞, skip time
  checks and do not boost long games simply because there is no limit.
- In native v1, neither cognitive nor mechanical effort may exceed
  the user's energy budget. Brain dead adds cognitive, narrative, and
  friction limits; it does not replace the mechanical limit.
- Unknown values do not become confident matches. First try conservative
  local estimates and manual values, even when no AI provider is configured.
  If essential values are still unknown, show an insufficient-data state
  with manual profile editing; AI analysis is an optional improvement.
  Never silently relax a hard constraint to produce three cards.
- Show which active restrictions removed candidates. Offer specific,
  explicit relaxations. Never re-include hidden/unavailable games as a fallback.

## Ranking proposal

Use a pure function over a library snapshot, context, setup evidence, and
recent recommendation events. Inject the current time; no hidden clock or
unseeded randomness inside scoring. Stable game ID breaks ties.

Rank eligible games by context fit first: energy margin, useful
session length, stopping flexibility, and startup effort. Then apply bounded
personal affinity, continuity, and rediscovery boosts. A favorite or novelty
bonus cannot overcome an eligibility failure.

Choose the strongest fit first. Where similarly suitable choices exist,
prefer a different style second and a forgotten suitable game third.
These are opportunities, not compulsory slots. Do not force a wildcard or
label something forgotten without supporting history. Explicit reshuffle
chooses from the remaining eligible set.

The document's 0.30/0.20 weights are starting hypotheses. Tune them against
reviewed examples before production. The prototype uses a small deterministic
demo score and style diversity; it does not implement learned preferences.

## Profile data contract

| Record | Proposed fields and rules |
| --- | --- |
| Context | Available minutes or no limit; low/medium/high energy; optional activity; Brain dead; setup; optional scope filters |
| Game profile | Energy estimate with its cognitive and mechanical components, narrative dependency, startup minutes, onboarding load, minimum/ideal session minutes, stopping flexibility, supported activity IDs |
| Profile values | Explicit unknown; units and valid range; field-level source, confidence, updated time, profile version, input fingerprint |
| Precedence | Personal override → reviewed profile → validated AI estimate → local rule estimate → unknown. Preserve underlying values and provenance. AI is still an estimate, not verified fact |
| Device context | Existing Best on assessment and reasons, separate personal preference, known installed state, required equipment |
| Recommendation session | Session ID, snapshot/context, shown IDs, dismissed IDs, remaining reshuffle, optional chosen ID |
| Feedback | Game ID, session/context reference, action, timestamp. Keep personal affinity distinct from Steam metadata |

Use three levels for cognitive/mechanical/narrative/onboarding effort in the
first implementation unless evaluated examples justify more precision. Keep
session and startup durations in minutes. Avoid storing both a derived
Brain dead score and its inputs without a versioned derivation.

Do not infer current progression from total playtime. A first launch may
need a tutorial even when later sessions are short. Unknown onboarding
effort matters most for unplayed games in Brain dead mode.

## Offline first, AI optional

The picker uses one local selector, regardless of how profiles were created.
Deal does not need an AI request. Cached AI profiles remain usable offline.
No account, key, or AI setup is needed to start choosing.

| Available information | Behavior |
| --- | --- |
| Personal ratings or reviewed profiles | Use those values; retain the underlying source values |
| Cached AI profiles | Use validated fields locally; show AI estimated in Details |
| No AI analysis | Use versioned rules over existing tags, categories, descriptions where deterministic, and known personal fields; show Estimated |
| Too little evidence for a required field | Keep it unknown; offer manual correction or optional analysis |

Without AI, broad activities can often come from precise tags (racing → Drive,
deckbuilder → Play cards). Use conservative session and energy estimates with
their rule IDs and supporting metadata. Broad tags such as RPG are not enough
for a precise activity or a confident energy rating. Do not assume that a card
game is mentally easy, or that playtime proves the user is past a tutorial.

Lower confidence should reduce rank among otherwise comparable games and
soften the reason shown. Estimated results can still fill a hand, but show
an Estimated label and explain the uncertain field in Details. A known
constraint failure is never repaired by replacing it with an estimate.
Missing Brain dead evidence should not be displayed as confirmed suitability.

### First AI feature: recommendation profiles

Use the existing Settings → AI provider work. The first consumer should be
**Analyze for Play now**, not a separate chat or a new provider flow.

1. Analyze visible eligible games with missing profiles first. Offer selected
   games and stale profiles as later scopes. Hidden games are outside the
   default analysis scope as well as outside recommendations.
2. Request game ID, supported activities, overall energy (low/medium/high),
   cognitive and mechanical effort, practical session estimates, stopping
   flexibility, Brain dead supporting fields, short reasons, and uncertainty.
   Energy rates required effort, not game quality or difficulty alone.
3. Treat responses as suggestions. Validate IDs, enumerated values, ranges,
   minimum/ideal ordering, and evidence references before caching. Missing
   fields remain unknown. Never attach a result by its position in a batch.
4. Save per game with provider/model, profile version, input fingerprint,
   and timestamp. Retain successful results if a batch fails or is cancelled.
   Only changed inputs, an explicit refresh, or a profile-version change make
   another analysis necessary. Reopening Play now does not cause a new request.
5. Let users correct an effort or activity value in game details. Their value
   takes precedence and survives reanalysis. Start with explicit corrections;
   automatic preference learning and automatic game-process detection remain later work.

AI may improve coverage and nuance; it does not guarantee better judgments.
Evaluate local rules and AI profiles on the same reviewed examples, especially
short sessions, low energy, cognitive-heavy card games, and fast games with
little narrative recall. Calibrate how overall energy combines mental and
mechanical effort before native ranking; the current HTML still demonstrates
the earlier mechanical ceiling plus separate Brain dead checks.

Analysis errors belong to the enrichment job. They never block an existing
local hand. Continue with saved profiles or local estimates, and let the user
retry analysis separately. Never switch a local model request to cloud silently.

## Feedback and launch semantics

| Action | Effect |
| --- | --- |
| Skip | Dismiss for this recommendation session only; Undo; no rating/status change; no automatic replacement |
| Save for later | Explicit separate saved-pick signal; toggleable; do not silently change status or favorite |
| Not interested | Persistent recommendation exclusion with Undo and a later management surface; do not hide the game from its library |
| Play | Start one timed session at the click; request a local launch when available; block further picks until Done playing |
| Launch error | Keep the timer and context; offer Retry launch or Done playing |
| Done playing | Record the explicit session end, clear the local timer, and return to setup |

Launching does not mark played, completed, or Playing automatically. A Steam
URI accepted by the OS is not evidence that the game ran. Selecting Steam
Deck while on a PC does not imply remote launch support. Native launch must
show the correct action for the current device and verified availability.

## States and accessibility

Setup; loading/cancel; three/two/one results; no matches; missing profile
data; missing cover; all cards dismissed/undo; reshuffle exhausted; details;
saved/recent picks; exclusion/undo; chosen game; launch error/retry.

Use semantic controls, visible focus, named icon buttons, keyboard access,
Escape for dialogs, restored focus, readable contrast, and reduced motion.
Do not rely on swipes, hover, animation, or color to complete any action.
Use GameSync's shared icon sources, system font, warm chrome, inset content
panel, and framed covers. Support light/dark and narrower desktop windows.

## Prototype scope

Open [the HTML prototype](../prototypes/play-now/index.html). Run from the
repository root with `python3 -m http.server 4176 --bind 127.0.0.1`, then open
`http://127.0.0.1:4176/prototypes/play-now/`. Direct file opening also works.

- Twelve sample records include one hidden game and use existing local artwork.
  Hidden games do not enter the visible count or candidate pool. All fit, device, installed,
  and history values are invented fixtures, not researched game assessments.
- State stays in memory and resets on reload. Save/exclude actions demonstrate
  behavior only. Play opens a local confirmation; no Steam call is made.
- Review controls expose loading, empty, unknown-data, missing-art, and launch
  error states. They are prototype tools, outside the proposed product flow.
- Local heuristic generation and AI profiles are specified above, not wired
  into this HTML. Its fixed sample profiles do not prove recommendation quality.
- Native library integration, durable feedback, sync projection, and the Rust
  selector now live in `desktop/`. The HTML still simulates these operations.
  AI metadata analysis and learned preferences remain later work.

## Review decisions

The user accepted the MVP and requested the icon, unlimited-time, and activity
changes. Still evaluate conservative energy filtering, confidence labels,
one reshuffle, and whether device/scope controls need more prominence.

The accepted input direction replaces the older feeling-only input in
[Choose](app-plan.md#choose). The native rules implement conservative estimates; their recommendation
quality still needs a reviewed game set and real use.

## Next implementation stages

### Native implementation scope — October 6

Branch `vova/play-now-native` implements the accepted native layout, a pure
offline selector, manual profile corrections, recoverable saved/excluded picks,
recent choices, and local Steam launch requests. It reuses game revisions,
status eligibility, Best on evidence, the shared image cache, and supplied SVGs.
AI provider requests remain the next stage: the existing provider settings are
not a working analysis service yet. Cached analysis has a separate validated
profile contract; manual fields take precedence.

Energy in this version is the higher of cognitive and mechanical effort.
Brain dead additionally requires low cognition, story recall, and onboarding,
plus a flexible stopping point. This keeps demanding card games out of Low
energy without assuming that all action needs substantial thought. Local tag
rules are estimates, not reviewed facts; insufficient evidence stays unknown.

Acceptance: hidden/status/ownership/device exclusions on every selection and
launch path; at most three unique deterministic picks; no-limit time; explicit
empty-state relaxations; persistent personal feedback independent of Steam;
guarded writes and retryable failures; keyboard/native resize review on macOS.
No AI call, status change, or claim of verified play follows a recommendation.

### Delivered and remaining

- Delivered: pure selector; versioned profiles and manual corrections; native
  setup, hand, details, save/exclude/undo, recent choices, and empty states;
  guarded personal revisions and sync projection; local Steam manifest checks.
- Energy uses the greater of mental and mechanical demand. Tag estimates do
  not establish onboarding or stopping flexibility. Brain dead can therefore
  need manual profiles until enrichment is available.
- Recent choices store the context and timestamp, with distinct no-launch,
  OS-accepted, and failed-launch outcomes, plus an optional explicit end time.
  Launch retries update the same session entry. No outcome proves that a game ran.
  History holds the latest 20 choices per game; the recent view shows the last
  choice for each game. Generated hands and temporary dismissals are not synced.
- Native profile precedence is manual → current validated cached AI profile →
  local estimate → unknown. A curated reviewed-profile tier is not implemented.
  AI source/confidence/fingerprint are profile-wide; manual source is per field.
- Native saved and excluded picks share a management view. Hidden/unowned
  records never appear there. Excluded games can be allowed again explicitly.
- Installation evidence is read at startup, and checked again before an actual
  launch. A remote setup never inherits this computer's install evidence.
- Remaining: opt-in cloud/local profile analysis, reviewed calibration fixtures,
  Linux native acceptance, and a live installed-game launch check.

See [native implementation checks](logs/2026-10-06-play-now-native.md).

### AI profile analysis — October 7

Branch `vova/ai-play-now` implements the first AI feature above. The user
chose all five providers and two entry points: a batch action and one game.

- **Providers.** Claude uses Anthropic Messages with `output_config.format`
  (JSON schema). OpenAI, Grok, Gemini, and Ollama use OpenAI-compatible chat
  completions with a strict `json_schema` response format. Claude Fable 5.1,
  Opus 5.5, Opus 5, and Sonnet 5.5 requests add `fallbacks: "default"` for
  safety-classifier declines. Refusal and truncated answers fail the batch.
- **Batch.** Analyze sits in the Play now toolbar, next to Saved and Recent,
  so it is available on setup as well as the hand. It targets visible,
  owned, status-eligible, not-excluded games with no current analysis and an
  incomplete manual profile. A dialog shows the count, request count,
  provider choice, what is sent, and a possible charge before any request.
  Demo libraries do not show it.
- **Analyze games (Settings → AI).** Under the provider list, shown only when
  a provider has a chosen model and the library is not a demo. It adds Play
  now data to every library game without a current analysis: all statuses,
  including games excluded from Play now. Hidden, removed, and unowned games
  are skipped. The row shows the newest analysis time ("Last: 3 days ago")
  or live progress, with Analyze, Cancel, or Retry. It uses the same confirm
  dialog as the toolbar. Play now and Settings share one job (`ui::analysis_job`),
  so both show the same progress and only one job runs at a time.
- **Profile modal (user review, October 7).** Order: AI suggestion (provider,
  model, confidence, reason, and Get suggestion / Refresh suggestion), then
  Activities, then **Energy and session**, collapsed by default with a
  one-line summary. Each control selects the effective value and tags its
  source: AI suggestion, Estimate, Yours, or Unknown. Choosing a value makes
  it yours; Reset returns the field to the suggestion. Minute fields show the
  suggestion as a placeholder. Only the user's values are saved, so a later
  analysis still updates the rest. The click is the request; no extra dialog.
- **Input.** Title, Steam short description (at most 1,500 characters), tags,
  and genres: exactly the fingerprinted inputs. No notes, ratings, history,
  or personal tags.
- **Validation.** Batches of 8. Malformed JSON fails the batch. Entries attach
  by game ID; unknown and duplicated IDs are dropped. Out-of-range values and
  unknown enum values become unknown; minimum above ideal clears both.
  Confidence outside 0–100 drops the entry.
- **Writes.** Each result is saved by game ID when its batch returns, under the
  game lock, only if the game's fingerprint still matches. Cancel stops after
  the current request. A failed batch keeps earlier results and offers Retry
  for the remaining games, including that batch. Deal never waits for analysis.
- **Display.** Details show "AI estimated" when any field comes from AI. Outside
  a hand, the AI reason replaces the generic Play now line in game details.
  With a current analysis, the energy/session line also shows the game's
  activity icons, each with its name as a tooltip. A saved profile shows no
  note in game details; the modal closes on save.

Remaining: a live request check for each provider, reviewed calibration
fixtures, sync of cached analysis (data-sync lists AI results as synced; the
operation projection does not include them yet), stale/selected scopes, and
cost estimates.

## Editable native design references

The [Elyx Play now family](../Design/design/screens/play-now/PlayNow.elyx)
records 12 settled states of this implementation, using the shared shell,
controls, and game book. See the [state index](../Design/state-index.md#play-now)
and [delivery checks](logs/2026-10-07-play-now-delivery.md). The earlier HTML
prototype remains a design reference; native session and motion behavior is
specified here and in the later logs.
