# macOS build after Play now merge

- Confirmed PR #7 merged into `windows-test-build` at `4d7c99888166fb96e20f32d0df27d925b01358e4`.
- Fast-forwarded the local `windows-test-build` branch to that commit.
- Ran `python3 desktop/scripts/package_macos.py --output /Volumes/Scylla/Developer/Gamesync/desktop/target/macos`.
- The locked, optimized release build passed in 32.24 seconds.
- Replaced `desktop/target/macos/GameSync.app` and `GameSync-0.1.0-macos-arm64.zip`.
- Apple Development signing and strict codesign verification passed with normal macOS certificate access. The sandbox could not read certificate trust.
- ZIP integrity passed. The executable is Mach-O arm64, version 0.1.0, minimum macOS 11.0.
- All 15 file-backed Mach-O sections match the compiled release executable. Code signing changes signature metadata, so whole-file hashes are not expected to match.
- ZIP SHA-256: `4832faa801e80beb10fc77ce1724ed8fc2151cbdb48813931c4e7d658b3e58ae`.
- Local signed build; not notarized. This pass did not launch the app or repeat the unchanged source's test suite. No new commit or push.

## Best on startup fix

- Removed the sidebar startup override that forced Best on open. It now starts collapsed; manual expansion still works for the current session.
- Kept the existing settings compatibility rule: Best on expansion is not persisted. Other smart collection settings are unchanged.
- `cargo fmt --manifest-path desktop/Cargo.toml --check` passed.
- `cargo clippy --manifest-path desktop/Cargo.toml --locked --all-targets -- -D warnings` passed.
- Built the native demo and verified Best on is collapsed on launch and expands when clicked. Used isolated demo data, not the personal library.
- The user requested a direct commit and push on `windows-test-build`, followed by a new primary macOS package. No new branch or PR.
