//! Bounded, device-local diagnostics. Never record request queries, headers with
//! credentials, response bodies, or personal game fields.
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const MAX_LOGS: usize = 10;
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(super) enum Event<'a> {
    Started {
        version: &'a str,
        platform: &'a str,
        arch: &'a str,
    },
    Stage {
        stage: &'a str,
        app_id: Option<u32>,
    },
    Issue {
        detail: &'a str,
    },
    Counts {
        new_games: usize,
        updated_games: usize,
        unchanged_games: usize,
        missing_details: usize,
        details_completed: usize,
    },
    Request {
        endpoint: &'a str,
        status: Option<u16>,
        content_type: Option<&'a str>,
        retry_after: Option<&'a str>,
        elapsed_ms: u128,
        outcome: &'a str,
    },
    Finished {
        outcome: &'a str,
        imported: usize,
        failures: usize,
        elapsed_ms: u128,
    },
    Truncated,
}

#[derive(Serialize)]
struct Entry<'a> {
    unix_ms: u128,
    #[serde(flatten)]
    event: Event<'a>,
}

struct Writer {
    file: File,
    bytes: usize,
    truncated: bool,
    failed: bool,
}

pub(super) struct SyncLog {
    path: PathBuf,
    writer: Mutex<Writer>,
    secrets: Vec<String>,
    started: Instant,
}

pub fn directory() -> Result<PathBuf> {
    let root = if let Some(root) = crate::settings::preview_dir() {
        root
    } else {
        directories::ProjectDirs::from("app", "GameSync", "GameSync")
            .context("Log directory is unavailable")?
            .data_local_dir()
            .to_owned()
    };
    Ok(root.join("logs").join("steam-sync"))
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

impl SyncLog {
    pub(super) fn open(folder: &Path, account: &str, key: &str) -> Result<Self> {
        fs::create_dir_all(folder)?;
        let path = folder.join(format!(
            "steam-sync-{}-{}.jsonl",
            unix_ms(),
            uuid::Uuid::new_v4()
        ));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        // Retention must not remove a log that another sync is still writing.
        file.try_lock()?;
        let log = Self {
            path,
            writer: Mutex::new(Writer {
                file,
                bytes: 0,
                truncated: false,
                failed: false,
            }),
            secrets: [account, key]
                .into_iter()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            started: Instant::now(),
        };
        log.record(Event::Started {
            version: env!("CARGO_PKG_VERSION"),
            platform: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        });
        if let Err(error) = prune(folder, &log.path) {
            log.record(Event::Issue {
                detail: &format!("Could not remove old sync logs: {error}"),
            });
        }
        Ok(log)
    }

    pub(super) fn record(&self, event: Event<'_>) {
        let final_event = matches!(event, Event::Finished { .. });
        let Ok(mut line) = serde_json::to_string(&Entry {
            unix_ms: unix_ms(),
            event,
        }) else {
            return;
        };
        // Redact even JSON-escaped credentials if an upstream error includes one.
        for secret in &self.secrets {
            if let Ok(encoded) = serde_json::to_string(secret) {
                line = line.replace(&encoded[1..encoded.len() - 1], "[redacted]");
            }
        }
        line.push('\n');
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        if writer.failed {
            return;
        }
        // Reserve room for the truncation marker and final summary.
        if !final_event && writer.bytes + line.len() > MAX_BYTES - 1024 {
            if writer.truncated {
                return;
            }
            writer.truncated = true;
            line = serde_json::to_string(&Entry {
                unix_ms: unix_ms(),
                event: Event::Truncated,
            })
            .unwrap_or_default();
            line.push('\n');
        }
        if let Err(error) = writer
            .file
            .write_all(line.as_bytes())
            .and_then(|_| writer.file.flush())
        {
            writer.failed = true;
            log::warn!("Could not write Steam sync diagnostics: {error}");
        } else {
            writer.bytes += line.len();
        }
    }

    pub(super) fn finish(&self, outcome: &str, imported: usize, failures: usize) {
        self.record(Event::Finished {
            outcome,
            imported,
            failures,
            elapsed_ms: self.started.elapsed().as_millis(),
        });
        // Windows locks also block readers. Make the completed log readable
        // before the final status displays its path.
        if let Ok(writer) = self.writer.lock() {
            let _ = writer.file.unlock();
        }
    }

    pub(super) fn feedback(&self) -> String {
        if self.writer.lock().is_ok_and(|writer| !writer.failed) {
            format!("Sync log: {}", self.path.display())
        } else {
            "Could not finish writing the Steam sync log.".into()
        }
    }
}

fn prune(folder: &Path, current: &Path) -> Result<()> {
    let mut paths = fs::read_dir(folder)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path != current)
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("steam-sync-") && n.ends_with(".jsonl"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    let excess = paths.len().saturating_sub(MAX_LOGS - 1);
    for path in paths.into_iter().take(excess) {
        let file = OpenOptions::new().read(true).write(true).open(&path)?;
        if file.try_lock().is_ok() {
            // Close the handle before removal on Windows.
            drop(file);
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_survive_reopen_and_redact_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let log = SyncLog::open(temp.path(), "76561198000000000", "a\"secret").unwrap();
        log.record(Event::Issue {
            detail: "failure for 76561198000000000 with a\"secret",
        });
        log.finish("failed", 0, 1);
        let path = log.path.clone();
        drop(log);
        let text = fs::read_to_string(path).unwrap();
        assert!(!text.contains("76561198000000000"));
        assert!(!text.contains("secret"));
        let entries = text
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(entries[0]["event"], "started");
        assert_eq!(
            entries[1]["detail"],
            "failure for [redacted] with [redacted]"
        );
        assert_eq!(entries[2]["outcome"], "failed");
    }

    #[test]
    fn retention_keeps_ten_logs_and_unrelated_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("keep.txt"), "keep").unwrap();
        for i in 0..12 {
            fs::write(
                temp.path().join(format!("steam-sync-{i:013}-old.jsonl")),
                "old",
            )
            .unwrap();
        }
        let _log = SyncLog::open(temp.path(), "", "").unwrap();
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), MAX_LOGS + 1);
        assert!(temp.path().join("keep.txt").exists());
    }

    #[test]
    fn size_cap_keeps_one_marker_and_the_final_summary() {
        let temp = tempfile::tempdir().unwrap();
        let log = SyncLog::open(temp.path(), "", "").unwrap();
        let detail = "x".repeat(MAX_BYTES);
        for _ in 0..3 {
            log.record(Event::Issue { detail: &detail });
        }
        log.finish("cancelled", 5, 2);
        let text = fs::read_to_string(&log.path).unwrap();
        assert!(text.len() < MAX_BYTES);
        assert_eq!(text.matches("\"event\":\"truncated\"").count(), 1);
        assert!(text.lines().last().unwrap().contains("cancelled"));
    }
}
