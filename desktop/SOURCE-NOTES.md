# Source notes

## Eagle Companion

Source: `/Users/vova/Developer/Github/eagle-linux/`.
Base commit: `d1fc38f9bb8b509c2831123fca1746ee7afdb231`.
Port date: 2026-09-11. The source manifest declares MIT.

The port uses the working tree. It includes local changes; it is not a clean
copy of the base commit. The source repository remains unchanged.

| Destination | Reuse |
| --- | --- |
| `src/ui/thumb_cache.rs` | Eagle cache and eviction logic; removed unused clear/stat accessors |
| `src/theme.rs`, `src/themes/` | Eagle appearance handling, eight theme files, and tests; theme choice is passed directly instead of loaded from Eagle settings |
| `src/ui/grid.rs` | Eagle row virtualization, viewport measurement, image loading, borders, and selection pattern; removed masonry/video and added game captions and keyboard focus |
| `src/ui/sidebar.rs` | Eagle row geometry and hover/selection colors; status rows replace folders |
| `src/ui/detail.rs` | Eagle inspector layout and metadata fields; game data replaces asset properties |
| `src/ui/app.rs`, `src/main.rs` | Eagle layout, entity subscriptions, toolbar, window, and appearance setup; removed library scanning and unrelated features |
| `Cargo.lock` | Seeded from Eagle, then pruned by Cargo for this package |

The source grid and inspector had local video-related edits at port time;
those branches are not used here. Cache and theme files had no local edits.
No video, password lock, Eagle parser, or watcher code is included in this milestone.

The storage file layer is new: Eagle's source has no matching revision writer.
It uses the existing locked `tempfile` version instead of custom temporary-file
naming and rename code. This adds a direct dependency, not a new locked package.

Folder loading also adapts Eagle's native path prompt, background rescan flow,
watcher filtering, cache clearing, and device-local settings pattern. The watcher
uses a bounded queue and a periodic fallback. The SQLite last-valid index and
game discovery are new. `rusqlite` adds SQLite; `notify` reuses Eagle's locked version.

## Demo artwork

Covers came from Steam's public asset CDN on 2026-09-11:
`https://cdn.cloudflare.steamstatic.com/steam/apps/<app-id>/library_600x900.jpg`.
Each filename under `fixtures/covers/` gives the App ID. The fixtures list
the corresponding titles. Artwork remains the property of its respective
owners; the project's code license does not grant rights to that artwork.

Descriptions and personal values are illustrative fixtures, not imported
Steam data or statements about the user's library.

Inspector editing keeps Eagle's detail panel and reuses its GPUI Input and
DropdownMenu patterns. `ui/editor.rs` owns one draft and calls the existing
revision services. No editor framework or dependency was added.

## Viewer filmstrip follow-up — 2026-09-22

Adapted `viewer.rs::render_strip` and `sync_strip` from Eagle's current working
copy at base `d1fc38f` into `src/ui/filmstrip.rs`. Retained its 80 px cells,
6 px spacing, horizontal virtual list, shared image cache, active frame and
instant hover feedback. Game records replace asset thumbnails; selection emits
an event so the focused game's existing pending-save guard remains authoritative.
Eagle itself was not changed.

## Baseline palettes

The native appearance catalog derives from obsidian-baseline revision
`8c56e831e1abb1d3841c4ffdecbe06b5182fbc68`. It includes all 25 named schemes. See `licenses/baseline/LICENSE.txt` and
`licenses/baseline/SOURCES.md` for upstream licensing and palette author credits.
The development-only importer resolves inherited colors into bundled data; no
upstream CSS or browser runtime is loaded by the app. See
[chrome and themes](../plan/chrome-themes.md) for regeneration and mapping rules.
