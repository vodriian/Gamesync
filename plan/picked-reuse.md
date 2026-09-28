# Reuse Glaze Picked

Source reviewed: `/Users/vova/Glaze/Picked/sources/`. Treat it as a read-only
reference. Do not import Glaze's generated runtime into the Rust app.

## Decisions

- Keep the Eagle shell, portrait grid, and inspector.
- Use feeling and available time as picker inputs. Reuse Picked's feeling mapping.
- Adapt grouped detail sections: store facts, Tune for the picker, notes, flags,
  and collections. Keep five direct rating stars.
- Port pure rules, response mappings, and prompts into small Rust modules.
  Reuse current revision storage and GPUI components.

## Corrections to carry into each port

- Picker: deterministic ranking with explicit time and exclusions. Remove random
  score jitter. Use our manifest's status eligibility and explain unknown data.
- Steam: track each enrichment stage separately. A failed request must remain
  retryable. Keep valid cached data and personal overrides.
- AI: reuse compact prompts and bounded batches. Validate game IDs, duplicates,
  missing results, and field values. Preserve manual fields and separate suggestions.
- Storage: retain our per-game revisions. Put collections and cover files in the
  portable library rather than browser localStorage.
- Platform: replace Glaze IPC, windows, AI access, and secret storage with native
  services when each feature is built. Glaze AI is not a portable provider.

## Order

Saved-game conflict review, collections, core Settings, and Steam sync are
implemented. The user requested collections and Steam before the picker.
Next: live account sync, Linux verification, tuning fields, then the offline
picker and AI adapters. See [current implementation](collections-settings-steam.md).
Session timers, affinity feedback, and reroll cooldowns remain deferred.
