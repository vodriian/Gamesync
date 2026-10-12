# Merged-main macOS build

- Source: `main` at `89033793433b9f2af7c030c8d09d23dac5a7168a`, after PR #12.
- Host: macOS 27.0.1, arm64.
- The source tree matches the tested `windows-first-run` integration exactly
  (`e78ed59f82908a981a9046831550262507320fa3`). Reused its 211 passing tests,
  11 optional ignored tests, and strict all-target Clippy results from today.
- Fresh formatting and locked binary compile passed.
- Ran `python3 desktop/scripts/package_macos.py --output
  /Volumes/Scylla/Developer/Gamesync/desktop/target/macos` successfully. It compiled
  the optimized release and replaced the app and ZIP in the requested directory.
- Artifacts:
  - `desktop/target/macos/GameSync.app`
  - `desktop/target/macos/GameSync-0.1.0-macos-arm64.zip`
- The app uses the existing Apple Development certificate. Strict codesign
  verification and ZIP integrity passed; the executable is Mach-O arm64.
- Executable SHA-256:
  `3cd63c795cb8d21cbdff2f5adc4b0ee851918f763dc15396a454373953426143`.
- Launched without demo flags using an isolated empty profile. The main window,
  empty Home state, and footer sync/settings icons rendered. The test instance
  was closed and exited with code 0. Existing SVG `currentColor` warnings remain.
- This is a signed local test build, not notarized. Live Steam sync, personal
  library workflows, and other platforms were not rechecked for this package.

## Context-menu shadow quick fix

- Increased macOS Metal shadow integration from 4 to 32 vertical Gaussian
  samples. The sparse sampling produced visible bands around rounded menus.
  Menu layout, shadow colors, and the other platform shaders stay unchanged.
- Native sample-library inspection showed a smooth context-menu shadow. The
  development build and shader compilation passed; formatting and diff checks
  passed. No Rust behavior changed, so the full test suite was not repeated.
- Rebuilt the app and ZIP in `desktop/target/macos`. Strict signature and ZIP
  checks passed. The local package remains signed and not notarized.
- Updated executable SHA-256:
  `cafcb4200a62cbb9f9523469748f66b2f6be9d97261ab31bd4f251abfb4d5b43`.
- No branch was created for this fix. Concurrent workspace work switched the
  checkout to `fixes-optimization` and committed startup instrumentation while
  the fix was in progress. That state was preserved.
- The user requested logs, commit, and push. The shadow shader, vendor patch
  note, and this build log form one scoped commit on `fixes-optimization`.
  Packaged binaries remain ignored by Git.
