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

## Demo artwork

Covers came from Steam's public asset CDN on 2026-09-11:
`https://cdn.cloudflare.steamstatic.com/steam/apps/<app-id>/library_600x900.jpg`.
Each filename under `fixtures/covers/` gives the App ID. The fixtures list
the corresponding titles. Artwork remains the property of its respective
owners; the project's code license does not grant rights to that artwork.

Descriptions and personal values are illustrative fixtures, not imported
Steam data or statements about the user's library.
