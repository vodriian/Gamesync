# Project guardrails

Build the smallest complete feature that meets the current need.

## Code

- Use clear names and small modules with one purpose.
- Keep data flow explicit. Avoid hidden mutation and global mutable state.
- Keep domain logic separate from GPUI, disk access, and network calls.
- Use concrete types first. Add a trait at a real boundary, not for every struct.
- Extract shared code when it removes real duplication. Do not build generic frameworks for possible future features.
- Use typed records and enums. Limit flexible JSON values to defined extension fields and external payloads.
- Handle expected errors with useful messages. Do not use `unwrap` or `expect` on user files or network data.
- Run disk, decoding, and network work off the UI thread. Bound work queues and memory use.
- Keep successful writes recoverable. Report failures; never silently discard user data.
- Reuse existing components and dependencies when they fit. Add dependencies only for a named requirement.
- Keep changes focused. Do not mix feature work with unrelated cleanup.
- Remove dead code introduced by the change. Do not leave alternate implementations or commented-out code.

## Comments

- Document public interfaces, units, invariants, and non-obvious decisions.
- Explain why code exists, especially at persistence and platform boundaries.
- Use brief module comments to explain ownership and data flow when needed.
- Do not repeat function names or describe each line.
- Keep comments accurate when behavior changes. Put long rationale in the plan.

## Validation

- Test observable behavior, not a copy of the implementation.
- Prioritize writes, recovery, imports, field overrides, AI validation, and recommendation constraints.
- Use small fixtures with no private data. Keep network tests separate from deterministic tests.
- For Rust changes, run formatting checks, targeted tests, and Clippy as appropriate to the changed scope.
- Broaden checks when changes cross module boundaries. Do not repeat passing checks without a reason.
- For GUI changes, inspect the running native app. A successful build is not a visual check.
- Cover keyboard input, focus, loading, empty results, errors, and reduced motion where relevant.
- For documentation-only changes, check links, consistency, and the diff. Do not build the app.
- Record exactly what was checked, on which platform, and what remains unverified.

## Writing

- Follow ASD-STE100 principles where practical; this is not a claim of formal compliance.
- Use short sentences, active voice, and one instruction per sentence.
- Use one term for each concept. Keep necessary Rust, GPUI, Steam, and API terms.
- Prefer specific labels: `Library`, `Choose`, `Analysis`, `Save`, and `Retry`.
- Explain the action and its result. Avoid slogans, jargon, and long introductions.

## Efficient agent work

- Read the index, then the documents and source files needed for the task.
- Use targeted searches and bounded output. Do not scan dependencies or logs without a reason.
- Reuse facts already checked in the task. Recheck facts that can change.
- Keep one source for each decision. Do not copy plans into logs or comments.
- Use a short task scope and acceptance check before changing code.
- Do not start parallel agents unless the user requests them.
- Optimize token use by reducing noise, not by skipping needed work or checks.

## Scope and records

- Work on a feature branch. Keep source projects and unrelated local changes intact.
- Keep project decisions and logs in `plan/`; do not use external memory as the project specification.
- Log changes, evidence, limits, and the next task. Do not store raw conversations or tool output.
- Keep keys, tokens, account IDs, and personal library content out of committed logs.
- Mark deferred work as deferred. Do not add a subsystem because it might be useful later.
