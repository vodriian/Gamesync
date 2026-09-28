//! Read-only folder scans with a last-valid SQLite index outside the library.
//! One worker owns the reader. Failed or missing files never delete cached games.

use crate::{
    library::{LibraryRevision, LibraryStore},
    record_store::{discover, RecordStore},
    records::GameRevision,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct LoadedLibrary {
    pub root: PathBuf,
    /// File problems block edits; cover problems only affect display.
    pub write_issue: Option<String>,
    pub manifest: LibraryRevision,
    pub games: Vec<GameRevision>,
    pub conflicts: BTreeMap<Uuid, String>,
    pub covers: BTreeMap<Uuid, PathBuf>,
    pub media_version: u64,
    pub issues: Vec<String>,
}

pub struct LibraryReader {
    root: PathBuf,
    index: Connection,
    manifest: Option<LibraryRevision>,
    games: BTreeMap<Uuid, GameRevision>,
    conflicts: BTreeMap<Uuid, String>,
}

impl LibraryReader {
    /// The cache must exist and stay outside the library,
    /// including after symlink resolution. The caller chooses a device-local cache.
    pub fn open(root: &Path, cache: &Path) -> Result<Self> {
        let root = resolve_folder(root)?;
        let cache = cache
            .canonicalize()
            .context("Local cache folder is unavailable")?;
        ensure!(
            !cache.starts_with(&root),
            "The index must be outside the library folder"
        );
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        root.hash(&mut hash);
        let index = Connection::open(cache.join(format!("library-{:016x}.sqlite", hash.finish())))?;
        index.busy_timeout(std::time::Duration::from_secs(2))?;
        index.execute_batch(
            "CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS games (id TEXT PRIMARY KEY, record TEXT NOT NULL);",
        )?;
        let binding: Option<String> = index
            .query_row("SELECT value FROM metadata WHERE key='root'", [], |row| {
                row.get(0)
            })
            .optional()?;
        let root_name = root.to_string_lossy();
        ensure!(
            binding.as_deref().is_none_or(|bound| bound == root_name),
            "Local index belongs to another folder"
        );
        index.execute(
            "INSERT OR IGNORE INTO metadata VALUES ('root', ?1)",
            [&*root_name],
        )?;
        let manifest: Option<String> = index
            .query_row(
                "SELECT value FROM metadata WHERE key='manifest'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let manifest = manifest
            .map(|value| serde_json::from_str::<LibraryRevision>(&value))
            .transpose()?;
        let games = {
            let mut statement = index.prepare("SELECT record FROM games")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            let mut games = BTreeMap::new();
            for row in rows {
                let game: GameRevision = serde_json::from_str(&row?)?;
                game.validate()?;
                games.insert(game.game_id, game);
            }
            games
        };
        Ok(Self {
            root,
            index,
            manifest,
            games,
            conflicts: BTreeMap::new(),
        })
    }

    pub fn refresh(&mut self) -> Result<LoadedLibrary> {
        self.conflicts.clear();
        let mut issues = Vec::new();
        let manifest = match LibraryStore::open(&self.root).and_then(|store| store.inspect()) {
            Ok(snapshot) => {
                if let Some(current) = snapshot.current() {
                    Some(current.clone())
                } else {
                    issues.extend(snapshot.issues);
                    issues.push(
                        "Library definitions need attention; showing the last valid data.".into(),
                    );
                    None
                }
            }
            Err(error) => {
                issues.push(format!("Library unavailable: {error:#}"));
                None
            }
        };
        let Some(manifest) = manifest else {
            return self.loaded(issues);
        };
        if self
            .manifest
            .as_ref()
            .is_some_and(|old| old.library_id != manifest.library_id)
        {
            self.games.clear();
            self.index.execute("DELETE FROM games", [])?;
        }
        self.manifest = Some(manifest.clone());
        let candidates = match discover(&self.root, &mut issues) {
            Ok(candidates) => candidates,
            Err(error) => {
                issues.push(format!("Could not scan games: {error:#}"));
                return self.loaded(issues);
            }
        };
        let store = RecordStore::open(&self.root)?;
        let ids: BTreeSet<_> = candidates
            .keys()
            .chain(self.games.keys())
            .copied()
            .collect();
        for id in ids {
            match store
                .inspect_candidates(id, candidates.get(&id).map(Vec::as_slice).unwrap_or(&[]))
            {
                Ok(snapshot) => {
                    if let Some(game) = snapshot.current() {
                        if manifest
                            .definitions
                            .status(&game.game.personal.status)
                            .is_some()
                        {
                            self.games.insert(id, game.clone());
                        } else {
                            issues.push(format!(
                                "{}: unknown status '{}'; keeping last valid data.",
                                game.game.title, game.game.personal.status
                            ));
                        }
                    } else {
                        let title = self
                            .games
                            .get(&id)
                            .map(|game| game.game.title.clone())
                            .or_else(|| {
                                snapshot
                                    .heads
                                    .first()
                                    .and_then(|head| snapshot.revisions.get(head))
                                    .map(|record| record.game.title.clone())
                            })
                            .unwrap_or_else(|| id.to_string());
                        if snapshot.has_conflict() {
                            let name = snapshot
                                .heads
                                .first()
                                .and_then(|head| snapshot.revisions.get(head))
                                .map(|record| record.game.title.clone())
                                .unwrap_or_else(|| title.clone());
                            self.conflicts.insert(id, name);
                        }
                        let reason = if snapshot.has_conflict() {
                            "conflicting edits"
                        } else if snapshot.revisions.is_empty() {
                            "files missing"
                        } else {
                            "incomplete history"
                        };
                        let action = if self.games.contains_key(&id) {
                            "keeping last valid data"
                        } else if snapshot.has_conflict() {
                            "open Conflicts to review"
                        } else {
                            "waiting for valid files"
                        };
                        issues.push(format!("{title}: {reason}; {action}."));
                        issues.extend(snapshot.issues);
                    }
                }
                Err(error) => issues.push(format!("{id}: {error:#}")),
            }
        }
        let transaction = self.index.transaction()?;
        transaction.execute("INSERT INTO metadata VALUES ('manifest', ?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [serde_json::to_string(&manifest)?])?;
        {
            let mut statement = transaction.prepare("INSERT INTO games VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record WHERE record != excluded.record")?;
            for (id, game) in &self.games {
                statement.execute(params![id.to_string(), serde_json::to_string(game)?])?;
            }
        }
        transaction.commit()?;
        self.loaded(issues)
    }

    fn loaded(&self, mut issues: Vec<String>) -> Result<LoadedLibrary> {
        let manifest = self
            .manifest
            .clone()
            .with_context(|| format!("No valid library is available. {}", issues.join("; ")))?;
        let write_issue = issues.first().cloned();
        let mut covers = BTreeMap::new();
        let mut media = std::collections::hash_map::DefaultHasher::new();
        let games: Vec<_> = self
            .games
            .values()
            .filter(|record| !record.deleted)
            .cloned()
            .collect();
        for game in &games {
            if let Some(relative) = game.game.cover() {
                let path = self.root.join(relative);
                // Never follow an imported cover symlink outside media/.
                match path.canonicalize() {
                    Ok(resolved) if resolved.starts_with(self.root.join("media")) => {
                        if let Ok(meta) = resolved.metadata() {
                            resolved.hash(&mut media);
                            meta.len().hash(&mut media);
                            meta.modified().ok().hash(&mut media);
                        }
                        covers.insert(game.game_id, resolved);
                    }
                    Ok(_) => {
                        issues.push(format!("{}: cover points outside media/.", game.game.title))
                    }
                    Err(_) => issues.push(format!("{}: cover is unavailable.", game.game.title)),
                }
            }
        }
        Ok(LoadedLibrary {
            root: self.root.clone(),
            conflicts: self.conflicts.clone(),
            write_issue,
            manifest,
            games,
            covers,
            media_version: media.finish(),
            issues,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn resolve_folder(path: &Path) -> Result<PathBuf> {
    // Resolve existing ancestors when the library is temporarily missing, so
    // /tmp and /private/tmp still address the same last-valid local index.
    let mut path = std::path::absolute(path)?;
    let mut missing = Vec::new();
    loop {
        match path.canonicalize() {
            Ok(mut resolved) => {
                for part in missing.into_iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    path.file_name()
                        .context("Library folder has no name")?
                        .to_owned(),
                );
                ensure!(path.pop(), "Library path has no existing parent");
            }
            Err(error) => return Err(error).context("Could not resolve library folder"),
        }
    }
}
