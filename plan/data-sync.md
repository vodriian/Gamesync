# Data sync across devices

Status: approved direction, September 29. No code yet. Branch: `data-sync-mode`.

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
| Override covers | Yes | Content-addressed image files |
| Manual games | Yes | Identity, title, and personal data |
| Library definitions | Yes | Statuses, collections, bound Steam account ID, and future field definitions and saved views |
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
| Theme and appearance | Omarchy mode (Linux only) |
| Reduce motion | Sync folder, device ID, device name |
| Show hidden games | Last Steam sync times |
| Sort, descending, and grouping | Detected Steam country |
| View choice for each section | Window size and position |
| Open smart groups | Legacy library path |
| Store country | |

Omarchy mode already replaces the theme on the device that turns it on. The
shared theme stays saved and applies again when the mode is off.

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
  changes/<device-id>/<seq>.jsonl    # immutable change batches
  media/<sha256>.<ext>               # override covers; immutable
  snapshots/<device-id>.json         # later: compacted state of one device
```

- `device-id` is a random UUID in device-local settings. It never goes in the
  library or shared settings, so a copied profile does not copy the identity.
- A batch is written once to a temporary name, then renamed, and never changed.
  A reader skips a file that it cannot parse and tries again at the next scan.
  A partial download or a placeholder is not an error.
- File names use only lowercase ASCII letters, digits, `-`, and `.`. They are
  short and safe on Windows, macOS, Linux, and case-insensitive volumes.
- Cloud conflict copies (`(conflicted copy)`, `.sync-conflict-`, ` (1)`) are
  ignored and reported. They occur only if two devices share one device ID.

## Change model

A change sets one value:

```json
{"id":"<uuid>","at":"<hlc>","target":"steam:620","field":"personal.rating","value":8,"base":["<change-id>"]}
```

- **Targets:** `steam:<app-id>` for Steam games, `game:<uuid>` for manual
  games, `status:<key>`, `collection:<uuid>`, `library`, `settings`, and
  `secret:<name>`.
- **Fields are small.** Collection membership is one boolean for each game and
  collection. Status order and board order use fractional ranks, as
  `board_rank` does now. Adds and reorders on two devices do not collide.
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
- **Deletion:** collections and statuses stay archived, as now. Nothing is
  removed from the logs.

### Conflict review

- The sidebar footer and the Sync section show "N changes need review".
- The review lists each value: game or item, field, this device's value, the
  other value, device name, and time.
- Choices for each value: **Keep this**, **Use other**. Text fields and tags
  also have **Keep both**: text is joined with a separator, tags are combined.
- Bulk choices: **Keep all from this device** and **Use all from <device>**.
- The game card marks a field that has a conflict.

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

- API keys are changes with target `secret:<name>`, for example
  `secret:steam_api_key` and `secret:ai.openai`. Their value is encrypted.
  Target and field names are not secret; the key value is.
- Encryption uses the `age` format with a passphrase (scrypt key derivation).
  This is a standard format with an audited Rust crate and a command-line tool
  for manual recovery. GameSync does not use custom cryptography.
- `sync.json` stores a small encrypted check value. A wrong passphrase is
  found at once and changes nothing.
- The user enters the passphrase once on each device. GameSync saves it in the
  OS credential store (macOS Keychain, Linux Secret Service, later Windows
  Credential Manager). Received keys also go to the OS credential store, never
  to a plain file.
- A lost passphrase: personal data and settings still sync. Keys cannot be
  read. The user enters keys again and sets a new passphrase; this makes new
  encrypted values and a new check value.
- Changing the passphrase encrypts all keys again in new changes.

Personal data and settings are not encrypted. The cloud provider can read
notes and tags. See the open questions.

## Connection to the local app

- **Out:** after a guarded personal, definitions, or settings write succeeds,
  the sync service adds one change for each changed field to the next batch.
  It writes a batch 2 seconds after the last edit and when the app closes.
- **In:** a file watcher, plus a scan every 10 seconds, finds new batches. The
  service merges them and applies the result through the existing guarded
  writes. Steam targets map to local games by Steam App ID.
- A change for a game that this device does not have yet stays pending. It is
  applied when Steam sync adds the game.
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

A new section after Steam:

- **Sync across devices:** off by default.
- **Folder:** path, Choose folder, and Show in file manager. Help text: "Select
  a folder in Dropbox, Google Drive, or OneDrive. Keep it available offline."
- **Passphrase:** set, enter, or change. It protects API keys.
- **This device:** an editable name. The default is the host name.
- **Status:** "Up to date", "Waiting for folder", "N changes need review", or
  an error, with the time and device of the last received change.
- **Devices:** name, platform, and last seen for each device.
- **Review changes**, **Sync now**, and **Stop syncing**.
- Messages for unreadable batches, a newer format, a wrong passphrase, or a
  folder that is only online.

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
2. **Folder transport:** batches, device files, media, skipped files, conflict
   copies, and a duplicate device ID. Tests use three temporary device folders.
3. **Local connection:** outgoing hooks, incoming apply, Steam App ID mapping,
   pending changes, shared settings, and joining.
4. **Settings section and review:** the Sync section, conflict review, join
   review, and conflict marks on cards.
5. **Encrypted keys:** `age` dependency, passphrase, check value, key changes,
   and credential store writes. Include the Steam key; AI keys use the same path.
6. **Live check** between macOS and Omarchy on one Dropbox folder and one
   Google Drive folder.
7. **Windows build:** GPUI Windows backend, `keyring` `windows-native`,
   directories, and file events. Then a check with three devices.
8. **Compaction:** each device writes its merged state to a snapshot and
   combines its own old batches.

Acceptance checks: edits on two open devices at the same time; a concurrent
edit that opens the review; one device offline for a day; a new device that
joins a folder with data; a wrong passphrase; a lost passphrase.

## Open questions

- Encrypt personal data and settings too? Recommendation: no for now. It makes
  the passphrase mandatory for all sync and makes problems harder to inspect.
  The format allows it later.
