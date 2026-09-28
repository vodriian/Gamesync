//! One write path for library definitions. Callers own their editor state.
use crate::model::Library;
use gamesync_desktop::library::{LibraryRevision, LibraryStore};
use gpui::{App, AppContext as _, Entity, Task};
use std::path::PathBuf;

/// Save reviewed definitions against their base revision, then apply them to
/// the shared model, so every view shows the same statuses and collections.
pub fn save(
    library: &Entity<Library>,
    root: PathBuf,
    base: LibraryRevision,
    cx: &mut App,
) -> Task<Result<(), String>> {
    if let Err(error) = base.definitions.validate() {
        return Task::ready(Err(error.to_string()));
    }
    let library = library.clone();
    cx.spawn(async move |cx| {
        let path = root.clone();
        let saved = cx
            .background_spawn(async move {
                LibraryStore::open(path)?.edit(base.revision_id, base.definitions)
            })
            .await
            .map_err(|error| format!("Not saved: {error}. Cancel and try again after refresh."))?;
        library
            .update(cx, |lib, cx| {
                // A library opened meanwhile keeps its own definitions.
                if lib.source.as_ref().is_some_and(|(path, _)| path == &root) {
                    lib.apply_definitions(root, saved);
                    cx.notify();
                }
            })
            .map_err(|error| error.to_string())
    })
}
