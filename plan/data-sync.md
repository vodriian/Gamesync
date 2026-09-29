# Data sync across devices

Status: approved direction, September 29. Steps 1 to 5 implemented: the
engine in `desktop/src/sync/`, the app connection in `desktop/src/sync_runtime.rs`,
the Settings section, and encrypted keys. Next: step 6, the live check. Branch: `data-sync-mode`.

## Goal

One person uses GameSync on several computers: macOS and Omarchy (Linux) first,
Windows after that. Personal data, settings, and API keys must be the same on
all of them. The transport is a folder that the user already syncs: Dropbox,
Google Drive, OneDrive, Syncthing, or a manual copy. GameSync adds no server and
no account.

## User decisions

- **Scope:** every data field that does not come from Steam syncs. This
  includes fields added in the future, such as custom fields and AI results.
- **Merging:** GameSync asks the user when two devices changed the same value.
  It does not select a winner silently.
- **Keys:** the Steam key and future AI provider keys sync in encrypted form,
  as part of settings sync.
- **Library location:** each device keeps its own local library. The folder
  holds only settings and personal content. A full merge occurs only once, when
  a device joins.
- **Windows:** starts after sync works between macOS and Omarchy.

## What syncs

| Data | Syncs | Note |
| --- | --- | --- |
| Personal game data | Yes | Status, board order, rating, favorite, hidden, tags, notes, collections, overrides, and every future or unknown personal field |
| Override covers | Deferred | Content-addressed image files. No feature sets an override cover yet |
| Manual games | Deferred | Identity, title, and personal data. No feature creates manual games yet |
| Archive state of a game | Deferred | The loaded library leaves out archived games |
| Library definitions | Yes | Name, default status, statuses, status order, collections, collection order, and future field definitions and saved views |
| Bound Steam account ID | No | Each device binds its own library. Different accounts on two devices are not supported |
| Future AI results | Yes | They are user data that costs money to create again |
| Shared settings | Yes | See the settings table |
| API keys | Yes, encrypted | Steam key and future AI provider keys |
| Steam data, Steam covers, prices | No | Each device fetches them from Steam |
| SQLite index, caches, window state | No | Device-local and rebuildable |
| Sample (`--demo`) libraries | No | |

The rule for new fields: a field outside the `steam` part of a game record, or
in library definitions, syncs by default. A field that must stay on one device
is listed as device-local in this document.

### Settings

| Shared | Device-local |
| --- | --- |
| Reduce motion | Theme and appearance |
| Show hidden games | Omarchy mode (Linux only) |
| Sort, descending, and grouping | Sync folder, device ID, device name |
| View choice for each section | Last Steam sync times |
| Open smart groups | Detected Steam country |
| Store country | Window size and position |
| | Legacy library path |

User decision, September 29, after the first live check: the theme stays on
each device. Each computer can have a different screen and desktop theme.
Test builds before this decision wrote theme and appearance changes to the
folder. Rounds ignore changes for device-local settings, so these old changes
make no conflict and change no theme.

## Decision: sync folder is a transport, not the library

The earlier plan put the whole library in Dropbox. That fails here:

- Each device runs Steam sync and rewrites game records. Three devices create
  branches on almost every game, and the current rules block edits until the
  user resolves each branch.
- Local game IDs come from each library's UUID. The same Steam game has a
  different ID on each device.
- Steam covers and full revision history put thousands of files in the cloud.
- Google Drive and OneDrive can give placeholder files and late or missing
  file events. A shared file with several writers is not safe there.

New approach: each device keeps its own local library, with no change to its
format. The folder holds small change logs. **Each file has exactly one
writer: the device that created it.** Cloud tools then see no concurrent writes
to one file and make no conflicted copies of GameSync data.

## Folder format

The user selects a folder. GameSync uses a `GameSync Sync/` subfolder in it.

```text
GameSync Sync/
  sync.json                          # format version, sync ID, key check; written once
  devices/<device-id>.json           # name, platform, app version, last seen
  changes/<device-id>/<seq>.json     # immutable change batches; seq has 10 digits
  media/<sha256>.<ext>               # override covers; immutable
  snapshots/<device-id>.json         # later: compacted state of one device
```

- `device-id` is a random UUID in device-local settings. It never goes in the
  library or shared settings, so a copied profile does not copy the identity.
- A batch is one JSON object with format, device, number, and at most 1000
  changes. It is written once to a hidden temporary name, then renamed without
  overwrite, and never changed. A partial download does not parse, so a reader
  skips it and tries again at the next scan. A batch whose device or number
  does not match its path, or that holds another device's change, is rejected.
- A device stores the number of its last batch. A batch in its own folder with
  a higher number, or with the same number and different content, comes from
  a second writer with the same device ID.
- Hidden files and `desktop.ini` are ignored. Other unknown names are reported.
- File writes use the Unix durable-write path. Windows needs its own path in
  step 7.
- File names use only lowercase ASCII letters, digits, `-`, and `.`. They are
  short and safe on Windows, macOS, Linux, and case-insensitive volumes.
- Cloud conflict copies (`(conflicted copy)`, `.sync-conflict-`, ` (1)`) are
  not read and are reported. They occur only if two devices share one device ID.

## Change model

A change sets one value:

```json
{"id":"<uuid>","at":{"ms":1790000000000,"n":0,"device":"<device-id>"},"target":"steam:620","field":"personal.rating","value":8,"base":["<change-id>"]}
```

- **Targets:** `steam:<app-id>` for Steam games, `game:<uuid>` for manual
  games, `status:<key>`, `collection:<uuid>`, `library`, `settings`, and
  `secret:<name>`.
- **Fields are small.** Collection membership is one boolean for each game and
  collection. Board order is the existing fractional `board_rank` of each
  game. Status order and collection order are one list field each; a status or
  collection that the received list does not name stays, after the listed
  ones. Two reorders at the same time are a conflict for review.
- **Unset:** a field with its default value is left out, and `null` clears a
  field to its default. Definitions equal to those of a new library count as
  unset too. So a value set on only one device is taken without a review.
- **Section views:** section keys can hold any text, so the setting field is
  `section_views.<hex of the key>`.
- **Unknown fields** from a newer app version are stored and passed on. A
  device does not drop a value that it cannot display.
- **Clock:** a hybrid logical clock: wall time, a counter, and the device ID
  as a tie-break. When a device reads a change, its clock moves past it.
- **`base`** lists the changes that the edit replaced: the value that the user
  saw on screen.

### When GameSync asks

- **Sequential edit:** the new change's `base` includes the current value.
  GameSync applies it with no question. This is the usual case: an edit on the
  Mac, then later an edit on Omarchy.
- **Concurrent edit:** two changes to one field have the same `base` and
  neither includes the other. This is a conflict. The device keeps its current
  local value and shows the conflict. Nothing is lost.
- **Resolution** is a new change whose `base` lists all conflicting changes.
  Every device reads it and clears the conflict. If two devices resolve the
  same conflict differently at the same time, the result is a new conflict.
- **Late history:** if a change lists a `base` that has not arrived yet, the
  newest candidate is shown and no review opens. The missing change usually
  shows that one candidate replaced the others. If it never arrives, the newest
  candidate stays.
- **Edit during review:** a new local edit of a field in conflict replaces all
  candidates, so it also resolves the conflict.
- **Faulty data:** two different changes with one ID keep the one with the
  lower JSON text on every device, and the app reports it. A cycle of bases
  opens a review.
- **Deletion:** collections and statuses stay archived, as now. Nothing is
  removed from the logs.

### Conflict review

- The sidebar footer shows "N to review"; it opens the Sync section. The
  section shows the review above the device list.
- The review lists each value: game or item, field, this device's value, and
  the other devices' values with device names. At most 50 rows show at once;
  bulk choices apply to all.
- Values show as text: status and collection names, "Sort by name,
  ascending, no groups", view names. The review never shows raw JSON. A value
  that this version cannot describe shows "A value this version cannot show";
  the device name tells the values apart.
- Choices for each value: **Keep this**, **Use other**. Text fields and tags
  also have **Keep both**: text is joined with a separator, tags are combined.
- Bulk choices: **Keep all from this device** and **Use all from <device>**.
- Deferred: a mark on the game card for a field in review.
- Each device keeps its own value until the user chooses. A choice is a new
  change; the next round applies it on every device.

## Joining (one time)

- **Empty folder:** GameSync creates `GameSync Sync/`, asks for a new sync
  passphrase, and writes all current personal data, definitions, shared
  settings, and keys of this device as the first batch.
- **Folder with GameSync data:** GameSync asks for the passphrase and reads
  the folder. Then it compares the shared result with local values:
  - Equal: no action.
  - Set only on one side: GameSync takes that value with no question.
  - Different: the join review opens. It uses the conflict review, with the
    bulk choices first. Examples: "212 games have a different rating or status.
    Use this device / Use the folder / Review each."
- Joining never changes Steam data and never removes local games.
- **Stop syncing:** local data stays. The folder stays. Other devices continue.

## Keys and encryption

- A random **sync key** (an `age` X25519 identity) encrypts each API key.
  The passphrase encrypts only the sync key (age scrypt), because passphrase
  encryption is slow on purpose and rounds run every 10 seconds.
- The protected sync key is the synced value `secret:sync_key`, field
  `wrapped`: the age text and the public recipient. A wrong passphrase fails
  to open it and changes nothing. The recipient shows a device that its saved
  sync key was replaced; it asks for the passphrase again.
- API keys are changes with target `secret:<name>`, field `value`:
  `secret:steam_api_key` now, AI provider keys later. The value is the age
  text and a fingerprint: a SHA-256 hash of the sync key, the name, and the
  API key. Encryption gives new bytes each round, so the engine compares
  fingerprints. Without the sync key the fingerprint tells nothing.
- The user enters the passphrase once on each device. The device saves the
  **sync key**, not the passphrase, in the OS credential store. Received API
  keys go to the same store, never to a file. Stop syncing removes the saved
  sync key.
- A device without the sync key does not read or send API keys; received
  keys wait. Removing a key on an unlocked device removes it on the others.
- Changing the passphrase protects the same sync key again. API keys and
  other devices are not affected.
- A lost passphrase: devices that still have the sync key keep working and
  can set a new passphrase. If no device has it, API keys must be entered
  again. A reset control for that case is deferred.
- Two devices that turn on key sync at the same time make a conflict on
  `secret:sync_key`. The review shows it; after the choice, the other device
  unlocks with the chosen passphrase.
- Minimum passphrase length: 10 characters. `age` uses its default scrypt
  work factor.
- Dependencies: `age` 0.12.1 with `armor`, and `sha2`, which was already in
  the lock through other crates.

Personal data and settings are not encrypted, by user decision. The cloud
provider can read notes, tags, and settings.

## Connection to the local app

The sync engine does not hook each save path. Each round compares three
values for each field: the local value now, the value at the last round
(`known`, saved on the device), and the merged folder value.

- **Local equals known:** no edit here. A new folder value is applied through
  the existing guarded writes. It becomes known only after the write succeeds.
- **Local differs from known:** an edit here, from any save path. It is
  written with the known changes as its `base`, so an unseen remote edit
  becomes a conflict and is not overwritten.
- **First round after joining:** nothing is known. A value set on one side is
  taken; different values are conflicts. Joining uses no separate code.
- **Absent targets:** a game that is not in the loaded library now (archived,
  unreadable, or not added by Steam sync yet) is never read as cleared. Its
  values wait and apply when the game is present.
- **Device state:** device ID, sync ID, last batch number, known values, and
  unwritten changes (outbox) are in one device-local file. An edit is saved
  there before it is written to the folder, so an offline folder or a crash
  loses nothing.
- **Missing folder:** an unmounted or replaced sync folder is an error. The
  engine never creates its folders on the local disk in its place.
- **Rounds:** the library refresh loop runs a round after each library read:
  after a local save (library file watcher), at start, on Sync now, and every
  10 seconds. The sync folder has no watcher; the 10-second scan finds new
  batches. A round that wrote to the library reads it again at once.
- **Shared settings from another device** are applied to the open windows:
  reduce motion, hidden games, sort and grouping, and section views.
- **Offline at start:** if the folder cannot open, each later round tries
  again. Edits made meanwhile are found as local changes when it opens.
- **Speed:** each game write inspects its record files. A first join that
  applies thousands of games can take time. Measure it in step 6.
- An open notes draft is never overwritten. The existing external-edit review
  appears.
- Steam sync writes only Steam data and makes no sync changes.
- Local revision history stays as it is. The logs are the only shared data.

## App open on several devices

This is a normal case, not an error:

- Edits on two devices go to two different files. No lock is necessary.
- A device shows a remote edit within 10 seconds after the cloud tool
  delivers the file.
- The same value edited on two devices before either saw the other: conflict
  review, as above.
- Two devices with one device ID (a copied settings folder): a device finds a
  batch number in its own folder that it did not write. It stops writing,
  makes a new device ID, and reports this in Settings.

## Settings: Sync section

A **Sync** section after General, where the Steam connection is:

- **Off:** a short description, the help text "Select a folder in Dropbox,
  Google Drive, or OneDrive, and keep it available offline", and **Choose
  folder…**. The sample library cannot sync.
- **Folder:** path, **Sync now**, **Show folder**, and **Stop syncing**. Stop
  asks again when changes wait to be written. It removes the device state and
  keeps the library and the folder.
- **Status:** "Up to date", "Waiting for the folder", "N changes need your
  review", or "N changes wait to be written", with the last received changes,
  values waiting for games, and up to three issues.
- **Review changes** (only when there are conflicts).
- **This device:** an editable name; the default is the host name.
- **Devices:** name, platform, and the time of the last written change.
- **API keys:** turn on key sync with a passphrase (entered twice), unlock
  with it on other devices, or change it. "The passphrase is wrong." when it
  does not open the sync key.

## Provider notes

| Provider | Note |
| --- | --- |
| Dropbox | Real files on all systems. Make the folder available offline. |
| Google Drive for desktop | Streams files by default. Make the folder available offline. File events can be late; the 10-second scan covers this. |
| OneDrive | Files On-Demand can give placeholders. Select Always keep on this device. |
| Syncthing | Works. No cloud copy. |
| iCloud Drive | Not recommended: poor Windows support, and files can be removed from the device. |

## Build order

1. **Change model** (pure Rust): change type, clock, merge, conflict detection,
   and resolution. Tests: same result in any order, repeats, clock skew,
   sequential and concurrent edits, resolution, and pending targets.
2. **Folder transport:** batches, device files, skipped files, conflict
   copies, and a duplicate device ID. Tests use three temporary device folders.
3. **Local connection:** projection of local values, the three-way round,
   apply through guarded writes, Steam App ID mapping, pending games, shared
   settings, joining, device state, and a duplicate device ID. Media, manual
   games, and archive state are deferred until a feature needs them.
4. **App connection, Settings section, and review:** run rounds in the app,
   the Sync section, conflict review, join review, and conflict marks on cards.
5. **Encrypted keys:** sync key protected by the passphrase, sealed API keys
   with keyed fingerprints, credential store writes, and the API keys group.
   The Steam key now; AI keys use the same path.
6. **Live check** between macOS and Omarchy on one Dropbox folder and one
   Google Drive folder.
7. **Windows build:** GPUI Windows backend, `keyring` `windows-native`,
   directories, and file events. Then a check with three devices.
8. **Compaction:** each device writes its merged state to a snapshot and
   combines its own old batches.

Acceptance checks: edits on two open devices at the same time; a concurrent
edit that opens the review; one device offline for a day; a new device that
joins a folder with data; a wrong passphrase; a lost passphrase.
