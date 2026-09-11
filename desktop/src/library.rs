//! Library identity and ordered status definitions. Game values stay in game files.

use crate::{
    records::{check_extra, ExtraFields, SCHEMA_VERSION},
    revision_store::{validate_for_write, Revision, RevisionFiles, RevisionSnapshot},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusDefinition {
    pub key: String,
    pub label: String,
    pub recommendation_eligible: bool,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryDefinitions {
    pub name: String,
    pub default_status: String,
    /// Array order is the display order. Keys remain stable when labels change.
    pub statuses: Vec<StatusDefinition>,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl LibraryDefinitions {
    pub fn new(name: impl Into<String>) -> Self {
        let statuses = [
            ("backlog", "Backlog", true),
            ("wanted", "Want to play", true),
            ("playing", "Playing", true),
            ("paused", "Paused", false),
            ("completed", "Completed", false),
            ("dropped", "Dropped", false),
        ]
        .into_iter()
        .map(|(key, label, recommendation_eligible)| StatusDefinition {
            key: key.into(),
            label: label.into(),
            recommendation_eligible,
            extra: ExtraFields::new(),
        })
        .collect();
        Self {
            name: name.into(),
            default_status: "backlog".into(),
            statuses,
            extra: ExtraFields::new(),
        }
    }

    pub fn status(&self, key: &str) -> Option<&StatusDefinition> {
        self.statuses.iter().find(|status| status.key == key)
    }

    pub fn validate(&self) -> Result<()> {
        check_extra(&self.extra, &["name", "default_status", "statuses"])?;
        ensure!(!self.name.trim().is_empty(), "Library name is empty");
        let mut keys = BTreeSet::new();
        for status in &self.statuses {
            check_extra(&status.extra, &["key", "label", "recommendation_eligible"])?;
            ensure!(!status.key.is_empty() && status.key.len() <= 64 && status.key.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'), "Status keys must use lowercase letters, digits, underscores, or hyphens (1–64 bytes)");
            ensure!(
                keys.insert(&status.key),
                "Duplicate status key: {}",
                status.key
            );
            ensure!(!status.label.trim().is_empty(), "Status label is empty");
        }
        ensure!(
            self.status(&self.default_status).is_some(),
            "Default status is not defined"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryRevision {
    pub schema_version: u32,
    pub library_id: Uuid,
    pub revision_id: Uuid,
    pub parents: Vec<Uuid>,
    pub definitions: LibraryDefinitions,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Revision for LibraryRevision {
    fn entity_id(&self) -> Uuid {
        self.library_id
    }
    fn revision_id(&self) -> Uuid {
        self.revision_id
    }
    fn parents(&self) -> &[Uuid] {
        &self.parents
    }
    fn set_revision(&mut self, id: Uuid, parents: Vec<Uuid>) {
        self.revision_id = id;
        self.parents = parents;
    }
    fn validate(&self) -> Result<()> {
        check_extra(
            &self.extra,
            &[
                "schema_version",
                "library_id",
                "revision_id",
                "parents",
                "definitions",
            ],
        )?;
        ensure!(
            self.schema_version == SCHEMA_VERSION,
            "Unsupported library schema version"
        );
        ensure!(
            !self.library_id.is_nil() && !self.revision_id.is_nil(),
            "Library IDs must not be nil"
        );
        ensure!(
            self.parents.iter().collect::<BTreeSet<_>>().len() == self.parents.len()
                && self
                    .parents
                    .iter()
                    .all(|id| !id.is_nil() && *id != self.revision_id),
            "Invalid library revision parents"
        );
        self.definitions.validate()
    }
}

pub type LibrarySnapshot = RevisionSnapshot<LibraryRevision>;

pub struct LibraryStore {
    root: PathBuf,
}

impl LibraryStore {
    /// Create a new directory. Never adopt or overwrite an existing folder.
    /// A failed initialization leaves its partial folder available for inspection.
    pub fn create(root: impl Into<PathBuf>, name: impl Into<String>) -> Result<Self> {
        ensure!(
            cfg!(unix),
            "Library storage is not implemented for this platform"
        );
        let root = root.into();
        let record = LibraryRevision {
            schema_version: SCHEMA_VERSION,
            library_id: Uuid::new_v4(),
            revision_id: Uuid::new_v4(),
            parents: Vec::new(),
            definitions: LibraryDefinitions::new(name),
            extra: ExtraFields::new(),
        };
        validate_for_write(&record)?;
        fs::create_dir(&root)
            .context("Choose a new library folder; existing folders are not replaced")?;
        for folder in ["games", "media", "history", "history/library"] {
            fs::create_dir(root.join(folder))
                .context("Could not initialize library; partial folder retained")?;
        }
        #[cfg(unix)]
        {
            File::open(root.join("history"))?.sync_all()?;
            File::open(&root)?.sync_all()?;
            File::open(
                root.parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or(Path::new(".")),
            )?
            .sync_all()?;
        }
        let store = Self { root };
        store.files().publish(&record)?;
        Ok(store)
    }

    /// Open without changing files. Partial/newer/conflicting manifests remain inspectable.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        ensure!(
            cfg!(unix),
            "Library storage is not implemented for this platform"
        );
        let root = root.into();
        ensure!(
            ["games", "media", "history", "history/library"]
                .iter()
                .all(|folder| root.join(folder).is_dir()),
            "Library folders are unavailable or initialization is incomplete"
        );
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn inspect(&self) -> Result<LibrarySnapshot> {
        let mut snapshot: LibrarySnapshot = self.files().inspect()?;
        if snapshot.revisions.is_empty() {
            snapshot
                .issues
                .push("No valid library revisions are available".into());
        }
        Ok(snapshot)
    }

    /// Hold the local definition lock while a personal edit is checked and saved.
    /// Remote sync can still arrive later; its branch is retained by the record store.
    pub fn edit_game_personal(
        &self,
        expected_library: &LibraryRevision,
        game_id: Uuid,
        expected_game: Uuid,
        personal: crate::records::PersonalData,
    ) -> Result<crate::records::GameRevision> {
        let files = self.files();
        let _lock = files.lock()?;
        let snapshot =
            files.checked::<LibraryRevision>(&BTreeSet::from([expected_library.revision_id]))?;
        let current = &snapshot.revisions[&expected_library.revision_id];
        ensure!(
            current == expected_library,
            "Library changed. Review its latest definitions"
        );
        ensure!(
            current.definitions.status(&personal.status).is_some(),
            "Choose a status in this library"
        );
        crate::record_store::RecordStore::open(&self.root)?.edit_personal(
            game_id,
            expected_game,
            personal,
        )
    }

    /// Update name, default status, labels, order, or eligibility. Existing keys
    /// cannot be removed until game reassignment is implemented.
    pub fn edit(&self, expected: Uuid, definitions: LibraryDefinitions) -> Result<LibraryRevision> {
        let files = self.files();
        let _lock = files.lock()?;
        let snapshot = files.checked::<LibraryRevision>(&BTreeSet::from([expected]))?;
        self.save_definitions(
            &files,
            snapshot.revisions[&expected].clone(),
            definitions,
            snapshot,
        )
    }

    /// The user supplies the reviewed definitions and the branch whose unknown
    /// envelope fields to retain. Every conflicting head must be acknowledged.
    pub fn resolve(
        &self,
        expected: &BTreeSet<Uuid>,
        chosen: Uuid,
        definitions: LibraryDefinitions,
    ) -> Result<LibraryRevision> {
        ensure!(
            expected.len() > 1 && expected.contains(&chosen),
            "Choose a conflicting library revision"
        );
        let files = self.files();
        let _lock = files.lock()?;
        let snapshot = files.checked::<LibraryRevision>(expected)?;
        self.save_definitions(
            &files,
            snapshot.revisions[&chosen].clone(),
            definitions,
            snapshot,
        )
    }

    fn save_definitions(
        &self,
        files: &RevisionFiles,
        mut record: LibraryRevision,
        mut definitions: LibraryDefinitions,
        snapshot: LibrarySnapshot,
    ) -> Result<LibraryRevision> {
        // No cross-file status migration exists yet. Even a resolution must keep
        // keys from both branches so synced games do not lose their definitions.
        for previous in snapshot.revisions.values() {
            for status in &previous.definitions.statuses {
                ensure!(
                    definitions.status(&status.key).is_some(),
                    "Status removal requires game reassignment: {}",
                    status.key
                );
            }
        }
        for (key, value) in &record.definitions.extra {
            definitions
                .extra
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
        for status in &mut definitions.statuses {
            if let Some(previous) = record.definitions.status(&status.key) {
                for (key, value) in &previous.extra {
                    status
                        .extra
                        .entry(key.clone())
                        .or_insert_with(|| value.clone());
                }
            }
        }
        record.definitions = definitions;
        files.publish_child(record, snapshot)
    }

    fn files(&self) -> RevisionFiles {
        RevisionFiles {
            current: self.root.join("library.json"),
            history: self.root.join("history/library"),
            entity_id: None,
        }
    }
}
