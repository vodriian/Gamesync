//! Write received values to the local library and settings.
//!
//! Every write uses the existing guarded library operations, so a received
//! value is checked like a local edit. A value that cannot be applied stays
//! unconfirmed and is tried again at the next sync round.

use super::{
    project::{default_definitions, plain_name, unhex, PERSONAL_FIELDS},
    FieldKey, Target,
};
use crate::{
    board,
    library::{
        CollectionDefinition, LibraryDefinitions, LibraryRevision, LibraryStore, StatusDefinition,
    },
    records::{ExtraFields, GameRevision, PersonalData},
    settings::Settings,
};
use anyhow::{bail, ensure, Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};
use uuid::Uuid;

/// A received value for one field. `Null` clears the field.
#[derive(Clone, Debug, PartialEq)]
pub struct Apply {
    pub key: FieldKey,
    pub value: Value,
    /// The changes that set this value. Stored after a successful write.
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Default)]
pub struct Applied {
    /// Indexes into the given values that were written.
    pub done: Vec<usize>,
    /// Values for games that this device does not have yet.
    pub pending: usize,
    pub issues: Vec<String>,
}

/// Apply library values: definitions first, so games can use new statuses and
/// collections. `games` is the latest loaded library; a stale record fails the
/// guarded write and is tried again later. Settings values are ignored; use
/// `apply_settings`.
pub fn apply_library(root: &Path, games: &[GameRevision], values: &[Apply]) -> Applied {
    let mut applied = Applied::default();
    let definitions: Vec<usize> = indexes(values, |t| {
        matches!(
            t,
            Target::Library | Target::Status(_) | Target::Collection(_)
        )
    });
    if !definitions.is_empty() {
        match apply_definitions(root, values, &definitions) {
            Ok(()) => applied.done.extend(&definitions),
            Err(error) => applied
                .issues
                .push(format!("Could not apply library settings: {error:#}")),
        }
    }

    let mut targets: BTreeMap<&Target, Vec<usize>> = BTreeMap::new();
    for (index, value) in values.iter().enumerate() {
        if matches!(value.key.0, Target::Steam(_) | Target::Game(_)) {
            targets.entry(&value.key.0).or_default().push(index);
        }
    }
    if targets.is_empty() {
        return applied;
    }
    let manifest = match LibraryStore::open(root).and_then(|store| store.inspect()) {
        Ok(snapshot) => match snapshot.current() {
            Some(manifest) => manifest.clone(),
            None => {
                applied
                    .issues
                    .push("Library settings have conflicting versions".into());
                return applied;
            }
        },
        Err(error) => {
            applied
                .issues
                .push(format!("Could not read library settings: {error:#}"));
            return applied;
        }
    };
    let steam = steam_games(games);
    for (target, indexes) in targets {
        // Manual games do not sync yet; no feature creates them.
        let Some(record) = (match target {
            Target::Steam(app) => steam.get(app).copied(),
            _ => None,
        }) else {
            applied.pending += indexes.len();
            continue;
        };
        match apply_game(root, &manifest, record, values, &indexes) {
            Ok(()) => applied.done.extend(indexes),
            Err(error) => applied.issues.push(format!("{target}: {error:#}")),
        }
    }
    applied.done.sort_unstable();
    applied
}

/// The same selection as `project`: the lowest game ID for each Steam game.
fn steam_games(games: &[GameRevision]) -> BTreeMap<u32, &GameRevision> {
    let mut sorted: Vec<&GameRevision> = games.iter().collect();
    sorted.sort_by_key(|game| game.game_id);
    let mut steam = BTreeMap::new();
    for game in sorted {
        if let Some(data) = &game.game.steam {
            steam.entry(data.app_id).or_insert(game);
        }
    }
    steam
}

/// Apply shared settings values to device settings. The caller saves them.
pub fn apply_settings(settings: &mut Settings, values: &[Apply]) -> Applied {
    let mut applied = Applied::default();
    for index in indexes(values, |t| *t == Target::Settings) {
        let value = &values[index];
        match apply_setting(settings, &value.key.1, &value.value) {
            Ok(()) => applied.done.push(index),
            Err(error) => applied
                .issues
                .push(format!("Setting {}: {error:#}", value.key.1)),
        }
    }
    applied
}

fn indexes(values: &[Apply], wanted: impl Fn(&Target) -> bool) -> Vec<usize> {
    (0..values.len())
        .filter(|i| wanted(&values[*i].key.0))
        .collect()
}

/// `Null` becomes the type's default, so a cleared field reads as unset.
fn parse<T: DeserializeOwned + Default>(value: &Value) -> Result<T> {
    if value.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(value.clone()).context("Value has the wrong type")
}

fn set_extra(extra: &mut ExtraFields, name: &str, value: &Value) -> Result<()> {
    ensure!(plain_name(name), "Unknown field");
    if value.is_null() {
        extra.remove(name);
    } else {
        extra.insert(name.to_owned(), value.clone());
    }
    Ok(())
}

fn apply_definitions(root: &Path, values: &[Apply], indexes: &[usize]) -> Result<()> {
    let store = LibraryStore::open(root)?;
    let snapshot = store.inspect()?;
    let current = snapshot
        .current()
        .context("Library settings have conflicting versions")?
        .clone();
    let mut definitions = current.definitions.clone();
    let defaults = default_definitions();
    let mut status_order = None;
    let mut collection_order = None;
    for index in indexes {
        let Apply { key, value, .. } = &values[*index];
        match &key.0 {
            Target::Library => match key.1.as_str() {
                // An unset value is the default of a new library.
                "name" => {
                    let name: String = parse(value)?;
                    definitions.name = if name.trim().is_empty() {
                        defaults.name.clone()
                    } else {
                        name
                    };
                }
                "default_status" => {
                    let status: String = parse(value)?;
                    definitions.default_status = if status.is_empty() {
                        defaults.default_status.clone()
                    } else {
                        status
                    };
                }
                "status_order" => {
                    let order: Vec<String> = parse(value)?;
                    status_order = Some(if order.is_empty() {
                        defaults.statuses.iter().map(|s| s.key.clone()).collect()
                    } else {
                        order
                    });
                }
                "collection_order" => collection_order = Some(parse::<Vec<Uuid>>(value)?),
                name => set_extra(&mut definitions.extra, name, value)?,
            },
            Target::Status(status_key) => {
                let default = defaults.status(status_key);
                let status = status_mut(&mut definitions, status_key);
                match key.1.as_str() {
                    // Statuses cannot be removed yet. A cleared label of a
                    // built-in status is its default; others keep their label.
                    "label" => {
                        let label: String = parse(value)?;
                        if !label.trim().is_empty() {
                            status.label = label;
                        } else if let Some(default) = default {
                            status.label = default.label.clone();
                        }
                    }
                    "recommendation_eligible" => {
                        status.recommendation_eligible = parse::<Option<bool>>(value)?
                            .unwrap_or_else(|| default.is_some_and(|d| d.recommendation_eligible))
                    }
                    name => set_extra(&mut status.extra, name, value)?,
                }
            }
            Target::Collection(id) => {
                let collection = collection_mut(&mut definitions, *id);
                match key.1.as_str() {
                    "name" => {
                        let name: String = parse(value)?;
                        if !name.trim().is_empty() {
                            collection.name = name;
                        }
                    }
                    "archived" => collection.archived = parse(value)?,
                    name => set_extra(&mut collection.extra, name, value)?,
                }
            }
            _ => {}
        }
    }
    if let Some(order) = status_order {
        sort_by_order(&mut definitions.statuses, &order, |s| s.key.clone());
    }
    if let Some(order) = collection_order {
        sort_by_order(&mut definitions.collections, &order, |c| c.id);
    }
    if definitions != current.definitions {
        store.edit(current.revision_id, definitions)?;
    }
    Ok(())
}

/// A status from another device that is not here yet. Its label arrives in
/// the same round; the key is a safe label until then.
fn status_mut<'a>(definitions: &'a mut LibraryDefinitions, key: &str) -> &'a mut StatusDefinition {
    if definitions.status(key).is_none() {
        definitions.statuses.push(StatusDefinition {
            key: key.to_owned(),
            label: key.to_owned(),
            recommendation_eligible: false,
            extra: ExtraFields::new(),
        });
    }
    definitions
        .statuses
        .iter_mut()
        .find(|s| s.key == key)
        .expect("the status was added above")
}

fn collection_mut(definitions: &mut LibraryDefinitions, id: Uuid) -> &mut CollectionDefinition {
    if !definitions.collections.iter().any(|c| c.id == id) {
        definitions.collections.push(CollectionDefinition {
            id,
            name: "Collection".into(),
            archived: false,
            extra: ExtraFields::new(),
        });
    }
    definitions
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .expect("the collection was added above")
}

/// Items in the received order. Items that the order does not list keep
/// their relative order after it, so a concurrent add is not lost.
fn sort_by_order<T, K: Ord>(items: &mut [T], order: &[K], key: impl Fn(&T) -> K) {
    let position: BTreeMap<&K, usize> = order.iter().enumerate().map(|(i, k)| (k, i)).collect();
    items.sort_by_key(|item| position.get(&key(item)).copied().unwrap_or(usize::MAX));
}

fn apply_game(
    root: &Path,
    manifest: &LibraryRevision,
    record: &GameRevision,
    values: &[Apply],
    indexes: &[usize],
) -> Result<()> {
    let mut personal = record.game.personal.clone();
    // Status first: a status change clears the board rank, and a received
    // rank for the new status must stay.
    let mut ordered: Vec<usize> = indexes.to_vec();
    ordered.sort_by_key(|i| values[*i].key.1 != "personal.status");
    for index in ordered {
        let Apply { key, value, .. } = &values[index];
        apply_personal(
            &mut personal,
            &key.1,
            value,
            &manifest.definitions.default_status,
        )?;
    }
    if personal != record.game.personal {
        LibraryStore::open(root)?.edit_game_personal(
            manifest,
            record.game_id,
            record.revision_id,
            personal,
        )?;
    }
    Ok(())
}

fn apply_personal(
    personal: &mut PersonalData,
    field: &str,
    value: &Value,
    default_status: &str,
) -> Result<()> {
    let Some(name) = field.strip_prefix("personal.") else {
        bail!("Unknown game field");
    };
    if let Some(id) = name.strip_prefix("collections.") {
        let id: Uuid = id.parse().context("Invalid collection ID")?;
        let member = parse::<Option<bool>>(value)?.unwrap_or(false);
        personal.collections.retain(|c| *c != id);
        if member {
            personal.collections.push(id);
        }
        return Ok(());
    }
    match name {
        "status" => {
            let status: String = parse(value)?;
            personal.set_status(if status.is_empty() {
                default_status.to_owned()
            } else {
                status
            });
        }
        "rating" => {
            let rating: Option<u8> = parse(value)?;
            ensure!(
                rating.is_none_or(|r| (1..=10).contains(&r)),
                "Rating is out of range"
            );
            personal.rating = rating;
        }
        "favorite" => personal.favorite = parse(value)?,
        "hidden" => personal.hidden = parse(value)?,
        "tags" => personal.tags = parse(value)?,
        "notes" => personal.notes = parse(value)?,
        "description" => personal.description = parse(value)?,
        "board_rank" => {
            let rank: Option<String> = parse(value)?;
            // An invalid rank from another writer counts as no rank.
            personal.board_rank = rank.filter(|r| board::valid_rank(r));
        }
        name if !PERSONAL_FIELDS.contains(&name) && name != "cover" && name != "collections" => {
            set_extra(&mut personal.extra, name, value)?
        }
        _ => bail!("Unknown game field"),
    }
    Ok(())
}

fn apply_setting(settings: &mut Settings, field: &str, value: &Value) -> Result<()> {
    if let Some(section) = field.strip_prefix("section_views.") {
        let section = unhex(section).context("Invalid section name")?;
        match parse::<Option<crate::settings::LibraryView>>(value)? {
            Some(view) => settings.section_views.insert(section, view),
            None => settings.section_views.remove(&section),
        };
        return Ok(());
    }
    match field {
        "reduce_motion" => settings.reduce_motion = parse(value)?,
        "show_hidden_games" => settings.show_hidden_games = parse(value)?,
        "library_display" => settings.library_display = parse(value)?,
        "smart_groups_open" => settings.smart_groups_open = parse(value)?,
        "store_country" => settings.store_country = parse(value)?,
        "best_on_rules" => settings.best_on_rules = parse(value)?,
        // A setting from a newer version. It is not stored: this version
        // cannot tell whether it is shared or device-local.
        _ => bail!("Unknown setting"),
    }
    Ok(())
}
