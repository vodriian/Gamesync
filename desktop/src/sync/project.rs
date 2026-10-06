//! Local library and settings as sync fields.
//!
//! A field that has its default value is left out: it counts as unset. Then a
//! device that has not set a value takes the value of another device without
//! a review, for example a game that Steam sync adds later.
//!
//! A target that is not present, such as an archived game or a game file that
//! cannot be read now, is not the same as a target with unset fields. Its
//! fields are not read as local edits.

use super::{FieldKey, Target};
use crate::{
    library::LibraryDefinitions,
    records::{ExtraFields, GameRevision},
    settings::{LibraryDisplay, Settings},
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Synced values on this device.
#[derive(Debug, Default)]
pub struct Projection {
    /// Every synced field with a value.
    pub fields: BTreeMap<FieldKey, Value>,
    /// Targets present on this device. Other targets are not read.
    pub targets: BTreeSet<Target>,
}

/// Settings that stay on each device. Earlier builds synced them, so the
/// folder can hold changes for them. Rounds ignore those changes.
const DEVICE_LOCAL_SETTINGS: [&str; 2] = ["theme", "appearance"];

/// False for a field that must not sync, even when the folder has changes
/// for it.
pub(crate) fn is_synced(key: &FieldKey) -> bool {
    !(key.0 == Target::Settings && DEVICE_LOCAL_SETTINGS.contains(&key.1.as_str()))
}

/// Fields read from `personal` of a game record. Other personal fields,
/// including unknown ones, use their own key. `cover` is a device path into
/// local media and does not sync until media files sync.
pub(crate) const PERSONAL_FIELDS: [&str; 8] = [
    "status",
    "rating",
    "favorite",
    "hidden",
    "tags",
    "notes",
    "description",
    "board_rank",
];

/// `games` is the loaded library, which leaves out archived games, so an
/// archive state does not sync yet.
pub fn project(
    definitions: &LibraryDefinitions,
    games: &[GameRevision],
    settings: &Settings,
) -> Projection {
    let mut fields = Projection::default();
    project_definitions(definitions, &mut fields);
    let mut seen_apps = BTreeSet::new();
    let mut games: Vec<&GameRevision> = games.iter().collect();
    games.sort_by_key(|game| game.game_id);
    for game in games {
        // One local record for each Steam game; a duplicate is not synced.
        // Manual games do not sync yet: no feature creates them.
        let target = match &game.game.steam {
            Some(steam) if seen_apps.insert(steam.app_id) => Target::Steam(steam.app_id),
            _ => continue,
        };
        project_game(game, &target, &definitions.default_status, &mut fields);
    }
    project_settings(settings, &mut fields);
    fields
}

fn set(fields: &mut Projection, target: &Target, field: &str, value: Value) {
    if !fields.targets.contains(target) {
        fields.targets.insert(target.clone());
    }
    if !value.is_null() {
        fields
            .fields
            .insert((target.clone(), field.to_owned()), value);
    }
}

/// Extension properties with names that are valid sync field names. Other
/// names come from no GameSync version and stay on this device.
fn project_extra(fields: &mut Projection, target: &Target, prefix: &str, extra: &ExtraFields) {
    for (key, value) in extra {
        if plain_name(key) {
            set(fields, target, &format!("{prefix}{key}"), value.clone());
        }
    }
}

pub(crate) fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// The name that app storage gives a new library.
pub(crate) const DEFAULT_LIBRARY_NAME: &str = "My games";

/// Definitions of a new library. Values equal to these count as unset, so a
/// new device takes the statuses and order of another device at its first
/// sync without a review.
pub(crate) fn default_definitions() -> LibraryDefinitions {
    LibraryDefinitions::new(DEFAULT_LIBRARY_NAME)
}

fn project_definitions(definitions: &LibraryDefinitions, fields: &mut Projection) {
    let defaults = default_definitions();
    let library = Target::Library;
    let name = (definitions.name != defaults.name).then_some(&definitions.name);
    set(fields, &library, "name", json!(name));
    let default_status = (definitions.default_status != defaults.default_status)
        .then_some(&definitions.default_status);
    set(fields, &library, "default_status", json!(default_status));
    let keys = |d: &LibraryDefinitions| -> Vec<String> {
        d.statuses.iter().map(|s| s.key.clone()).collect()
    };
    let order = keys(definitions);
    if order != keys(&defaults) {
        set(fields, &library, "status_order", json!(order));
    }
    if !definitions.collections.is_empty() {
        let order: Vec<_> = definitions.collections.iter().map(|c| c.id).collect();
        set(fields, &library, "collection_order", json!(order));
    }
    project_extra(fields, &library, "", &definitions.extra);
    for status in &definitions.statuses {
        let target = Target::Status(status.key.clone());
        let default = defaults.status(&status.key);
        let label = (default.map(|d| &d.label) != Some(&status.label)).then_some(&status.label);
        set(fields, &target, "label", json!(label));
        let eligible = default.is_some_and(|d| d.recommendation_eligible);
        if status.recommendation_eligible != eligible {
            set(
                fields,
                &target,
                "recommendation_eligible",
                json!(status.recommendation_eligible),
            );
        }
        project_extra(fields, &target, "", &status.extra);
    }
    for collection in &definitions.collections {
        let target = Target::Collection(collection.id);
        set(fields, &target, "name", json!(collection.name));
        if collection.archived {
            set(fields, &target, "archived", json!(true));
        }
        project_extra(fields, &target, "", &collection.extra);
    }
}

fn project_game(
    game: &GameRevision,
    target: &Target,
    default_status: &str,
    fields: &mut Projection,
) {
    let personal = &game.game.personal;
    if personal.status != default_status {
        set(fields, target, "personal.status", json!(personal.status));
    }
    set(fields, target, "personal.rating", json!(personal.rating));
    if personal.favorite {
        set(fields, target, "personal.favorite", json!(true));
    }
    if personal.hidden {
        set(fields, target, "personal.hidden", json!(true));
    }
    if !personal.tags.is_empty() {
        set(fields, target, "personal.tags", json!(personal.tags));
    }
    if !personal.notes.is_empty() {
        set(fields, target, "personal.notes", json!(personal.notes));
    }
    set(
        fields,
        target,
        "personal.description",
        json!(personal.description),
    );
    set(
        fields,
        target,
        "personal.board_rank",
        json!(personal.board_rank),
    );
    for id in &personal.collections {
        set(
            fields,
            target,
            &format!("personal.collections.{id}"),
            json!(true),
        );
    }
    project_extra(fields, target, "personal.", &personal.extra);
}

/// Shared settings only. See `plan/data-sync.md` for the device-local list.
fn project_settings(settings: &Settings, fields: &mut Projection) {
    let target = Target::Settings;
    if settings.reduce_motion {
        set(fields, &target, "reduce_motion", json!(true));
    }
    if settings.show_hidden_games {
        set(fields, &target, "show_hidden_games", json!(true));
    }
    if settings.library_display != LibraryDisplay::default() {
        set(
            fields,
            &target,
            "library_display",
            json!(settings.library_display),
        );
    }
    for (section, view) in &settings.section_views {
        set(
            fields,
            &target,
            &format!("section_views.{}", hex(section)),
            json!(view),
        );
    }
    if !settings.smart_groups_open.is_empty() {
        set(
            fields,
            &target,
            "smart_groups_open",
            json!(settings.smart_groups_open),
        );
    }
    set(
        fields,
        &target,
        "store_country",
        json!(settings.store_country),
    );
    if settings.best_on_rules != crate::suitability::Rules::default() {
        set(
            fields,
            &target,
            "best_on_rules",
            json!(settings.best_on_rules),
        );
    }
}

/// Section keys hold any text, such as a smart rule in JSON. Hex keeps them
/// inside the sync field name alphabet.
pub(crate) fn hex(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex(text: &str) -> Option<String> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let bytes = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_names_round_trip_through_field_names() {
        let section = r#"smart:{"kind":"my_tag","value":"Wochenende ☕"}"#;
        let field = hex(section);
        assert!(field.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(unhex(&field).as_deref(), Some(section));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }

    #[test]
    fn a_new_library_has_only_unset_definitions() {
        let fields = project(&default_definitions(), &[], &Settings::default());
        assert!(fields.fields.is_empty(), "{:?}", fields.fields);
        assert!(fields.targets.contains(&Target::Library));
        assert!(fields.targets.contains(&Target::Status("backlog".into())));
    }

    #[test]
    fn best_on_rules_sync_but_protondb_stays_on_this_device() {
        let mut settings = Settings::default();
        settings.best_on_rules.prefer_pc = vec!["Open World".into()];
        settings.protondb = true;
        let fields = project(&default_definitions(), &[], &settings).fields;
        let names: Vec<_> = fields
            .keys()
            .filter(|key| key.0 == Target::Settings)
            .map(|key| key.1.as_str())
            .collect();
        assert_eq!(names, ["best_on_rules"]);
    }
}
