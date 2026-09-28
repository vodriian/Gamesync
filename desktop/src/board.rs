//! Board order and status definition edits. No UI or disk access.
//!
//! A board rank is a fractional index: base-36 digits compared as plain text,
//! read as the fraction `0.<digits>`. A rank always fits between two neighbors,
//! so a move writes only the moved game and never renumbers a column.

use crate::library::{LibraryDefinitions, StatusDefinition};
use anyhow::{bail, ensure, Result};

const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
/// Repeated inserts at one place add about one digit per five moves.
pub const MAX_RANK_LEN: usize = 256;
pub const MAX_STATUS_LABEL_LEN: usize = 64;

/// Ranks from other writers are not trusted. An invalid rank counts as no rank.
pub fn valid_rank(rank: &str) -> bool {
    !rank.is_empty()
        && rank.len() <= MAX_RANK_LEN
        && !rank.ends_with('0')
        && rank
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

/// A rank strictly between `before` and `after`. `None` means the column start
/// or end. Returns `None` for invalid or misordered bounds, or a rank too long.
pub fn rank_between(before: Option<&str>, after: Option<&str>) -> Option<String> {
    let before = before.unwrap_or("");
    if !before.is_empty() && !valid_rank(before) {
        return None;
    }
    if let Some(after) = after {
        if !valid_rank(after) || before >= after {
            return None;
        }
    }
    let rank = midpoint(before.as_bytes(), after.map(str::as_bytes));
    (rank.len() <= MAX_RANK_LEN).then(|| String::from_utf8(rank).ok())?
}

/// Requires `a < b`, valid digits, and no trailing zero. The caller checks this.
fn midpoint(a: &[u8], b: Option<&[u8]>) -> Vec<u8> {
    if let Some(b) = b {
        // Keep the shared prefix; a missing digit in `a` reads as zero.
        let shared = b
            .iter()
            .enumerate()
            .take_while(|(i, d)| a.get(*i).copied().unwrap_or(b'0') == **d)
            .count();
        if shared > 0 {
            let mut out = b[..shared].to_vec();
            out.extend(midpoint(a.get(shared..).unwrap_or(&[]), Some(&b[shared..])));
            return out;
        }
    }
    let digit = |d: u8| DIGITS.iter().position(|x| *x == d).unwrap_or(0);
    let low = a.first().map_or(0, |d| digit(*d));
    let high = b
        .and_then(|b| b.first())
        .map_or(DIGITS.len(), |d| digit(*d));
    if high - low > 1 {
        return vec![DIGITS[(low + high).div_ceil(2)]];
    }
    // Adjacent digits: the shorter upper bound, or extend the lower bound.
    if let Some(b) = b.filter(|b| b.len() > 1) {
        return vec![b[0]];
    }
    let mut out = vec![DIGITS[low]];
    out.extend(midpoint(a.get(1..).unwrap_or(&[]), None));
    out
}

/// Rank for a drop in one column. `ranked` holds the column's valid ranks in
/// display order, without the moved game. `before` is the index of the drop
/// target in `ranked`; `None` places the game after the last ranked game.
pub fn rank_for_drop(ranked: &[&str], before: Option<usize>) -> Option<String> {
    match before.filter(|&i| i < ranked.len()) {
        None => rank_between(ranked.last().copied(), None),
        Some(i) => {
            let after = ranked[i];
            // Synced edits can produce equal ranks. Skip them to keep a valid gap.
            let lower = ranked[..i].iter().rev().find(|r| **r < after).copied();
            rank_between(lower, Some(after))
        }
    }
}

/// A stable key from the label: lowercase ASCII words joined by `-`.
/// Labels without ASCII letters or digits get `status`. Existing keys get a suffix.
pub fn status_key(label: &str, taken: impl Fn(&str) -> bool) -> String {
    let mut base = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            base.push(c.to_ascii_lowercase());
        } else if !base.is_empty() && !base.ends_with('-') {
            base.push('-');
        }
    }
    base.truncate(48);
    let base = base.trim_end_matches('-');
    let base = if base.is_empty() { "status" } else { base };
    if !taken(base) {
        return base.into();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|key| !taken(key))
        .unwrap_or_else(|| base.into())
}

fn clean_label(
    definitions: &LibraryDefinitions,
    label: &str,
    except: Option<&str>,
) -> Result<String> {
    let label = label.trim();
    ensure!(!label.is_empty(), "Enter a status name.");
    ensure!(
        label.len() <= MAX_STATUS_LABEL_LEN,
        "Choose a shorter status name."
    );
    ensure!(
        !definitions
            .statuses
            .iter()
            .any(|s| Some(s.key.as_str()) != except
                && s.label.trim().to_lowercase() == label.to_lowercase()),
        "A status has this name already."
    );
    Ok(label.into())
}

/// Append a status. New statuses are not recommendation eligible until the
/// user enables that; this keeps Choose results unchanged. Returns the new key.
pub fn add_status(definitions: &mut LibraryDefinitions, label: &str) -> Result<String> {
    let label = clean_label(definitions, label, None)?;
    let key = status_key(&label, |key| definitions.status(key).is_some());
    definitions.statuses.push(StatusDefinition {
        key: key.clone(),
        label,
        recommendation_eligible: false,
        extra: Default::default(),
    });
    Ok(key)
}

/// Change a label. The key stays, so game files do not change.
pub fn rename_status(definitions: &mut LibraryDefinitions, key: &str, label: &str) -> Result<()> {
    let label = clean_label(definitions, label, Some(key))?;
    let Some(status) = definitions.statuses.iter_mut().find(|s| s.key == key) else {
        bail!("This status is no longer available.");
    };
    status.label = label;
    Ok(())
}

/// Move a status one place. `false` means it is already at that edge.
pub fn move_status(definitions: &mut LibraryDefinitions, key: &str, later: bool) -> bool {
    let Some(i) = definitions.statuses.iter().position(|s| s.key == key) else {
        return false;
    };
    let j = if later { i + 1 } else { i.wrapping_sub(1) };
    if j >= definitions.statuses.len() {
        return false;
    }
    definitions.statuses.swap(i, j);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_fit_between_neighbors_without_renumbering() {
        let first = rank_between(None, None).unwrap();
        let mut ranks = vec![first];
        // Append, prepend, and repeatedly insert at one place.
        for _ in 0..40 {
            let end = rank_between(ranks.last().map(String::as_str), None).unwrap();
            ranks.push(end);
            let start = rank_between(None, Some(&ranks[0])).unwrap();
            ranks.insert(0, start);
            let mid = rank_between(Some(&ranks[1]), Some(&ranks[2])).unwrap();
            ranks.insert(2, mid);
        }
        assert!(ranks.windows(2).all(|w| w[0] < w[1]));
        assert!(ranks.iter().all(|r| valid_rank(r)));
        assert!(ranks.iter().all(|r| r.len() < 32));
    }

    #[test]
    fn invalid_or_misordered_bounds_are_rejected() {
        assert_eq!(rank_between(Some("b"), Some("a")), None);
        assert_eq!(rank_between(Some("a"), Some("a")), None);
        assert_eq!(rank_between(Some("A"), None), None);
        assert_eq!(rank_between(Some("a0"), None), None);
        assert_eq!(rank_between(None, Some("")), None);
        assert_eq!(rank_between(None, Some("1")).as_deref(), Some("0i"));
        let long = "z".repeat(MAX_RANK_LEN);
        assert_eq!(rank_between(Some(&long), None), None);
    }

    #[test]
    fn drops_insert_before_target_or_after_ranked_games() {
        let ranked = ["b", "d", "d", "x"];
        let end = rank_for_drop(&ranked, None).unwrap();
        assert!(end.as_str() > "x");
        let before_first = rank_for_drop(&ranked, Some(0)).unwrap();
        assert!(before_first.as_str() < "b");
        // Tied ranks skip back to the last smaller rank.
        let before_tie = rank_for_drop(&ranked, Some(2)).unwrap();
        assert!("b" < before_tie.as_str() && before_tie.as_str() < "d");
        assert_eq!(rank_for_drop(&[], None).as_deref(), Some("i"));
        assert_eq!(rank_for_drop(&[], Some(3)).as_deref(), Some("i"));
    }

    #[test]
    fn rank_is_optional_in_records_and_resets_with_status() {
        let mut personal = crate::records::PersonalData::default();
        let json = serde_json::to_value(&personal).unwrap();
        assert!(json.get("board_rank").is_none());
        personal.board_rank = Some("i".into());
        let json = serde_json::to_value(&personal).unwrap();
        let back: crate::records::PersonalData = serde_json::from_value(json).unwrap();
        assert_eq!(back.board_rank.as_deref(), Some("i"));
        personal.set_status("backlog".into());
        assert_eq!(personal.board_rank.as_deref(), Some("i"));
        personal.set_status("playing".into());
        assert_eq!(personal.board_rank, None);
    }

    #[test]
    fn status_keys_are_stable_ascii_and_unique() {
        let taken = |k: &str| ["playing", "status", "on-hold"].contains(&k);
        assert_eq!(status_key("On hold", |_| false), "on-hold");
        assert_eq!(status_key("On hold", taken), "on-hold-2");
        assert_eq!(status_key("  Playing!! ", taken), "playing-2");
        assert_eq!(status_key("Играю", taken), "status-2");
        assert_eq!(status_key("100% done", |_| false), "100-done");
        let long = status_key(&"a".repeat(200), |_| false);
        assert_eq!(long.len(), 48);
    }

    #[test]
    fn status_edits_validate_labels_and_keep_keys() {
        let mut defs = LibraryDefinitions::new("Test");
        assert!(add_status(&mut defs, "  ").is_err());
        assert!(add_status(&mut defs, "playing").is_err());
        let key = add_status(&mut defs, "On hold").unwrap();
        assert_eq!(defs.statuses.last().unwrap().key, key);
        assert!(!defs.statuses.last().unwrap().recommendation_eligible);
        defs.validate().unwrap();
        rename_status(&mut defs, &key, "Later").unwrap();
        // Renaming to its own label in another case is allowed.
        rename_status(&mut defs, &key, "LATER").unwrap();
        assert!(rename_status(&mut defs, &key, "Backlog").is_err());
        assert!(rename_status(&mut defs, "missing", "Name").is_err());
        assert_eq!(defs.status(&key).unwrap().label, "LATER");
        assert!(!move_status(&mut defs, &key, true));
        assert!(move_status(&mut defs, &key, false));
        assert_eq!(defs.statuses[defs.statuses.len() - 2].key, key);
        assert!(!move_status(&mut defs, "backlog", false));
        defs.validate().unwrap();
    }
}
