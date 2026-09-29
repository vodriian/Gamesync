//! The Sync section: folder, devices, status, and review of conflicting edits.
use super::*;
use crate::sync_runtime::{self, KeyState, SyncState};
use gamesync_desktop::settings::{GroupBy, LibraryDisplay, LibraryView, SortBy};
use gamesync_desktop::sync::{Conflict, FieldKey, Target};
use gpui::{div, px, PathPromptOptions};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    v_flex, ActiveTheme as _, Disableable as _, StyledExt as _,
};
use serde_json::Value;

/// Rows shown at once. Bulk choices apply to all conflicts, shown or not.
const MAX_REVIEW_ROWS: usize = 50;
/// Shown for a value that this version cannot describe.
const UNKNOWN_VALUE: &str = "A value this version cannot show";

impl SettingsView {
    pub(super) fn device_name_input(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let name = settings::load()
            .ok()
            .and_then(|s| s.device_name)
            .unwrap_or_else(sync_runtime::host_name);
        cx.new(|cx| InputState::new(window, cx).default_value(name))
    }

    fn choose_sync_folder(&mut self, cx: &mut Context<Self>) {
        if self.sync_busy {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Use for sync".into()),
        });
        self.sync_busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let folder = match paths.await {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.remove(0),
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.sync_busy = false;
                        this.message = format!("Could not open the folder picker: {error:#}");
                        cx.notify();
                    });
                    return;
                }
                _ => {
                    let _ = this.update(cx, |this, cx| {
                        this.sync_busy = false;
                        cx.notify();
                    });
                    return;
                }
            };
            let chosen = folder.clone();
            let result = cx
                .background_spawn(async move {
                    let existing = gamesync_desktop::sync::SyncFolder::exists(&chosen);
                    let handle = sync_runtime::open(&chosen, true)?;
                    settings::update(|s| s.sync_folder = Some(chosen))?;
                    Ok::<_, anyhow::Error>((handle, existing))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.sync_busy = false;
                match result {
                    Ok((handle, existing)) => {
                        this.sync.update(cx, |sync, cx| {
                            *sync = SyncState {
                                handle: Some(handle),
                                folder: Some(folder),
                                ..Default::default()
                            };
                            cx.notify();
                        });
                        this.message = if existing {
                            "Joined the sync folder. Different values will appear for review."
                        } else {
                            "Sync started. Select the same folder on your other computers."
                        }
                        .into();
                        cx.emit(SettingsEvent::Refresh);
                    }
                    Err(error) => this.message = format!("Could not use this folder: {error:#}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn stop_sync(&mut self, cx: &mut Context<Self>) {
        let waiting = self.sync.read(cx).waiting;
        if waiting > 0 && !self.stop_confirm {
            self.stop_confirm = true;
            self.message = format!(
                "{waiting} changes on this device are not in the folder yet. Stop anyway to discard them from sync; your library keeps them."
            );
            cx.notify();
            return;
        }
        self.stop_confirm = false;
        self.sync_busy = true;
        let handle = self.sync.read(cx).handle.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { sync_runtime::stop(handle.as_deref()) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.sync_busy = false;
                match result {
                    Ok(()) => {
                        this.sync.update(cx, |sync, cx| {
                            *sync = SyncState::default();
                            cx.notify();
                        });
                        this.message =
                            "Sync stopped. Your library and the folder are unchanged.".into();
                    }
                    Err(error) => this.message = format!("Could not stop sync: {error:#}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save_device_name(&mut self, cx: &mut Context<Self>) {
        let name = self.device_name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.message = "Enter a name for this device.".into();
            cx.notify();
            return;
        }
        let Some(handle) = self.sync.read(cx).handle.clone() else {
            self.message = "Connect the sync folder first.".into();
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { sync_runtime::rename(&handle, name) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.message = match result {
                    Ok(()) => "Device name saved.".into(),
                    Err(error) => format!("Could not save the device name: {error:#}"),
                };
                cx.emit(SettingsEvent::Refresh);
                cx.notify();
            });
        })
        .detach();
    }

    /// Write the chosen values. The next round applies them to the library.
    fn resolve(&mut self, choices: Vec<(FieldKey, Value)>, cx: &mut Context<Self>) {
        let Some(handle) = self.sync.read(cx).handle.clone() else {
            return;
        };
        if choices.is_empty() || self.sync_busy {
            return;
        }
        self.sync_busy = true;
        let keys: Vec<FieldKey> = choices.iter().map(|(key, _)| key.clone()).collect();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { sync_runtime::resolve(&handle, choices) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.sync_busy = false;
                match result {
                    Ok(issues) => {
                        this.sync.update(cx, |sync, cx| {
                            sync.conflicts.retain(|c| !keys.contains(&c.key));
                            cx.notify();
                        });
                        this.message = issues.first().cloned().unwrap_or_default();
                    }
                    Err(error) => this.message = format!("Could not save the choice: {error:#}"),
                }
                cx.emit(SettingsEvent::Refresh);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Run a key action with the entered passphrase. Scrypt takes about a
    /// second, so it runs in the background.
    fn key_action(
        &mut self,
        confirm: bool,
        action: fn(&sync_runtime::Handle, &str) -> anyhow::Result<()>,
        done: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.sync.read(cx).handle.clone() else {
            return;
        };
        let passphrase = self.passphrase.read(cx).value().to_string();
        if confirm && passphrase != self.passphrase_confirm.read(cx).value().as_ref() {
            self.message = "The passphrases are different.".into();
            cx.notify();
            return;
        }
        if self.sync_busy {
            return;
        }
        self.sync_busy = true;
        self.message = "Working with the passphrase…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { action(&handle, &passphrase) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.sync_busy = false;
                match result {
                    Ok(()) => {
                        this.clear_passphrase = true;
                        this.message = done.into();
                        cx.emit(SettingsEvent::Refresh);
                    }
                    Err(error) => this.message = format!("{error:#}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn keys_group(&mut self, keys: KeyState, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let busy = self.sync_busy;
        let input = |state: &Entity<InputState>| div().w(px(320.)).child(Input::new(state));
        let group = v_flex()
            .p_5()
            .gap_4()
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().secondary)
            .child(div().font_semibold().child("API keys"));
        match keys {
            KeyState::Off => group
                .child(
                    "Sync the Steam API key, and later AI keys, encrypted with a passphrase. \
                     Enter the same passphrase once on each computer.",
                )
                .child(input(&self.passphrase))
                .child(input(&self.passphrase_confirm))
                .child(
                    h_flex().child(
                        Button::new("enable-keys")
                            .primary()
                            .label("Turn on key sync")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.key_action(
                                    true,
                                    sync_runtime::enable_keys,
                                    "Key sync is on. Enter the same passphrase on your other computers.",
                                    cx,
                                )
                            })),
                    ),
                )
                .child(div().text_sm().text_color(muted).child(
                    "Use at least 10 characters. GameSync cannot recover a lost passphrase. \
                     A computer that already has the keys can set a new one.",
                )),
            KeyState::Locked | KeyState::Replaced => group
                .child(if keys == KeyState::Replaced {
                    "The sync key changed on another computer. Enter the passphrase again."
                } else {
                    "Key sync is on. Enter its passphrase to send and receive keys on this computer."
                })
                .child(input(&self.passphrase))
                .child(
                    h_flex().child(
                        Button::new("unlock-keys")
                            .primary()
                            .label("Unlock")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.key_action(
                                    false,
                                    sync_runtime::unlock_keys,
                                    "Keys unlocked on this computer.",
                                    cx,
                                )
                            })),
                    ),
                ),
            KeyState::Review => group.child(
                "Two computers turned on key sync at the same time. \
                 Choose one in Review changes, then unlock with its passphrase.",
            ),
            KeyState::Unlocked => group
                .child("Keys sync on this computer: Steam API key.")
                .child(div().font_semibold().child("Change passphrase"))
                .child(input(&self.passphrase))
                .child(input(&self.passphrase_confirm))
                .child(
                    h_flex().child(
                        Button::new("change-passphrase")
                            .label("Change passphrase")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.key_action(
                                    true,
                                    sync_runtime::change_passphrase,
                                    "Passphrase changed. Other computers keep their keys.",
                                    cx,
                                )
                            })),
                    ),
                ),
        }
    }

    fn device_label(&self, device: uuid::Uuid, cx: &gpui::App) -> String {
        let sync = self.sync.read(cx);
        if sync.device == Some(device) {
            return "This device".into();
        }
        sync.devices
            .iter()
            .find(|(id, _)| *id == device)
            .map_or_else(|| "Another device".into(), |(_, info)| info.name.clone())
    }

    fn target_label(&self, target: &Target, cx: &gpui::App) -> String {
        let library = self.library.read(cx);
        let definitions = library.source.as_ref().map(|(_, m)| &m.definitions);
        match target {
            Target::Steam(app) => library
                .games
                .iter()
                .find(|g| {
                    g.record
                        .as_ref()
                        .and_then(|r| r.game.steam.as_ref())
                        .is_some_and(|s| s.app_id == *app)
                })
                .map_or_else(|| format!("Steam app {app}"), |g| g.title.clone()),
            Target::Status(key) => definitions.and_then(|d| d.status(key)).map_or_else(
                || format!("Status {key}"),
                |s| format!("Status {}", s.label),
            ),
            Target::Collection(id) => definitions
                .and_then(|d| d.collections.iter().find(|c| c.id == *id))
                .map_or_else(
                    || "A collection".into(),
                    |c| format!("Collection {}", c.name),
                ),
            Target::Library => "Library".into(),
            Target::Settings => "Settings".into(),
            Target::Secret(name) if name == "sync_key" => "Key sync".into(),
            Target::Secret(name) if name == "steam_api_key" => "Steam API key".into(),
            other => other.to_string(),
        }
    }

    fn status_name(&self, key: &str, cx: &gpui::App) -> String {
        self.library
            .read(cx)
            .source
            .as_ref()
            .and_then(|(_, m)| m.definitions.status(key))
            .map_or_else(|| key.to_owned(), |s| s.label.clone())
    }

    fn collection_name(&self, id: &str, cx: &gpui::App) -> String {
        self.library
            .read(cx)
            .source
            .as_ref()
            .and_then(|(_, m)| {
                m.definitions
                    .collections
                    .iter()
                    .find(|c| c.id.to_string() == id)
            })
            .map_or_else(|| "a collection".into(), |c| c.name.clone())
    }

    fn field_label(&self, field: &str, cx: &gpui::App) -> String {
        if let Some(id) = field.strip_prefix("personal.collections.") {
            return format!("In {}", self.collection_name(id, cx));
        }
        if field.starts_with("section_views.") {
            return "View of a section".into();
        }
        match field {
            "personal.status" => "Status",
            "personal.rating" => "Rating",
            "personal.favorite" => "Favorite",
            "personal.hidden" => "Hidden",
            "personal.tags" => "Tags",
            "personal.notes" => "Notes",
            "personal.description" => "Description",
            "personal.board_rank" => "Board position",
            "name" | "label" => "Name",
            "archived" => "Archived",
            "default_status" => "Default status",
            "status_order" => "Status order",
            "collection_order" => "Collection order",
            "recommendation_eligible" => "Used for recommendations",
            "reduce_motion" => "Reduce motion",
            "show_hidden_games" => "Show hidden games",
            "library_display" => "Sort and grouping",
            "smart_groups_open" => "Open smart groups",
            "store_country" => "Store country",
            "wrapped" => "Passphrase",
            "value" => "Value",
            other => other,
        }
        .into()
    }

    fn value_label(&self, field: &str, value: &Value, cx: &gpui::App) -> String {
        let text = match (field, value) {
            (_, Value::Null) => "Not set".into(),
            // Encrypted values cannot be shown; the device name tells them apart.
            (_, Value::Object(object)) if object.contains_key("age") => "Protected value".into(),
            ("personal.rating", Value::Number(n)) => {
                let stars = n.as_f64().unwrap_or(0.) / 2.;
                format!("{stars} stars")
            }
            ("personal.status" | "default_status", Value::String(key)) => self.status_name(key, cx),
            ("status_order", Value::Array(keys)) => keys
                .iter()
                .filter_map(Value::as_str)
                .map(|key| self.status_name(key, cx))
                .collect::<Vec<_>>()
                .join(", "),
            ("collection_order", Value::Array(ids)) => ids
                .iter()
                .filter_map(Value::as_str)
                .map(|id| self.collection_name(id, cx))
                .collect::<Vec<_>>()
                .join(", "),
            ("library_display", Value::Object(_)) => {
                serde_json::from_value::<LibraryDisplay>(value.clone())
                    .map_or_else(|_| UNKNOWN_VALUE.into(), |d| display_label(&d))
            }
            ("smart_groups_open", Value::Array(groups)) => count(groups.len(), "group open"),
            (field, Value::String(view)) if field.starts_with("section_views.") => {
                serde_json::from_value::<LibraryView>(value.clone())
                    .map_or_else(|_| view.clone(), |v| view_label(v).into())
            }
            (_, Value::Bool(true)) => "Yes".into(),
            (_, Value::Bool(false)) => "No".into(),
            (_, Value::String(text)) if text.is_empty() => "Empty".into(),
            (_, Value::String(text)) => text.clone(),
            (_, Value::Array(items)) if items.iter().all(Value::is_string) => items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            (_, Value::Number(n)) => n.to_string(),
            // A structured value from a newer version, or one without a
            // label. Raw JSON tells the user nothing.
            _ => UNKNOWN_VALUE.into(),
        };
        // One line, bounded, so a long note does not stretch the review.
        let line = text.lines().next().unwrap_or_default();
        let mut short: String = line.chars().take(120).collect();
        if short.len() < text.len() {
            short.push('…');
        }
        short
    }

    pub(super) fn sync_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = cx.theme().secondary;
        let radius = cx.theme().radius_lg;
        let muted = cx.theme().muted_foreground;
        let group = || v_flex().p_5().gap_4().rounded(radius).bg(surface);
        let available = self.library.read(cx).source.is_some() && !self.library.read(cx).demo;
        let busy = self.sync_busy;
        let sync = self.sync.read(cx);

        if !sync.enabled() {
            return v_flex().gap_6().child(
                group()
                    .child(div().font_semibold().child("Sync across devices"))
                    .child(
                        "Keep collections, statuses, notes, tags, ratings, favorites, and \
                         settings the same on your computers. GameSync uses a folder that you \
                         already sync. Steam data stays on each computer.",
                    )
                    .child(div().text_sm().text_color(muted).child(
                        "Select a folder in Dropbox, Google Drive, or OneDrive, and keep it \
                         available offline. GameSync makes a GameSync Sync folder in it.",
                    ))
                    .child(
                        h_flex().child(
                            Button::new("choose-sync-folder")
                                .primary()
                                .label("Choose folder…")
                                .disabled(busy || !available)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.choose_sync_folder(cx)),
                                ),
                        ),
                    )
                    .when(!available, |group| {
                        group.child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("Sync is not available for the sample library."),
                        )
                    }),
            );
        }

        let folder = sync
            .folder
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let connected = sync.handle.is_some();
        let status = if !connected {
            "Waiting for the folder. Check that it is available on this computer.".to_string()
        } else if !sync.conflicts.is_empty() {
            format!(
                "{} your review below.",
                count(sync.conflicts.len(), "change needs")
            )
        } else if sync.waiting > 0 {
            format!(
                "{} to be written to the folder.",
                count(sync.waiting, "change waits")
            )
        } else if sync.last_round_ms.is_some() {
            "Up to date.".into()
        } else {
            "Checking the folder…".into()
        };
        let received = sync
            .last_received
            .map(|(at, n)| format!("Last received: {}, {}.", count(n, "change"), age(at)));
        let pending = (sync.pending > 0).then(|| {
            format!(
                "{} for games that Steam has not added on this computer.",
                count(sync.pending, "value waits")
            )
        });
        let issues: Vec<String> = sync.issues.iter().take(3).cloned().collect();
        let devices: Vec<(bool, String)> = sync
            .devices
            .iter()
            .map(|(id, info)| {
                let this_device = sync.device == Some(*id);
                let text = format!(
                    "{}{} · {} · {}",
                    info.name,
                    if this_device { " (this device)" } else { "" },
                    platform(&info.platform),
                    if info.last_seen_ms == 0 {
                        "not seen yet".into()
                    } else {
                        format!("last change {}", age(info.last_seen_ms))
                    }
                );
                (this_device, text)
            })
            .collect();
        let conflicts = sync.conflicts.clone();
        let own = sync.device;
        let keys = sync.keys;

        let status_group = group()
            .child(div().font_semibold().child("Folder"))
            .child(div().text_sm().child(folder.clone()))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("sync-now")
                            .primary()
                            .label("Sync now")
                            .disabled(busy)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Refresh))),
                    )
                    .child(
                        Button::new("show-sync-folder")
                            .label("Show folder")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(folder) = &this.sync.read(cx).folder {
                                    cx.reveal_path(folder);
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("stop-sync")
                            .label(if self.stop_confirm {
                                "Stop anyway"
                            } else {
                                "Stop syncing"
                            })
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.stop_sync(cx))),
                    ),
            )
            .child(div().font_semibold().child(status))
            .children(received)
            .children(pending)
            .children(
                issues
                    .into_iter()
                    .map(|issue| div().text_sm().text_color(muted).child(issue)),
            );

        let device_group = group()
            .child(div().font_semibold().child("This device"))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(260.)).child(Input::new(&self.device_name)))
                    .child(
                        Button::new("save-device-name")
                            .label("Save name")
                            .disabled(!connected)
                            .on_click(cx.listener(|this, _, _, cx| this.save_device_name(cx))),
                    ),
            )
            .child(div().font_semibold().child("Devices"))
            .when(devices.is_empty(), |group| {
                group.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("No devices found yet."),
                )
            })
            .children(devices.into_iter().map(|(this_device, text)| {
                div()
                    .text_sm()
                    .when(!this_device, |row| row.text_color(muted))
                    .child(text)
            }));

        let mut column = v_flex().gap_6().child(status_group);
        if !conflicts.is_empty() {
            column = column.child(self.review_group(&conflicts, own, cx));
        }
        if connected {
            column = column.child(self.keys_group(keys, cx));
        }
        column.child(device_group)
    }

    fn review_group(
        &mut self,
        conflicts: &[Conflict],
        own: Option<uuid::Uuid>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = cx.theme().secondary;
        let radius = cx.theme().radius_lg;
        let muted = cx.theme().muted_foreground;
        let busy = self.sync_busy;

        // Bulk choices: this device, or each other device in the conflicts.
        let keep_local: Vec<(FieldKey, Value)> = conflicts
            .iter()
            .map(|c| (c.key.clone(), c.local.clone()))
            .collect();
        let mut others: Vec<uuid::Uuid> = conflicts
            .iter()
            .flat_map(|c| c.candidates.iter().map(|change| change.at.device))
            .filter(|device| Some(*device) != own)
            .collect();
        others.sort();
        others.dedup();
        let bulk_other: Vec<(String, Vec<(FieldKey, Value)>)> = others
            .into_iter()
            .map(|device| {
                let choices = conflicts
                    .iter()
                    .map(|c| {
                        // The newest value from that device; others keep this device's value.
                        let value = c
                            .candidates
                            .iter()
                            .rev()
                            .find(|change| change.at.device == device)
                            .map_or_else(|| c.local.clone(), |change| change.value.clone());
                        (c.key.clone(), value)
                    })
                    .collect();
                (self.device_label(device, cx), choices)
            })
            .collect();

        let rows: Vec<_> = conflicts
            .iter()
            .take(MAX_REVIEW_ROWS)
            .enumerate()
            .map(|(index, conflict)| self.review_row(index, conflict, own, cx))
            .collect();
        let hidden = conflicts.len().saturating_sub(MAX_REVIEW_ROWS);

        v_flex()
            .p_5()
            .gap_4()
            .rounded(radius)
            .bg(surface)
            .child(div().font_semibold().child("Review changes"))
            .child(div().text_sm().text_color(muted).child(
                "Two devices changed these values before either saw the other change. \
                 Choose the value to keep. Until then, each device keeps its own value.",
            ))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("keep-all-local")
                            .label("Keep all from this device")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.resolve(keep_local.clone(), cx)
                            })),
                    )
                    .children(bulk_other.into_iter().enumerate().map(
                        |(index, (name, choices))| {
                            Button::new(("use-all-from", index))
                                .label(format!("Use all from {name}"))
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.resolve(choices.clone(), cx)
                                }))
                        },
                    )),
            )
            .children(rows)
            .when(hidden > 0, |group| {
                group.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(format!("{hidden} more appear after you review these.")),
                )
            })
    }

    fn review_row(
        &self,
        index: usize,
        conflict: &Conflict,
        own: Option<uuid::Uuid>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let field = conflict.key.1.clone();
        let title = format!(
            "{} · {}",
            self.target_label(&conflict.key.0, cx),
            self.field_label(&field, cx)
        );
        let busy = self.sync_busy;
        let key = conflict.key.clone();
        let local = conflict.local.clone();
        let mut values = vec![(
            "This device".to_string(),
            self.value_label(&field, &local, cx),
        )];
        let mut buttons = Vec::new();
        {
            let (key, value) = (key.clone(), local.clone());
            buttons.push(
                Button::new(("keep-this", index))
                    .label("Keep this")
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.resolve(vec![(key.clone(), value.clone())], cx)
                    })),
            );
        }
        let mut seen = vec![local.clone()];
        for (n, change) in conflict.candidates.iter().enumerate() {
            if seen.contains(&change.value) || Some(change.at.device) == own {
                continue;
            }
            seen.push(change.value.clone());
            let name = self.device_label(change.at.device, cx);
            values.push((name.clone(), self.value_label(&field, &change.value, cx)));
            let (key, value) = (key.clone(), change.value.clone());
            buttons.push(
                Button::new(("use-other", index * 16 + n))
                    .label(format!("Use {name}"))
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.resolve(vec![(key.clone(), value.clone())], cx)
                    })),
            );
        }
        if let Some(both) = keep_both(&field, &seen) {
            buttons.push(
                Button::new(("keep-both", index))
                    .label("Keep both")
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.resolve(vec![(key.clone(), both.clone())], cx)
                    })),
            );
        }
        v_flex()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(div().font_semibold().child(title))
            .children(values.into_iter().map(|(name, value)| {
                h_flex()
                    .gap_2()
                    .text_sm()
                    .child(div().w(px(140.)).flex_shrink_0().child(name))
                    .child(div().min_w_0().child(value))
            }))
            .child(h_flex().flex_wrap().gap_2().children(buttons))
    }

    pub fn set_reduce_motion(&mut self, reduce: bool, cx: &mut Context<Self>) {
        self.reduce_motion = reduce;
        cx.notify();
    }
}

/// Text joins the versions; tags combine. Other fields have no combined value.
fn keep_both(field: &str, values: &[Value]) -> Option<Value> {
    if values.len() < 2 {
        return None;
    }
    match field {
        "personal.notes" | "personal.description" => {
            let texts: Vec<&str> = values
                .iter()
                .filter_map(Value::as_str)
                .filter(|t| !t.is_empty())
                .collect();
            (texts.len() > 1).then(|| Value::String(texts.join("\n\n")))
        }
        "personal.tags" => {
            let mut tags: Vec<String> = Vec::new();
            for value in values {
                for tag in value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                        tags.push(tag.to_owned());
                    }
                }
            }
            Some(Value::from(tags))
        }
        _ => None,
    }
}

/// "1 change", "2 changes"; "1 change waits", "2 changes wait".
fn display_label(display: &LibraryDisplay) -> String {
    let sort = match display.sort {
        SortBy::Name => "name",
        SortBy::Status => "status",
        SortBy::Hours => "hours played",
        SortBy::Collection => "collection",
    };
    let order = if display.descending {
        "descending"
    } else {
        "ascending"
    };
    let group = match display.group {
        GroupBy::None => "no groups",
        GroupBy::Status => "grouped by status",
        GroupBy::Collections => "grouped by collection",
    };
    let board = if display.board_manual {
        "Board in manual order"
    } else {
        "Board sorted"
    };
    let mut label = format!("Sort by {sort}, {order}, {group}, {board}");
    if !display.grid_title {
        label.push_str(", Grid without titles");
    }
    if !display.grid_metadata {
        label.push_str(", Grid without details");
    }
    label
}

fn view_label(view: LibraryView) -> &'static str {
    match view {
        LibraryView::Cards => "Cards",
        LibraryView::Grid => "Grid",
        LibraryView::Table => "Table",
        LibraryView::Board => "Board",
    }
}

fn count(n: usize, phrase: &str) -> String {
    if n == 1 {
        return format!("1 {phrase}");
    }
    let (noun, verb) = phrase.split_once(' ').unwrap_or((phrase, ""));
    let verb = verb.strip_suffix('s').unwrap_or(verb);
    format!("{n} {noun}s {verb}").trim_end().to_owned()
}

fn platform(os: &str) -> &str {
    match os {
        "macos" => "macOS",
        "linux" => "Linux",
        "windows" => "Windows",
        other => other,
    }
}

fn age(ms: u64) -> String {
    let seconds = gamesync_desktop::sync::wall_ms().saturating_sub(ms) / 1000;
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} hours ago", seconds / 3600)
    } else {
        format!("{} days ago", seconds / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::{count, display_label, keep_both, GroupBy, LibraryDisplay, SortBy};

    #[test]
    fn counts_use_singular_and_plural() {
        assert_eq!(count(1, "change"), "1 change");
        assert_eq!(count(3, "change"), "3 changes");
        assert_eq!(count(1, "change waits"), "1 change waits");
        assert_eq!(count(2, "change waits"), "2 changes wait");
        assert_eq!(count(2, "group open"), "2 groups open");
    }

    #[test]
    fn sort_and_grouping_read_as_text() {
        assert_eq!(
            display_label(&LibraryDisplay::default()),
            "Sort by name, ascending, no groups, Board in manual order"
        );
        let display = LibraryDisplay {
            sort: SortBy::Hours,
            descending: true,
            group: GroupBy::Status,
            board_manual: false,
            grid_title: false,
            grid_metadata: true,
        };
        assert_eq!(
            display_label(&display),
            "Sort by hours played, descending, grouped by status, Board sorted, Grid without titles"
        );
    }
    use serde_json::json;

    #[test]
    fn keep_both_joins_text_and_combines_tags() {
        assert_eq!(
            keep_both("personal.notes", &[json!("Mac"), json!("Linux")]),
            Some(json!("Mac\n\nLinux"))
        );
        assert_eq!(
            keep_both(
                "personal.tags",
                &[json!(["Short", "Co-op"]), json!(["co-op", "Cozy"])]
            ),
            Some(json!(["Short", "Co-op", "Cozy"]))
        );
        assert_eq!(keep_both("personal.rating", &[json!(4), json!(8)]), None);
        assert_eq!(
            keep_both("personal.notes", &[json!("Mac"), json!("")]),
            None
        );
    }
}
