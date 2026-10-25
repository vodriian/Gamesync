# Rust review: Steam sync commit 93c6ead

Follow-up: the error-chain fix and first-stage lint baseline are now implemented.
See the [delivery log](../logs/2026-10-25-rust-baseline.md). The review below
describes the original commit; later lint adoption uses explicit group priorities.

Reviewed `93c6ead6069480ade66f3c298103ce6bf85457ee` against its parent.
This is desktop application code, including an internal library used by the
binary and tests. It is not a published SDK. The matching cache, guarded writes,
and retry logic have good behavioral tests. No blocking correctness defect was
found. The main follow-up is to preserve fatal error causes when adding log
feedback. Formatting, tests, and the proposed small lint baseline passed on
Windows. The broader diagnostic Clippy run completed with warnings.

## Scope and installed skills

- Installed [rust-best-practices](../../.agents/skills/rust-best-practices/SKILL.md)
  and [rust-review](../../.agents/skills/rust-review/SKILL.md), with their reference
  and evaluation files, in the project's `.agents/skills/` directory.
- Source: [BeautifulRust revision 2b2d53a](https://github.com/morganlinton/BeautifulRust/tree/2b2d53a66f6f36ab4b8d6a93070183d91e77fad4).
  The upstream MIT license and pinned source record are stored beside the skills.
- Both skills were read and applied for this review. `AGENTS.md` links them for
  future Rust work. Existing project guardrails take precedence.
- Reviewed the changed Rust code and its storage/UI callers. No application
  source or persistent lint configuration was changed.
- Cargo edition: 2021. No `rust-version` is declared. Checks used rustc 1.99.0
  and Clippy 0.1.99 on Windows x64; this does not establish an older MSRV.

## Must fix

None found in the reviewed change. This is not a claim that all runtime paths
or platforms have been verified.

## Should fix

### 1. Preserve the cause when attaching log feedback

Location: `desktop/src/steam/mod.rs:184`.

**Rule:** Effective Rust, Item 4, preserve useful error causes; BeautifulRust
best-practices section 5, application error handling with `anyhow::Context`.
See the installed [Effective Rust digest](../../.agents/skills/rust-best-practices/references/effective-rust.md).

**Problem:** `anyhow!("{error}\n{feedback}")` makes a new message-only error.
For an error with context, ordinary Display keeps only the outer message;
the original cause and type are lost. The log records the full chain when it
works, but callers cannot recover it, and a failed log leaves no full diagnostic.
The existing fatal-error test uses a single message, so it does not catch this.

**Fix:** Keep the original error and render its chain at the UI boundary:

```rust
// finish_sync
Err(error) => Err(error.context(feedback)),

// SteamJob::start error branch, desktop/src/ui/steam_job.rs:140
this.message = format!("{error:#}");
```

Add a regression case with a contextual `std::io::Error` and unavailable logging.
Check that the returned error retains its I/O cause and the displayed chain
contains the original failure. This fix is proposed, not applied or tested.

## Consider

### 2. Give the new lint exceptions a reason

Locations: `desktop/src/steam/mod.rs:237` and `desktop/src/steam/mod.rs:742`.

**Rule:** `clippy::allow_attributes_without_reason`; Microsoft
`M-LINT-OVERRIDE-EXPECT`, summarized in the installed
[Microsoft digest](../../.agents/skills/rust-best-practices/references/microsoft-rust-guidelines.md).

**Problem:** The commit adds two `too_many_arguments` suppressions without an
explanation. The explicit worker/test dependencies are reasonable, but future
changes cannot tell why the exception exists or when it stops being needed.

**Fix:** For these two functions, use a narrow expectation:

```rust
#[expect(
    clippy::too_many_arguments,
    reason = "Keep the sync worker's transport, cancellation, progress, and log dependencies explicit"
)]
```

The existing wishlist suppression at line 606 has the same cleanup opportunity.
There is no need to add an abstraction solely to reduce the parameter count.

### 3. Avoid allocating a temporary feedback string

Location: `desktop/src/steam/mod.rs:180`.

**Rule:** `clippy::format_push_string`; BeautifulRust best-practices section 6.

**Problem:** Appending formatted feedback allocates a temporary string. This runs
once at completion, so the performance impact is small.

**Fix:** The content needs only two direct appends:

```rust
state.message.push('\n');
state.message.push_str(&feedback);
```

## What's already good

- One identity/history index is shared by owned and wishlist imports. New IDs
  enter the index immediately; tests check reuse and one full scan per run.
- Record updates still re-read under the record lock. Tests cover personal edits
  after indexing, late conflict copies, and no revision for unchanged Steam data.
- Metadata completion remains separate from user data. Tests cover unchanged
  second runs, new games, wishlist changes, and retrying only missing steps.
- Diagnostics exclude request queries and bodies, redact known credentials,
  bound disk use, and retain the final outcome. Local HTTP tests cover redirects,
  throttling, and malformed JSON. There is no new unsafe code in the commit.

## Recommended Clippy policy

Use a small baseline. The project already has `[lints.rust]` for
`unexpected_cfgs`; preserve it. Do not paste the entire upstream lint file.
Clippy's [official guidance](https://doc.rust-lang.org/clippy/lints.html)
supports choosing individual pedantic and restriction lints.

### Enable first

Append `unsafe_op_in_unsafe_fn = "warn"` to the existing `[lints.rust]` table.
Add this Clippy table:

```toml
[lints.clippy]
all = { level = "warn", priority = -2 }
correctness = { level = "deny", priority = -1 }
undocumented_unsafe_blocks = "warn"
dbg_macro = "warn"
todo = "warn"
```

`all` makes the existing default checks explicit, including lock-across-await
checks. Keep correctness errors stronger than the group setting. The other
rules protect platform safety comments and catch debugging or unfinished code.
The equivalent warning checks passed with `-D warnings` on Windows. Existing
dependency warnings remain outside the first-party gate.

### Enable after a focused cleanup

| Rule | Reason and measured work |
| --- | --- |
| `unwrap_used = "warn"` | Enforce the input/error guardrail. With upstream test exemptions, 47 sites remain: 7 in application UI code and 40 in integration-test helpers. Inspect invariants; do not replace every unwrap with expect. |
| `expect_used = "warn"` | Also check expect on files and network data. Ten sites remain, including build-time and app invariants. Allow only justified local cases. This is stricter than upstream's general allowance. |
| `allow_attributes_without_reason = "warn"` | Four current sites need reasons, including the two introduced here. Prefer `expect` when a lint should still fire. |

For the panic checks, add `desktop/clippy.toml` with:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
```

These exemptions cover test functions and `#[cfg(test)]` code. Unannotated
integration-test helper functions still need handling; the measured 40 helper
sites show that the config alone is insufficient. Keep any helper allowance
narrow and explained. See the [Clippy configuration reference](https://doc.rust-lang.org/clippy/lint_configuration.html#allow-unwrap-in-tests).

Because `desktop/scripts/windows.ps1 -Command clippy` uses `-D warnings`, new
warn rules become build gates immediately. Resolve or justify their existing
sites in the same change that enables them.

### Defer or keep optional

- Full `pedantic`: the diagnostic run with additional restriction rules and
  upstream configuration produced 942 distinct first-party warnings across all
  targets. This is not a count of bugs, nor solely of pedantic warnings.
  Examples include 100 missing error-doc sections and 97 must-use suggestions.
- `as_conversions`: 58 sites include routine UI conversions. Review the narrower
  `cast_possible_truncation` (16), `cast_sign_loss` (5), and `cast_possible_wrap`
  (9) rules in a separate boundary-focused cleanup before enabling them.
- `panic_in_result_fn`: all 15 current reports are assertions in tests that return
  `Result`. It adds little signal here beyond a manual panic review and the
  targeted unwrap/expect rules.
- `fn_params_excessive_bools` with upstream's zero threshold: 70 warnings include
  UI state switches. `struct_excessive_bools` also flags independent completion
  flags; a single enum would not represent those independent states.
- `clone_on_ref_ptr`: 29 style reports, including intentional shared diagnostics
  and cancellation handles. Useful for readability, but not an ownership defect.
- Blanket API-documentation/Debug rules and `print_stdout`/`print_stderr`: this is
  an app with internal APIs and output-producing examples. Apply them selectively.
- `format_push_string` can remain an advisory review check; finding 3 is cheap to
  fix but does not justify a separate cleanup project.

The lint configuration would catch findings 2 and 3. It would not catch the
error-chain loss in finding 1; that needs code review and a regression test.

## Checks and limits

- Windows `cargo fmt --check`: passed.
- Locked default test suite: 224 passed, 0 failed, 12 optional tests ignored.
- Locked Clippy, all targets/all features, with pedantic and selected upstream
  restriction rules: completed; repeated target diagnostics were deduplicated by
  lint, primary source position, and message. Counts above use upstream
  `clippy.toml`, including its bool threshold and test exemptions.
- Locked Clippy, all targets/all features, with the proposed first-stage warning
  checks and `-D warnings`: passed for GameSync. Vendored GPUI float-literal
  warnings and the proc-macro-error2 future-compatibility notice remain.
- Raw command outputs are local, ignored files under
  `desktop/target/beautiful-rust-review/`.
- No live Steam account sync, GUI inspection, release rebuild, macOS build, or
  Linux build was performed for this review. Lint recommendations do not verify
  platform code excluded from the Windows build.
- No production fix or lint configuration has been applied. Follow-up work:
  preserve error causes, then adopt the small baseline and focused panic checks.
