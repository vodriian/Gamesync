//! Publish a complete revision before replacing a current JSON record.
//!
//! This is the file layer, not a conflict resolver. The caller must validate the
//! schema, choose safe library paths, and check revision parents before saving.
//! Directory creation and library reconciliation belong to the next layer.

use std::{fs, io::Write, path::Path};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tempfile::{Builder, NamedTempFile};

/// Save a typed record in existing directories on macOS or Linux.
///
/// `revision` must be a new, immutable history path. An exact retry is allowed;
/// different bytes at that path cause an error. Both files contain the same JSON.
/// If current-file publication fails, the revision remains available for recovery.
/// Call from a worker. Success means local file publication, not cloud upload.
///
/// This function does not lock records or reject stale edits. A library service
/// must do that before exposing this operation to the editor.
pub fn save_record<T: Serialize>(current: &Path, revision: &Path, record: &T) -> Result<()> {
    if !cfg!(unix) {
        bail!("Durable file writes are not implemented for this platform");
    }
    if current == revision {
        bail!("Current and revision paths must differ");
    }

    // Finish serialization before any disk change. A bad value must not truncate
    // either the previous record or its recovery copy.
    let mut bytes = serde_json::to_vec_pretty(record).context("Could not encode record")?;
    bytes.push(b'\n');
    publish_revision(revision, &bytes)?;

    replace_current(current, &bytes).with_context(|| {
        format!(
            "Could not confirm current record at {}. Revision retained at {}",
            current.display(),
            revision.display()
        )
    })
}

fn publish_revision(path: &Path, bytes: &[u8]) -> Result<()> {
    let staged = stage(path, bytes)?;
    match staged.persist_noclobber(path) {
        Ok(_) => {}
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let previous = fs::read(path)
                .with_context(|| format!("Could not read revision at {}", path.display()))?;
            if previous != bytes {
                bail!(
                    "Revision already exists with different content: {}",
                    path.display()
                );
            }
            // A retry can follow a failed directory sync. Flush again before
            // allowing the current record to reference this revision.
            fs::File::open(path)?.sync_all()?;
        }
        Err(error) => return Err(error.error).context("Could not publish revision"),
    }
    sync_parent(path).context("Could not confirm revision directory")
}

fn replace_current(path: &Path, bytes: &[u8]) -> Result<()> {
    stage(path, bytes)?
        .persist(path)
        .map_err(|error| error.error)
        .context("Could not replace current file")?;
    sync_parent(path)
}

fn stage(path: &Path, bytes: &[u8]) -> Result<NamedTempFile> {
    // A sibling temporary file keeps rename on the same filesystem. Incomplete
    // .tmp files are never records and must be ignored by future folder scans.
    let mut file = Builder::new()
        .prefix(".gamesync-")
        .suffix(".tmp")
        .tempfile_in(parent(path)?)
        .with_context(|| format!("Could not stage file beside {}", path.display()))?;
    file.write_all(bytes)
        .context("Could not write staged record")?;
    file.as_file()
        .sync_all()
        .context("Could not flush staged record")?;
    Ok(file)
}

fn parent(path: &Path) -> Result<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .context("Record path must include its library directory")
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<()> {
    // Sync the directory entry as well as the data before reporting success.
    // Hardware and cloud clients can add their own persistence limits.
    fs::File::open(parent(path)?)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent(_: &Path) -> Result<()> {
    bail!("Directory sync is not implemented for this platform")
}
