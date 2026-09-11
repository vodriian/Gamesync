//! Inspect revisions before each edit. Sync clients do not take our local lock;
//! a late remote branch is retained and becomes a conflict on the next scan.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{ensure, Context, Result};
use uuid::Uuid;

use crate::{records::SCHEMA_VERSION, storage};
use serde::{de::DeserializeOwned, Serialize};

const MAX_RECORD_BYTES: u64 = 1024 * 1024;

/// Valid records stay available even when a partial or newer file blocks edits.
#[derive(Debug)]
pub struct RevisionSnapshot<T> {
    pub revisions: BTreeMap<Uuid, T>,
    pub heads: BTreeSet<Uuid>,
    pub issues: Vec<String>,
}

impl<T> RevisionSnapshot<T> {
    pub fn current(&self) -> Option<&T> {
        if !self.issues.is_empty() || self.heads.len() != 1 {
            return None;
        }
        self.heads.first().and_then(|id| self.revisions.get(id))
    }

    pub fn has_conflict(&self) -> bool {
        self.heads.len() > 1
    }
}

/// Shared only by game records and library definitions, which need the same
/// publication and conflict checks. Domain editing stays in their services.
pub(crate) trait Revision: Serialize + DeserializeOwned + Clone + PartialEq {
    fn entity_id(&self) -> Uuid;
    fn revision_id(&self) -> Uuid;
    fn parents(&self) -> &[Uuid];
    fn set_revision(&mut self, id: Uuid, parents: Vec<Uuid>);
    fn validate(&self) -> Result<()>;
}

pub(crate) struct RevisionFiles {
    pub current: PathBuf,
    pub history: PathBuf,
    pub entity_id: Option<Uuid>,
}

impl RevisionFiles {
    /// Read canonical files and copies that retain the current filename prefix.
    /// Incomplete history blocks edits until the missing files arrive or recover.
    pub fn inspect<T: Revision>(&self) -> Result<RevisionSnapshot<T>> {
        self.inspect_candidates(None)
    }

    pub fn inspect_candidates<T: Revision>(
        &self,
        candidates: Option<&[PathBuf]>,
    ) -> Result<RevisionSnapshot<T>> {
        let mut paths = Vec::new();
        let history = self.history.clone();
        match fs::read_dir(&history) {
            Ok(entries) => {
                for entry in entries {
                    let path = entry?.path();
                    if is_json(&path) {
                        paths.push(path);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Could not read revision history"),
        }
        if let Some(candidates) = candidates {
            paths.extend_from_slice(candidates);
        } else {
            let prefix = self
                .current
                .file_stem()
                .context("Current file has no name")?
                .to_string_lossy();
            for entry in fs::read_dir(
                self.current
                    .parent()
                    .context("Current file has no directory")?,
            )? {
                let path = entry?.path();
                if is_json(&path)
                    && path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with(prefix.as_ref()))
                {
                    paths.push(path);
                }
            }
        }
        paths.sort();
        let mut snapshot = RevisionSnapshot {
            revisions: BTreeMap::new(),
            heads: BTreeSet::new(),
            issues: Vec::new(),
        };
        for path in paths {
            match read_record::<T>(&path, self.entity_id) {
                Ok(record) => {
                    if let Some(previous) = snapshot.revisions.get(&record.revision_id()) {
                        if previous != &record {
                            snapshot.issues.push(format!(
                                "Revision ID has different content: {}",
                                path.display()
                            ));
                        }
                    } else {
                        snapshot.revisions.insert(record.revision_id(), record);
                    }
                }
                Err(error) => snapshot
                    .issues
                    .push(format!("{}: {error:#}", path.display())),
            }
        }
        let identities: BTreeSet<_> = snapshot
            .revisions
            .values()
            .map(Revision::entity_id)
            .collect();
        if identities.len() > 1 {
            snapshot
                .issues
                .push("Files belong to different libraries or games".into());
        }
        reconcile(&mut snapshot);
        Ok(snapshot)
    }

    pub fn checked<T: Revision>(&self, expected: &BTreeSet<Uuid>) -> Result<RevisionSnapshot<T>> {
        let snapshot = self.inspect::<T>()?;
        ensure!(
            snapshot.issues.is_empty(),
            "Files need attention: {}",
            snapshot.issues.join("; ")
        );
        ensure!(
            !expected.is_empty() && snapshot.heads == *expected,
            "Record changed or has a conflict. Review the latest revisions"
        );
        Ok(snapshot)
    }

    pub fn publish_child<T: Revision>(
        &self,
        mut record: T,
        snapshot: RevisionSnapshot<T>,
    ) -> Result<T> {
        record.set_revision(Uuid::new_v4(), snapshot.heads.iter().copied().collect());
        validate_for_write(&record)?;
        // Retain parents found only in a Dropbox conflict copy before a later
        // cleanup can remove that copy. Never rewrite the original candidate.
        for previous in snapshot.revisions.values() {
            let path = self
                .history
                .join(format!("{}.json", previous.revision_id()));
            if path.exists() {
                ensure!(
                    read_record::<T>(&path, self.entity_id)? == *previous,
                    "History changed; review the latest revisions"
                );
                File::open(&path)?.sync_all()?;
            } else {
                storage::retain_revision(&path, previous)?;
            }
        }
        #[cfg(unix)]
        File::open(&self.history)?.sync_all()?;
        self.checked::<T>(&snapshot.heads)?;
        self.publish(&record)?;
        Ok(record)
    }

    pub fn publish<T: Revision>(&self, record: &T) -> Result<()> {
        validate_for_write(record)?;
        storage::save_record(
            &self.current,
            &self.history.join(format!("{}.json", record.revision_id())),
            record,
        )
    }
    pub fn lock(&self) -> Result<File> {
        // Advisory locks coordinate local processes only. OS release on exit
        // avoids stale-lock cleanup; sync copies of this empty file are ignored.
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.history.join(".writer.lock"))
            .context("Revision history is unavailable")?;
        file.try_lock()
            .context("Another local writer is saving this game")?;
        Ok(file)
    }
}
fn is_json(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "json")
}

pub(crate) fn validate_for_write<T: Revision>(record: &T) -> Result<()> {
    record.validate()?;
    ensure!(
        (serde_json::to_vec_pretty(record)?.len() as u64) < MAX_RECORD_BYTES,
        "Record exceeds the 1 MiB limit"
    );
    Ok(())
}

fn read_record<T: Revision>(path: &Path, entity_id: Option<Uuid>) -> Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_RECORD_BYTES,
        "Record exceeds the 1 MiB limit"
    );
    // Read the version first so a newer shape is reported as unsupported, not corrupt.
    #[derive(serde::Deserialize)]
    struct Header {
        schema_version: u32,
    }
    let header: Header = serde_json::from_slice(&bytes)?;
    ensure!(
        header.schema_version == SCHEMA_VERSION,
        "Unsupported schema version {}",
        header.schema_version
    );
    let record: T = serde_json::from_slice(&bytes)?;
    record.validate()?;
    ensure!(
        entity_id.is_none_or(|id| record.entity_id() == id),
        "Record identity does not match its file location"
    );
    Ok(record)
}

fn reconcile<T: Revision>(snapshot: &mut RevisionSnapshot<T>) {
    snapshot.heads = snapshot.revisions.keys().copied().collect();
    let mut children: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    let mut degree: BTreeMap<Uuid, usize> = BTreeMap::new();
    for record in snapshot.revisions.values() {
        degree.insert(record.revision_id(), record.parents().len());
        for parent in record.parents() {
            if !snapshot.revisions.contains_key(parent) {
                snapshot
                    .issues
                    .push(format!("Missing revision parent {parent}"));
            }
            snapshot.heads.remove(parent);
            children
                .entry(*parent)
                .or_default()
                .push(record.revision_id());
        }
    }
    // Iterative traversal also handles long histories without recursive stack growth.
    let mut ready: VecDeque<_> = degree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for child in children.get(&id).into_iter().flatten() {
            if let Some(count) = degree.get_mut(child) {
                *count -= 1;
                if *count == 0 {
                    ready.push_back(*child);
                }
            }
        }
    }
    if visited != snapshot.revisions.len() {
        snapshot
            .issues
            .push("Revision history is incomplete or contains a cycle".into());
    }
}
