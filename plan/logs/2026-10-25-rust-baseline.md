# Steam error handling and Rust baseline

- Keep fatal Steam sync errors as `anyhow::Error` with log feedback as context.
  The UI renders the complete cause chain, including when the log is unavailable.
- Added a regression test with a contextual permission error. It checks both
  available and unavailable logging, the retained I/O error type, and full text.
  It failed against the old wrapper and passed after the fix.
- Enabled the baseline in `desktop/Cargo.toml`: Clippy `all`, `correctness`,
  `undocumented_unsafe_blocks`, `dbg_macro`, and `todo`, plus Rust's
  `unsafe_op_in_unsafe_fn`. Correctness is denied; the other rules warn.
  Group priorities are -2 for `all`, -1 for `correctness`, and 0 for individual
  rules. Strict Clippy verified this precedence. Broader rules remain deferred.
- Included the project-local BeautifulRust skills, pinned source and MIT license,
  project invocation instructions, and the [commit review](../reviews/2026-10-25-steam-sync.md).

## Windows checks

- Formatting and strict all-target Clippy passed.
- Default locked test suite: 225 passed, 0 failed, 12 optional tests ignored.
- Locked release build passed. Existing vendored GPUI float-literal warnings and
  the proc-macro-error2 future-compatibility notice remain.
- No live account sync was run. macOS and Linux checks remain unverified.
- The rebuilt executable opened an isolated empty preview. Native inspection
  confirmed the Home screen, empty-state text, and sidebar rendered. The first
  hidden launches were not useful for inspection; a visible launch was checked.
  This is a startup smoke check, not a live Steam error-flow check.

Executable: `desktop/target/windows-error-handling/GameSync.exe`.
Size: 37,750,784 bytes.
SHA-256: `48f741d92163427de1f8e8e732e4e6b4c7996c8309743e768df57eb56f9e979a`.

Next: use this build for Steam sync. The regression test covers the original
error-cause failure; future errors should retain the underlying reason in the UI.
