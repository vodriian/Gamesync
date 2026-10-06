# Best on smart collection

Branch: `best-on-smart-collection`.

## Goal

Help choose between Steam Deck and PC. Keep compatibility, suitability, and
personal preference separate. Both is a valid result. Missing evidence is not Both.

## Delivery order

The user requested a step-by-step implementation: prototype first, tests next.

1. **Native demo prototype — implemented, ready for review.** Add Best on to the existing Smart
   collections. Show Steam Deck, PC, Both, and Needs review. Use bundled mock
   assessments and in-memory preference changes. Show reasons and a Why panel.
   Build and inspect the native prototype before adding automated tests.
2. **Behavior tests.** Test classification boundaries, unknown and blocked
   setups, preferences, counts, shared scopes, and demo isolation. Test record
   defaults and settings compatibility before enabling durable writes.
3. **Evidence and durable preferences — basic version on `details-card-v2`.**
   See [Steam evidence](#steam-evidence). Remaining: refresh of stale
   evidence, Linux runtime check, and calibration against reviewed games. Validate Steam payloads, fetch Deck
   assessment and controller support, add source/time/version tracking, and
   save personal choices through the existing guarded revision path. Preserve
   old records, unknown fields, and conflicting edits. Keep saved data offline.
4. **Weighted suitability.** Calibrate the rubric against reviewed fixtures.
   Deck uses portability, ergonomics, and technical fit. PC uses hardware,
   input, and immersion benefit. Avoid repeated bonuses from one evidence source.
5. **Automatic analysis and setup profiles.** Implement one real AI provider
   path. Validate proposed factors by game ID; Rust calculates results. Add
   Windows/Linux setup context and bounded preference adjustments. Hardware
   detection and a performance database remain deferred.

## Prototype contract

- Launch with `python3 desktop/scripts/dev.py --best-on-demo`.
- Opening the macOS `BestOnDemo.app` bundle without flags also selects demo
  mode. It has a distinct executable and bundle ID.
- The mode creates a fresh `BestOnDemo.library` store in app storage at each
  launch, seeded from the fixtures with their assessments. The full card editor
  works, including notes. It does not open the real Steam library or the
  persistent sample store. All changes reset on restart.
- Assessments are invented interaction fixtures, not current game benchmarks.
  PC means a capable Windows desktop; Deck means local handheld play. These
  assumptions define the demo fixtures. Do not infer Linux compatibility.
- Reuse `SmartKind`, `SmartRule`, cached counts, and shared scope filtering.
  Membership is exclusive. Both games appear in Both, not in two counts.
- Prototype scores are supplied by fixtures, not produced by AI or a weighted
  engine. The provisional classifier uses a 15-point difference and a minimum
  score of 50 for a suitable setup. Unknown scores or two poor/blocked setups
  return Needs review. One known blocker only selects the other setup when
  that setup has sufficient evidence and meets the minimum.
- Store assessment and personal choice in separate optional typed fields.
  Existing records omit both. This step only mutates in-memory fixture records;
  production enrichment and preference writes are not enabled.
- Automatic uses the assessment. Steam Deck, PC, and Both are explicit personal
  choices and determine collection membership. An incompatible personal choice
  retains the warning and does not alter the automatic recommendation.
- The Best on panel starts collapsed. Its summary shows only the setup name
  (Steam Deck, PC, Both, or Needs review), without repeating “Best on” or a leading chevron. A More details button
  below the title reveals reasons, blockers, and Your preference. A nested Why
  disclosure shows fit scores with a short explanation.
- Book details (`details-card-v2`): the panel sits between rating and notes on
  the right page. The setup name and confidence always show. More details
  reveals one fit row per setup (supplied handheld or monitor icon, score bar,
  reason, blocker), a short note, and Your preference. It starts collapsed
  because more metadata will join it. Recommended setups have a filled row.
  Unknown and blocked fits show an empty bar, not zero. A preference saves
  through the editor's guarded write into the demo store.
- Show separate recommendation confidence beside the setup title, using neutral
  theme colors. Confidence is an optional 0–100 fixture value, not the fit score
  or score difference. Missing values show Unknown. The Demo label and expanded
  explanation identify sample values. Real confidence needs an evidence-quality
  model in the later scoring phase.
- No new dependency. No network or AI calls to populate the prototype.
- Best on uses the supplied `computer-stroke-rounded.svg` directly from
  `Design/resources/icons-new/`; the SVG is unchanged.
- Keep new sidebar expansion state out of persisted settings until compatibility
  is tested. Existing view keys remain strings.

## Steam evidence

User decision (October 6): real libraries use Steam evidence, not demo values.

- Steam sync has a fourth resumable stage per game. It reads Valve's Deck
  report (`ajaxgetdeckappcompatibilityreport`, an undocumented store endpoint)
  and store categories (`appdetails`, `filters=categories`). It saves
  `SetupEvidence` in `steam.metadata.setup`: Deck rating, Valve test notes,
  controller support (category 28 full, 18 partial), and check time.
  A missing value means not fetched; a later sync retries it.
  **Resync details** refreshes it for one game. Stale evidence is not
  refreshed automatically yet.
- The parser accepts only known fields, short ASCII test tokens, and at most
  twelve notes. A failed report stays retryable; an unrated game is Unknown.
- `suitability::assess` calculates the result on each load. It is never saved,
  so rule changes apply to old evidence. A saved demo assessment wins.
- Rules: Deck Verified 85, Playable 65; Unsupported is a blocker; Unknown has
  no score and gives Needs review. PC starts at 75 (a capable Windows PC).
  Full controller support adds 5 to Deck. No controller support removes 15
  from Deck and adds 10 to PC. After two hours of play, at least 60% on Deck
  adds 10 to Deck; at most 20% adds 10 to PC. Deck share uses total playtime,
  because Steam can also count Deck time as Linux. The existing classifier
  (minimum 50, 15-point difference) chooses the result.
- Confidence measures evidence coverage, not accuracy: 15, plus 50 for a Deck
  rating, 15 for store categories, and 20 for two hours of play.
- The panel names the source: Demo, or Steam with the check age. Personal
  records gain no field until the user chooses a preference.

## Prototype review

Check each collection in Grid, Cards, Table, and Board. Change a preference,
close details, and confirm the count and membership change. Return to Automatic.
Check that Both and unknown remain distinct and a blocker warning survives a
personal choice. Inspect scroll, keyboard use, and the minimum window size.

## Later acceptance

- Personal choices survive refresh, restart, Steam sync, and reanalysis.
- Missing, stale, and conflicting evidence is explained. Failed fetches keep
  valid cached evidence. A blocker never proves the other setup works.
- Old records/settings load; unknown fields survive writes. No extra revision
  is written when provider values do not change.
- Counts agree across all presentations; hidden and wishlist games stay excluded.
- Deterministic tests pass before real-library use. Native macOS and Linux
  checks are reported separately. A build is not visual verification.

## Current evidence

October 5: macOS development build, formatting, scoped Clippy with warnings
denied, and diff checks passed. Native review covered all four result buckets,
Grid/Table/Board navigation, Both-to-PC preference changes and Automatic reset,
scrolling and expanded reasons, and a blocker warning after a Deck preference.
Keyboard Space and Escape worked for card details and return to the library.

The final toolbar correction and 960 × 600 layout still need a fresh visual
check: native automation resolved the rebuilt demo to a different running app
instance. Cards view, full keyboard focus coverage, light appearance, and Linux
runtime remain unverified. Automated tests have not been added or run; step 2
follows prototype review. Production scoring, provider enrichment, and durable
preference writes are not implemented.

Review follow-up: collapsed the details by default and removed the repeated
“Best on” label. Targeted formatting and diff checks passed. The follow-up build
is blocked by current editor/card compilation errors (`InspectorEditor` lacks
`Render`; a `CardPose` initializer lacks `hinge`). This panel revision has not
had a fresh native visual check. A subsequent retry of
`python3 desktop/scripts/dev.py --best-on-demo --build-only` passed and produced
`desktop/target/BestOnDemo.app`; the compilation blockers are resolved.

## Planned: rules and enrichment

Accepted direction on 2026-10-06; designs are in Elyx and not built yet.

- Fit is the only metric. Confidence leaves the interface. (i) shows a
  tooltip on hover and a modal with each fit step on click.
- Settings → Best on holds your rules: My PC (Standard or High-end), Prefer
  PC tags, Prefer Steam Deck tags, and equipment that makes a game PC only.
  Rules start empty with one-click suggestions and live in app settings.
- Enrichment: ProtonDB Steam Deck reports, off by default. Turning it on
  downloads the monthly ODbL export (about 70 MB) from
  `github.com/bdefore/protondb-data`, keeps only small per-game results in a
  local cache, and checks monthly. Turning it off deletes the cache. Fit and
  Settings show the ODbL credit. The undocumented ProtonDB API and SteamDB
  (no API; scraping not allowed) are not used.
