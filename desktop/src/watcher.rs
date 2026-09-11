//! Eagle's watcher pattern adapted to library folders and a bounded event queue.
use futures::channel::mpsc::Sender;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;

pub fn watch(
    path: &Path,
    mut events: Sender<Result<(), String>>,
) -> anyhow::Result<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        let message = match result {
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
            Ok(event)
                if !event.paths.is_empty()
                    && event.paths.iter().all(|path| {
                        path.extension().is_some_and(|ext| ext == "tmp")
                            || path.file_name().is_some_and(|name| name == ".writer.lock")
                    }) =>
            {
                return
            }
            Ok(_) => Ok(()),
            Err(error) => Err(format!(
                "Live updates failed: {error}. Retrying with periodic scans."
            )),
        };
        // Coalesce bursts instead of storing an unbounded list of file events.
        let _ = events.try_send(message);
    })?;
    watcher.watch(path, RecursiveMode::Recursive)?;
    Ok(watcher)
}
