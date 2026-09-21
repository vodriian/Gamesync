# Collections, Settings, and Steam

## Delivered

Keep Eagle's native shell and portrait grid. Use Glaze's grouped settings and
flat collections. Do not import its runtime or browser storage.

- Collections: create, rename, remove, counts, search within a collection, and
  inline inspector membership with automatic saves.
- Conflicting library definitions: explicit whole-version choice. Missing
  collection IDs from the chosen version remain archived. Other status keys stay
  available. Every branch stays in history.
- Settings: one native window, sidebar entry, and Command/Control-comma.
  General, Look and feel, and AI sections; all themes; Steam account and key;
  sync, cancel, last success, and device disconnect.
- Steam: validate account and key before saving, then import owned games and
  playtime. Enrich descriptions, genres, release year, reviews, and portrait covers.
  Failed stages retry on the next sync. Completed stages are reused.
- Inspector: Refresh cover fetches public Steam artwork again, even for a stage
  marked complete. Resolve current asset paths before legacy portraits and header
  fallback. Reject near-uniform placeholder images; keep old media on failure.

Credentials use Test key followed by Save key. Both fields are masked. Repeating
a save of the same key does not rewrite secure storage. The Library group was removed from Settings. Open folders from the main toolbar.
Normal startup does not load fixtures; demo data requires an explicit QA flag.

## Keys

Enter keys only in the masked native field. The OS credential entry is scoped to
library UUID. JSON preferences contain no secret. HTTP errors exclude URLs and
bodies; credential-bearing HTTP debug logging is disabled.

If secure storage is locked or unavailable, unlock it before saving. A failed
connection test does not replace a working credential. Remove key removes the
local key, not the portable account association or any games.

macOS uses Keychain. Linux uses a running Secret Service and an unlocked keyring.
The Linux build needs D-Bus development libraries in addition to GPUI dependencies.
There is no plaintext fallback and no automatic cloud credential sync.

## Validation

See the [implementation log](logs/2026-09-11.md). The deterministic suite uses
fixed Steam responses and dummy credentials. Optional integration tests separately
check native secure storage and public Steam metadata without an account key.

The user has run a real owned-library sync from Settings. Linux runtime,
live Dropbox concurrency, and a large unique-cover library remain unchecked.
The current GPUI stack does not expose custom controls through the native
accessibility tree; keyboard operation does not establish screen-reader support.

## Next

1. Keep demo records separate from personal libraries; improve main-window sync progress.
2. Verify Linux UI and Secret Service before treating the app as cross-platform ready.
3. Add editable mood, energy, and session length; then the offline feeling/time picker.
4. Add AI suggestions after manual tuning works. Backup/restore and ProtonDB stay deferred.

## AI settings preview

Ollama and OpenAI, Claude, Grok, and Gemini have interactive mock forms. Model
entry comes before the API key. Test and add/remove states are simulated and
kept only while Settings is open. No network requests or credential writes occur.
Actual provider integration and model discovery remain deferred.
