//! Independent guarded writes: one stale game must not discard other successful edits.
use crate::{
    library::{LibraryRevision, LibraryStore},
    records::{GameRevision, PersonalData},
};
use uuid::Uuid;

#[derive(Clone)]
pub enum Change {
    Favorite(bool),
    Status(String),
    Collection(Uuid, bool),
    Hidden(bool),
}
impl Change {
    pub fn apply(&self, personal: &mut PersonalData) {
        match self {
            Self::Favorite(value) => personal.favorite = *value,
            Self::Status(value) => personal.status = value.clone(),
            Self::Hidden(value) => personal.hidden = *value,
            Self::Collection(id, add) => {
                if *add {
                    if !personal.collections.contains(id) {
                        personal.collections.push(*id);
                    }
                } else {
                    personal.collections.retain(|existing| existing != id);
                }
            }
        }
    }
}

pub struct Outcome {
    pub saved: Vec<GameRevision>,
    pub failed: Vec<(Uuid, String)>,
}

pub fn apply(
    store: &LibraryStore,
    manifest: &LibraryRevision,
    targets: Vec<GameRevision>,
    change: &Change,
) -> Outcome {
    let mut outcome = Outcome {
        saved: Vec::new(),
        failed: Vec::new(),
    };
    for base in targets {
        let mut personal = base.game.personal.clone();
        change.apply(&mut personal);
        // Even unchanged values go through the revision guard, so stale snapshots
        // cannot be reported as successful or copied back into the visible model.
        match store.edit_game_personal(manifest, base.game_id, base.revision_id, personal) {
            Ok(record) => outcome.saved.push(record),
            Err(error) => outcome.failed.push((base.game_id, error.to_string())),
        }
    }
    outcome
}
