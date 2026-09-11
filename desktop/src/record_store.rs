//! Game editing operations over the shared revision file service.
use crate::{
    records::{ExtraFields, GameData, GameRevision, PersonalData, SCHEMA_VERSION},
    revision_store::{validate_for_write, RevisionFiles, RevisionSnapshot},
};
use anyhow::{ensure, Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub type RecordSnapshot = RevisionSnapshot<GameRevision>;
/// The games/ and history/ folders must already exist in an offline library.
/// This service neither creates a library manifest nor scans other game IDs.
pub struct RecordStore {
    root: PathBuf,
}

impl RecordStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        ensure!(
            cfg!(unix),
            "Record storage is not implemented for this platform"
        );
        let root = root.into();
        ensure!(
            root.join("games").is_dir() && root.join("history").is_dir(),
            "Library games/ and history/ folders are unavailable"
        );
        Ok(Self { root })
    }

    pub fn create(&self, game: GameData) -> Result<GameRevision> {
        let record = GameRevision {
            schema_version: SCHEMA_VERSION,
            game_id: Uuid::new_v4(),
            revision_id: Uuid::new_v4(),
            parents: Vec::new(),
            deleted: false,
            game,
            extra: ExtraFields::new(),
        };
        validate_for_write(&record)?;
        let folder = self.files(record.game_id).history;
        fs::create_dir(&folder).context("Could not create game history")?;
        // Persist the new history directory before any current record points into it.
        #[cfg(unix)]
        File::open(self.root.join("history"))?.sync_all()?;
        self.files(record.game_id).publish(&record)?;
        Ok(record)
    }

    pub fn inspect(&self, game_id: Uuid) -> Result<RecordSnapshot> {
        let mut issues = Vec::new();
        let candidates = discover(&self.root, &mut issues)?;
        let mut snapshot = self.inspect_candidates(
            game_id,
            candidates.get(&game_id).map(Vec::as_slice).unwrap_or(&[]),
        )?;
        snapshot.issues.extend(issues);
        Ok(snapshot)
    }

    pub(crate) fn inspect_candidates(
        &self,
        game_id: Uuid,
        paths: &[PathBuf],
    ) -> Result<RecordSnapshot> {
        self.files(game_id).inspect_candidates(Some(paths))
    }
    /// Reject an edit based on an old revision or an unresolved branch.
    pub fn edit_personal(
        &self,
        game_id: Uuid,
        expected: Uuid,
        personal: PersonalData,
    ) -> Result<GameRevision> {
        let files = self.files(game_id);
        let _lock = files.lock()?;
        let snapshot = self.checked(game_id, &BTreeSet::from([expected]))?;
        let mut record = snapshot.revisions[&expected].clone();
        ensure!(!record.deleted, "Restore the archived game before editing");
        // Unknown personal fields are retained even if the editor does not know them.
        let mut personal = personal;
        for (key, value) in &record.game.personal.extra {
            personal
                .extra
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
        record.game.personal = personal;
        files.publish_child(record, snapshot)
    }

    /// Explicitly keep one branch and acknowledge every head the user reviewed.
    /// The unchosen branch stays in history. A newly arrived head rejects the action.
    pub fn resolve(
        &self,
        game_id: Uuid,
        expected: &BTreeSet<Uuid>,
        chosen: Uuid,
    ) -> Result<GameRevision> {
        let files = self.files(game_id);
        let _lock = files.lock()?;
        ensure!(
            expected.len() > 1 && expected.contains(&chosen),
            "Choose one of the conflicting heads"
        );
        let snapshot = self.checked(game_id, expected)?;
        files.publish_child(snapshot.revisions[&chosen].clone(), snapshot)
    }

    /// Archive or restore through a revision; never remove files to delete a game.
    pub fn set_archived(
        &self,
        game_id: Uuid,
        expected: Uuid,
        archived: bool,
    ) -> Result<GameRevision> {
        let files = self.files(game_id);
        let _lock = files.lock()?;
        let snapshot = self.checked(game_id, &BTreeSet::from([expected]))?;
        let mut record = snapshot.revisions[&expected].clone();
        record.deleted = archived;
        files.publish_child(record, snapshot)
    }

    fn checked(&self, id: Uuid, expected: &BTreeSet<Uuid>) -> Result<RecordSnapshot> {
        let snapshot = self.inspect(id)?;
        ensure!(
            snapshot.issues.is_empty(),
            "Game files need attention: {}",
            snapshot.issues.join("; ")
        );
        ensure!(
            !expected.is_empty() && snapshot.heads == *expected,
            "Game changed or has a conflict. Review the latest revisions"
        );
        Ok(snapshot)
    }

    fn files(&self, game_id: Uuid) -> RevisionFiles {
        RevisionFiles {
            current: self.root.join("games").join(format!("{game_id}.json")),
            history: self.root.join("history").join(game_id.to_string()),
            entity_id: Some(game_id),
        }
    }
}

pub(crate) fn discover(
    root: &Path,
    issues: &mut Vec<String>,
) -> Result<BTreeMap<Uuid, Vec<PathBuf>>> {
    let mut candidates: BTreeMap<Uuid, Vec<PathBuf>> = BTreeMap::new();
    for entry in fs::read_dir(root.join("history"))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Ok(id) = Uuid::parse_str(&entry.file_name().to_string_lossy()) {
                candidates.entry(id).or_default();
            }
        }
    }
    // Read each current file once for discovery. Full validation and history
    // reconciliation use the shared record service, including renamed copies.
    for entry in fs::read_dir(root.join("games"))? {
        let path = entry?.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let filename_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.get(..36))
            .and_then(|prefix| Uuid::parse_str(prefix).ok());
        let parsed = (|| -> Result<Uuid> {
            let mut bytes = Vec::new();
            File::open(&path)?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 1024 * 1024, "Record exceeds 1 MiB");
            #[derive(serde::Deserialize)]
            struct Header {
                game_id: Uuid,
            }
            Ok(serde_json::from_slice::<Header>(&bytes)?.game_id)
        })();
        match (filename_id, parsed) {
            (Some(id), _) => {
                candidates.entry(id).or_default().push(path);
            }
            (None, Ok(id)) => {
                candidates.entry(id).or_default().push(path);
            }
            (None, Err(error)) => issues.push(format!("{}: {error:#}", path.display())),
        }
    }
    Ok(candidates)
}
