# Play now prototype

An early HTML exploration of GameSync's recommendation flow. The native
app and reference web app do not import these files. Later native changes
(sliding controls, modal details, and timed play sessions) are documented in
`plan/play-now.md` and `Design/design/screens/play-now/PlayNow.elyx`; this
prototype is retained as the initial design reference.

From the repository root:

```sh
python3 -m http.server 4176 --bind 127.0.0.1
```

Open <http://127.0.0.1:4176/prototypes/play-now/>. You can also open
`index.html` directly. There is no build step or new dependency.

- Set time (including ∞ for no limit), energy, an activity, and Brain dead.
  Deal up to three cards. Activity chips describe what you want to do, and
  the selected activity must match unless you explicitly choose Anything.
- Use More options for setup, installation, favorites, and playing scope.
- Inspect a card, save it, dismiss it with Undo, or choose it.
- Reshuffle once per session. Editing preferences starts a new hand.
- Use Review states in the footer for loading, no matches, missing profiles,
  missing artwork, and a simulated launch error. Toggle light/dark there.

Twelve sample records include one hidden game, excluded from the visible
library count and all hands. They use artwork in `Design/resources/images/` and
unchanged SVGs from `Design/resources/icons-new/`. Fit values, history,
installation, and device assessments are invented sample data. Nothing is
read from a personal library, saved to disk, sent to AI, or launched in Steam.
Reload clears the session. Sidebar items outside Play now are visual context.
Offline profile rules and optional AI enrichment are specified in the
requirements; this HTML continues to use fixed, invented profiles.

See [requirements](../../plan/play-now.md) and the
[verification log](../../plan/logs/2026-10-06-play-now.md).
