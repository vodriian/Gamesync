# Steam review sync repair

Branch: `windows-first-run`, based on `056abe7`.
Scope: repair the review stage and build a Windows executable.

## Cause and change

- Local records had 18 incomplete review stages. Other metadata stages were complete.
- The old `/appreviews` endpoint returned HTTP 302 to the Steam homepage.
- Moved review requests to Valve's documented
  [GetAppReviews service](https://partner.steamgames.com/doc/webapi/IUserReviewsService).
  It uses `input_json`, numeric filters, and a `response` envelope without `success`.
- Kept all languages and all purchase types. Request one review to limit the
  response; only its summary is saved. Zero requests return the default 20 reviews.
- Kept existing count validation, guarded writes, and retry behavior.
  No personal library data was changed during diagnosis or validation.
- Added response-shape regression tests and an optional public API test.

## Checks

- Windows locked release build passed.
- Default suite: 213 passed, 0 failed, 12 optional tests ignored.
- Compiled Rust client passed the optional live reviews test for three apps.
- Separate public API probes returned valid counts for all 18 affected games.
- Formatting, strict all-target Clippy, and Git whitespace checks passed.
- Existing GPUI float-literal and `proc-macro-error2` compatibility warnings remain.
- No GUI code changed. The repaired app has not run a full account sync yet.

## Artifact

- `desktop/target/windows-review-fix/GameSync.exe`
- 37,715,968 bytes; unsigned Windows release build.
- SHA-256: `3f7e38d001ef007b5cce7d61f4abf2c14bc7c2f1263a18cd94212d112997eae7`.
- The previous app was open, so the repaired build uses a separate output folder.

Next: close the previous app, open this build, and run Steam sync. Existing
completed metadata stages are skipped; the failed review stages are retried.

## Persistent Steam sync diagnostics

- Added a device-local JSON Lines log for each account sync after key retrieval.
  The final status shows its path, including when sync fails or is cancelled.
- Logs record Unix millisecond timestamps, version/platform, stages and App IDs,
  HTTP status, content type, Retry-After, elapsed time, issues, and final totals.
  Disk logs include failures beyond the UI's 20-issue limit.
- Request queries and response bodies are excluded. Known API keys and account
  IDs are redacted from all entries. Errors may contain titles and local paths.
- Retention keeps 10 logs, excluding active runs from deletion. Each run has a
  2 MiB cap, with space reserved for a truncation marker and final result.
- Logging failures do not block sync. Windows releases the log lock on completion
  so the file can be read when the status shows its path.
- Windows folder: `%LOCALAPPDATA%\GameSync\GameSync\data\logs\steam-sync\`.
  See the [desktop instructions](../../desktop/README.md#steam-sync-logs).

Checks: Windows locked release build, formatting, strict all-target Clippy,
and whitespace checks passed. The final default suite passed 220 tests;
12 optional tests stayed ignored. New checks cover secret redaction, HTTP
302/429, invalid JSON, retention, size limits, fatal and cancelled runs,
failure/retry logs, and unavailable logging. Existing dependency warnings remain;
Clippy also reported a non-fatal incremental-cache hard-link fallback.
No live account sync or GUI review was run for this logging build.

Updated executable: `desktop/target/windows-sync-logs/GameSync.exe`.
It includes the review API repair. Size: 37,710,848 bytes.
SHA-256: `b09e1d64198efdbd97c899567adb28d50b882c748e983fe85ab2e1a16add0df3`.

Next: open this build and run Steam sync; inspect its completed log if a step fails.

## One-pass Steam matching and clear progress

- Replaced the full identity/history scan inside every import with one index
  per account sync. Owned games and the wishlist share it; new IDs enter the
  index immediately. A local import-session lock prevents parallel Steam
  imports from invalidating the index.
- Kept current record locks and revision checks. Filename discovery no longer
  reads the contents of UUID-named files just to obtain their known identity.
  Per-write filename checks and target-history reads remain, including detection
  of new, nonstandard conflict-copy names. This is one full matching scan,
  not a promise of only one filesystem directory listing.
- Count new, updated, and unchanged records across owned and wishlist data.
  Updates are measured under the game lock, so concurrent personal edits do
  not count as Steam changes. Wishlist priority changes and removals count too.
- Plan only missing metadata steps, including tags. Progress reports checked
  steps against that plan, and the final message gives the remaining count.
  Cached steps do not appear as downloads. Logs include the new counters.
- Tests prove one full scan per sync, unchanged second runs, activity changes,
  wishlist changes, failed-step retries, latest personal edits, late conflicts,
  import locking, reused new IDs, and rejection of unreadable files.
- Final Windows suite: 224 passed, 0 failed, 12 optional tests ignored.
  Formatting, strict all-target Clippy, Git whitespace, and locked release build
  passed. Existing dependency and incremental-cache warnings remain.
- No live account sync or GUI review was performed for this build.

Latest executable: `desktop/target/windows-incremental-sync/GameSync.exe`.
It includes the reviews repair and persistent logs. Size: 37,751,296 bytes.
PE headers confirm x64 and Windows GUI subsystem.
SHA-256: `c32d06cd81a359b1bd1ddb965749f73f51e211ae2ba4b6e12b2dc78fa61bd79f`.

Next: close the previous build, open this executable, and check the counts on
two Steam syncs. A completed, unchanged library should need no detail downloads.
