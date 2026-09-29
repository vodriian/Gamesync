//! The merged view of all changes known to one device.
//!
//! The result depends only on the set of changes, not on the order in which
//! they arrive, so every device that has the same changes shows the same state.

use super::{Change, Clock, Target};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// One synced field: a target and a dotted field path.
pub type FieldKey = (Target, String);

#[derive(Debug, PartialEq)]
pub enum FieldState<'a> {
    /// The field value. While a replaced change has not arrived yet, this is
    /// the newest candidate and not a conflict: the missing change usually
    /// shows that one candidate replaced the others. If it never arrives, the
    /// newest candidate stays the value.
    Value(&'a Change),
    /// Edits that did not see each other, oldest first. The user must choose.
    Conflict(Vec<&'a Change>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Insert {
    Added,
    /// The same change was already known. Cloud folders can deliver a file twice.
    Known,
    /// A different change with the same ID was already known. Every device
    /// keeps the change with the lower JSON text, so all devices agree. This
    /// is not caused by normal use; report it.
    Clash,
}

pub struct ChangeSet {
    clock: Clock,
    changes: HashMap<Uuid, Change>,
    fields: BTreeMap<FieldKey, Vec<Uuid>>,
}

impl ChangeSet {
    pub fn new(device: Uuid) -> Self {
        Self {
            clock: Clock::new(device),
            changes: HashMap::new(),
            fields: BTreeMap::new(),
        }
    }

    /// Add a change from this or another device. The caller validates changes
    /// from files first; see `Change::validate`.
    pub fn insert(&mut self, change: Change) -> Insert {
        self.clock.observe(change.at);
        let Some(known) = self.changes.get(&change.id) else {
            self.add(change);
            return Insert::Added;
        };
        if *known == change {
            return Insert::Known;
        }
        if json_text(&change) < json_text(known) {
            let old = self.remove(change.id);
            debug_assert!(old.is_some());
            self.add(change);
        }
        Insert::Clash
    }

    /// Record a local edit. It replaces every current candidate of the field,
    /// so an edit to a field in conflict also resolves the conflict.
    pub fn edit(&mut self, wall_ms: u64, target: Target, field: &str, value: Value) -> Change {
        let key = (target, field.to_owned());
        let base = self
            .fields
            .get(&key)
            .map(|ids| self.heads(ids).into_iter().map(|c| c.id).collect())
            .unwrap_or_default();
        let (target, field) = key;
        let change = Change {
            id: Uuid::new_v4(),
            at: self.clock.tick(wall_ms),
            target,
            field,
            value,
            base,
            extra: BTreeMap::new(),
        };
        self.add(change.clone());
        change
    }

    pub fn state(&self, target: &Target, field: &str) -> Option<FieldState<'_>> {
        let ids = self.fields.get(&(target.clone(), field.to_owned()))?;
        Some(self.state_of(ids))
    }

    pub fn fields(&self) -> impl Iterator<Item = (&FieldKey, FieldState<'_>)> {
        self.fields
            .iter()
            .map(|(key, ids)| (key, self.state_of(ids)))
    }

    pub fn conflicts(&self) -> impl Iterator<Item = (&FieldKey, Vec<&Change>)> {
        self.fields().filter_map(|(key, state)| match state {
            FieldState::Conflict(heads) => Some((key, heads)),
            FieldState::Value(_) => None,
        })
    }

    pub fn changes(&self) -> impl Iterator<Item = &Change> {
        self.changes.values()
    }

    fn add(&mut self, change: Change) {
        self.fields
            .entry((change.target.clone(), change.field.clone()))
            .or_default()
            .push(change.id);
        self.changes.insert(change.id, change);
    }

    fn remove(&mut self, id: Uuid) -> Option<Change> {
        let change = self.changes.remove(&id)?;
        let key = (change.target.clone(), change.field.clone());
        if let Some(ids) = self.fields.get_mut(&key) {
            ids.retain(|known| *known != id);
            if ids.is_empty() {
                self.fields.remove(&key);
            }
        }
        Some(change)
    }

    /// Changes of one field that no other known change replaced, oldest first.
    /// A cycle of bases, which only a faulty writer can make, returns all its
    /// changes, so the user reviews them.
    fn heads(&self, ids: &[Uuid]) -> Vec<&Change> {
        let changes: Vec<&Change> = ids.iter().map(|id| &self.changes[id]).collect();
        let replaced: HashSet<Uuid> = changes
            .iter()
            .flat_map(|c| c.base.iter().copied())
            .collect();
        let mut heads: Vec<&Change> = changes
            .iter()
            .copied()
            .filter(|c| !replaced.contains(&c.id))
            .collect();
        if heads.is_empty() {
            heads = changes;
        }
        heads.sort_by_key(|c| (c.at, c.id));
        heads
    }

    fn state_of(&self, ids: &[Uuid]) -> FieldState<'_> {
        let mut heads = self.heads(ids);
        let known: HashSet<&Uuid> = ids.iter().collect();
        let missing = ids
            .iter()
            .flat_map(|id| &self.changes[id].base)
            .any(|base| !known.contains(base));
        if heads.len() == 1 || missing {
            FieldState::Value(heads.pop().expect("a field has at least one change"))
        } else {
            FieldState::Conflict(heads)
        }
    }
}

fn json_text(change: &Change) -> String {
    serde_json::to_string(change).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::Stamp;
    use serde_json::json;

    const MAC: Uuid = Uuid::from_u128(1);
    const LINUX: Uuid = Uuid::from_u128(2);
    const RATING: &str = "personal.rating";

    fn game() -> Target {
        Target::Steam(620)
    }

    fn receive(to: &mut ChangeSet, from: &ChangeSet) {
        for change in from.changes() {
            to.insert(change.clone());
        }
    }

    fn value(set: &ChangeSet) -> Value {
        match set.state(&game(), RATING) {
            Some(FieldState::Value(change)) => change.value.clone(),
            other => panic!("expected a value, found {other:?}"),
        }
    }

    fn summary(set: &ChangeSet) -> Vec<(FieldKey, Vec<Uuid>)> {
        set.fields()
            .map(|(key, state)| {
                let ids = match state {
                    FieldState::Value(change) => vec![change.id],
                    FieldState::Conflict(heads) => heads.iter().map(|c| c.id).collect(),
                };
                (key.clone(), ids)
            })
            .collect()
    }

    #[test]
    fn an_edit_after_seeing_another_edit_applies_without_review() {
        let mut mac = ChangeSet::new(MAC);
        let mut linux = ChangeSet::new(LINUX);
        mac.edit(1_000, game(), RATING, json!(6));
        receive(&mut linux, &mac);
        // Linux's wall clock is behind; the edit still replaces the Mac value.
        linux.edit(10, game(), RATING, json!(8));
        receive(&mut mac, &linux);
        assert_eq!(value(&mac), json!(8));
        assert_eq!(value(&linux), json!(8));
        assert_eq!(mac.conflicts().count(), 0);
    }

    #[test]
    fn edits_that_did_not_see_each_other_need_review_on_both_devices() {
        let mut mac = ChangeSet::new(MAC);
        let mut linux = ChangeSet::new(LINUX);
        mac.edit(1_000, game(), RATING, json!(6));
        linux.edit(2_000, game(), RATING, json!(8));
        receive(&mut mac, &linux);
        receive(&mut linux, &mac);
        for set in [&mac, &linux] {
            let conflicts: Vec<_> = set.conflicts().collect();
            assert_eq!(conflicts.len(), 1);
            let values: Vec<_> = conflicts[0].1.iter().map(|c| &c.value).collect();
            assert_eq!(values, [&json!(6), &json!(8)]);
        }
    }

    #[test]
    fn a_resolution_on_one_device_clears_the_conflict_on_the_other() {
        let mut mac = ChangeSet::new(MAC);
        let mut linux = ChangeSet::new(LINUX);
        mac.edit(1_000, game(), RATING, json!(6));
        linux.edit(2_000, game(), RATING, json!(8));
        receive(&mut mac, &linux);
        mac.edit(3_000, game(), RATING, json!(6));
        receive(&mut linux, &mac);
        assert_eq!(value(&linux), json!(6));
        assert_eq!(linux.conflicts().count(), 0);
    }

    #[test]
    fn the_result_does_not_depend_on_arrival_order() {
        let mut mac = ChangeSet::new(MAC);
        let mut linux = ChangeSet::new(LINUX);
        mac.edit(1_000, game(), RATING, json!(4));
        receive(&mut linux, &mac);
        linux.edit(1_500, game(), RATING, json!(6));
        mac.edit(2_000, game(), RATING, json!(9));
        mac.edit(2_100, Target::Settings, "reduce_motion", json!(true));
        linux.edit(2_200, Target::Settings, "theme", json!("flexoki"));
        let mut all: Vec<Change> = mac.changes().chain(linux.changes()).cloned().collect();
        all.sort_by_key(|c| c.id);
        all.dedup_by_key(|c| c.id);
        assert_eq!(all.len(), 5);

        let mut expected = None;
        permute(&mut all, 0, &mut |order| {
            let mut set = ChangeSet::new(Uuid::from_u128(3));
            for change in order {
                assert_eq!(set.insert(change.clone()), Insert::Added);
            }
            let found = summary(&set);
            assert_eq!(expected.get_or_insert_with(|| found.clone()), &found);
        });
        // 4 was replaced by both 6 and 9, which did not see each other.
        let conflicts = expected.unwrap();
        assert_eq!(conflicts.iter().filter(|(_, ids)| ids.len() > 1).count(), 1);
    }

    fn permute(items: &mut [Change], start: usize, visit: &mut impl FnMut(&[Change])) {
        if start == items.len() {
            return visit(items);
        }
        for i in start..items.len() {
            items.swap(start, i);
            permute(items, start + 1, visit);
            items.swap(start, i);
        }
    }

    #[test]
    fn a_repeated_change_has_no_effect() {
        let mut set = ChangeSet::new(MAC);
        let change = set.edit(1_000, game(), RATING, json!(6));
        assert_eq!(set.insert(change), Insert::Known);
        assert_eq!(set.changes().count(), 1);
    }

    #[test]
    fn a_clashing_id_keeps_the_same_change_in_any_order() {
        let mut source = ChangeSet::new(MAC);
        let first = source.edit(1_000, game(), RATING, json!(6));
        let mut second = first.clone();
        second.target = Target::Steam(440);
        second.value = json!(2);

        let mut one = ChangeSet::new(LINUX);
        assert_eq!(one.insert(first.clone()), Insert::Added);
        assert_eq!(one.insert(second.clone()), Insert::Clash);
        let mut two = ChangeSet::new(LINUX);
        assert_eq!(two.insert(second), Insert::Added);
        assert_eq!(two.insert(first), Insert::Clash);
        assert_eq!(summary(&one), summary(&two));
        assert_eq!(one.changes().count(), 1);
        assert_eq!(one.fields().count(), 1);
    }

    #[test]
    fn a_missing_replaced_change_does_not_open_a_review() {
        let mut mac = ChangeSet::new(MAC);
        let mut linux = ChangeSet::new(LINUX);
        let first = mac.edit(1_000, game(), RATING, json!(4));
        receive(&mut linux, &mac);
        let middle = linux.edit(2_000, game(), RATING, json!(6));
        mac.insert(middle.clone());
        let last = mac.edit(3_000, game(), RATING, json!(9));

        // A third device has the first and last change; the middle is late.
        let mut late = ChangeSet::new(Uuid::from_u128(3));
        late.insert(first);
        late.insert(last);
        assert_eq!(value(&late), json!(9));
        late.insert(middle);
        assert_eq!(value(&late), json!(9));
        assert_eq!(late.conflicts().count(), 0);
    }

    #[test]
    fn null_clears_a_value() {
        let mut set = ChangeSet::new(MAC);
        set.edit(1_000, game(), RATING, json!(6));
        set.edit(2_000, game(), RATING, json!(null));
        assert_eq!(value(&set), json!(null));
    }

    #[test]
    fn a_cycle_of_bases_opens_a_review() {
        let at = |ms| Stamp {
            ms,
            n: 0,
            device: MAC,
        };
        let (a, b) = (Uuid::from_u128(10), Uuid::from_u128(11));
        let change = |id, base, ms, value| Change {
            id,
            at: at(ms),
            target: game(),
            field: RATING.into(),
            value: json!(value),
            base: vec![base],
            extra: BTreeMap::new(),
        };
        let mut set = ChangeSet::new(LINUX);
        set.insert(change(a, b, 1, 3));
        set.insert(change(b, a, 2, 5));
        assert_eq!(
            set.conflicts().next().map(|(_, heads)| heads.len()),
            Some(2)
        );
    }

    #[test]
    fn local_edits_sort_after_received_changes() {
        let mut mac = ChangeSet::new(MAC);
        let mut linux = ChangeSet::new(LINUX);
        let remote = mac.edit(9_000_000, game(), RATING, json!(6));
        receive(&mut linux, &mac);
        let local = linux.edit(1_000, Target::Settings, "theme", json!("x"));
        assert!(local.at > remote.at);
    }
}
