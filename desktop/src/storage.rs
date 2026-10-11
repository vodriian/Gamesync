//! Publish a complete revision before replacing a current JSON record.
//!
//! This is the file layer, not a conflict resolver. The caller must validate the
//! schema, choose safe library paths, and check revision parents before saving.
//! Directory creation and library reconciliation belong to the next layer.

use std::{fs, io::Write, path::Path};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tempfile::{Builder, NamedTempFile};

/// Save a typed record in existing directories on macOS, Linux, or Windows.
///
/// `revision` must be a new, immutable history path. An exact retry is allowed;
/// different bytes at that path cause an error. Both files contain the same JSON.
/// If current-file publication fails, the revision remains available for recovery.
/// Call from a worker. Success means local file publication, not cloud upload.
///
/// This function does not lock records or reject stale edits. A library service
/// must do that before exposing this operation to the editor.
pub fn save_record<T: Serialize>(current: &Path, revision: &Path, record: &T) -> Result<()> {
    if !cfg!(any(unix, windows)) {
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

/// Publish an immutable file. An exact retry succeeds; different bytes fail.
pub(crate) fn publish_revision(path: &Path, bytes: &[u8]) -> Result<()> {
    let staged = stage(path, bytes)?;
    match publish(staged, path, false) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
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
            sync_file(path)?;
        }
        Err(error) => return Err(error).context("Could not publish revision"),
    }
    sync_parent(path).context("Could not confirm revision directory")
}

/// Retain an imported branch without changing the current record.
pub(crate) fn retain_revision<T: Serialize>(path: &Path, record: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(record)?;
    bytes.push(b'\n');
    publish_revision(path, &bytes)
}

/// Atomically replace a file that has one writer.
pub(crate) fn replace_current(path: &Path, bytes: &[u8]) -> Result<()> {
    publish(stage(path, bytes)?, path, true).context("Could not replace current file")?;
    sync_parent(path)
}

#[cfg(not(windows))]
pub(crate) fn publish(file: NamedTempFile, path: &Path, replace: bool) -> std::io::Result<()> {
    let result = if replace {
        file.persist(path)
    } else {
        file.persist_noclobber(path)
    };
    result.map(|_| ()).map_err(|error| error.error)
}

#[cfg(windows)]
pub(crate) fn publish(file: NamedTempFile, path: &Path, replace: bool) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    // Close the staged handle before rename. TempPath removes it on failure.
    // Both paths are on the same volume; immutable history never uses replace.
    let staged = file.into_temp_path();
    let wide = |path: &Path| -> std::io::Result<Vec<u16>> {
        let mut value: Vec<_> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path contains NUL",
            ));
        }
        value.push(0);
        Ok(value)
    };
    let source = wide(&staged)?;
    let destination = wide(path)?;
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    for attempt in 0..8 {
        // SAFETY: Both pointers refer to live, NUL-terminated UTF-16 paths.
        if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } != 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        // Readers and virus scanners can briefly prevent a Windows rename.
        // Bound retries to 140 ms; a lasting failure keeps revision recovery.
        if attempt == 7 || !matches!(error.raw_os_error(), Some(5 | 32 | 33)) {
            return Err(error);
        }
        std::thread::sleep(std::time::Duration::from_millis(5 * (attempt + 1)));
    }
    unreachable!("Every final publication attempt returns")
}

pub(crate) fn sync_file(path: &Path) -> Result<()> {
    // FlushFileBuffers requires write access on Windows. Do not truncate history.
    fs::OpenOptions::new()
        .read(true)
        .write(cfg!(windows))
        .open(path)?
        .sync_all()?;
    Ok(())
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
pub(crate) fn sync_parent(path: &Path) -> Result<()> {
    // Sync the directory entry as well as the data before reporting success.
    // Hardware and cloud clients can add their own persistence limits.
    fs::File::open(parent(path)?)?.sync_all()?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn sync_parent(_: &Path) -> Result<()> {
    // Windows publication uses MoveFileExW with WRITE_THROUGH after flushing
    // file data. It has no supported equivalent of Unix directory fsync;
    // power-loss durability still depends on the filesystem and device.
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn sync_parent(_: &Path) -> Result<()> {
    bail!("Directory sync is not implemented for this platform")
}
