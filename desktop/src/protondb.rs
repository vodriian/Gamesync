//! Optional ProtonDB enrichment for Steam Deck fit. The source is ProtonDB's
//! monthly community export on GitHub, released under the ODbL; the app does
//! not call protondb.com. Only small per-game results stay on this device, in
//! a cache apart from library records. Turning enrichment off deletes it.

use crate::suitability::{ProtonDeck, PROTON_MIN_REPORTS};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

const LISTING: &str = "https://api.github.com/repos/bdefore/protondb-data/contents/reports";
const DOWNLOAD: &str = "https://github.com/bdefore/protondb-data/raw/master/reports/";
/// Required credit for data from the export.
pub const CREDIT: &str = "Contains information from ProtonDB, made available under the ODbL.";
/// The export is about 70 MB; refuse anything far larger.
const DOWNLOAD_LIMIT: u64 = 400 * 1024 * 1024;
/// A new export appears about once a month.
const CHECK_INTERVAL_SECONDS: i64 = 30 * 86_400;

/// Imported results. `apps` holds games with enough Deck reports only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtonData {
    /// Export file name, for example `reports_oct1_2026.tar.gz`.
    pub source: String,
    /// Unix seconds of the last check for a new export.
    pub checked_at: i64,
    pub apps: BTreeMap<u32, ProtonDeck>,
}

impl ProtonData {
    /// The export date as written in its file name, for example "October 1, 2026".
    pub fn export_label(&self) -> String {
        export_date(&self.source).map_or(self.source.clone(), |(year, month, day)| {
            const MONTHS: [&str; 12] = [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ];
            format!("{} {day}, {year}", MONTHS[month as usize - 1])
        })
    }

    pub fn due(&self, now: i64) -> bool {
        now - self.checked_at >= CHECK_INTERVAL_SECONDS
    }
}

fn path() -> Result<PathBuf> {
    Ok(
        directories::ProjectDirs::from("app", "GameSync", "GameSync")
            .context("App data directory is unavailable")?
            .data_dir()
            .join("protondb-deck.json"),
    )
}

pub fn load() -> Result<Option<ProtonData>> {
    match fs::read(path()?) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).context("ProtonDB data is unreadable")?,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Turning enrichment off removes all downloaded data.
pub fn remove() -> Result<()> {
    match fs::remove_file(path()?) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

fn save(data: &ProtonData) -> Result<()> {
    let path = path()?;
    let parent = path.parent().context("App data directory is unavailable")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec(data)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Year, month, and day from `reports_<mon><day>_<year>.tar.gz`.
fn export_date(name: &str) -> Option<(u16, u8, u8)> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let stem = name.strip_prefix("reports_")?.strip_suffix(".tar.gz")?;
    let (month_day, year) = stem.split_once('_')?;
    let month = MONTHS.iter().position(|m| month_day.starts_with(m))?;
    let day = month_day[3..].parse().ok()?;
    Some((year.parse().ok()?, month as u8 + 1, day))
}

fn export_seconds(name: &str) -> Option<i64> {
    let (year, month, day) = export_date(name)?;
    // Days from the civil date (Howard Hinnant's algorithm).
    let (y, m) = if month <= 2 {
        (i64::from(year) - 1, i64::from(month) + 9)
    } else {
        (i64::from(year), i64::from(month) - 3)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400)
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent("GameSync (personal game library)")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(15 * 60))
        .build()?)
}

/// The newest export file name in the public repository.
fn latest_export(client: &reqwest::blocking::Client) -> Result<String> {
    let response = client
        .get(LISTING)
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("Could not reach GitHub. Check your connection and retry.")?;
    ensure!(
        response.status().is_success(),
        "GitHub returned HTTP {}. Try again later.",
        response.status().as_u16()
    );
    let files: Vec<serde_json::Value> = response.json().context("GitHub listing is invalid")?;
    files
        .iter()
        .filter_map(|file| file["name"].as_str())
        .filter_map(|name| Some((export_seconds(name)?, name)))
        .max()
        .map(|(_, name)| name.to_owned())
        .context("No ProtonDB export was found")
}

/// Download progress in bytes, and the total when the server reports it.
pub type Progress<'a> = &'a mut dyn FnMut(u64, Option<u64>);

/// Fetch the newest export when it differs from `current`, then save the
/// results. With the same export, only the check time changes.
pub fn update(
    current: Option<&ProtonData>,
    cancel: &AtomicBool,
    progress: Progress,
    now: i64,
) -> Result<ProtonData> {
    let data = fetch(current, cancel, progress, now)?;
    save(&data)?;
    Ok(data)
}

/// `update` without saving, so a network check leaves app data alone.
pub fn fetch(
    current: Option<&ProtonData>,
    cancel: &AtomicBool,
    progress: Progress,
    now: i64,
) -> Result<ProtonData> {
    let client = client()?;
    let source = latest_export(&client)?;
    if let Some(current) = current.filter(|current| current.source == source) {
        return Ok(ProtonData {
            checked_at: now,
            ..current.clone()
        });
    }
    let response = client
        .get(format!("{DOWNLOAD}{source}"))
        .send()
        .context("Could not download ProtonDB data. Check your connection and retry.")?;
    ensure!(
        response.status().is_success(),
        "GitHub returned HTTP {} for the ProtonDB export. Try again later.",
        response.status().as_u16()
    );
    let total = response.content_length();
    ensure!(
        total.is_none_or(|total| total <= DOWNLOAD_LIMIT),
        "The ProtonDB export is unexpectedly large"
    );
    let reader = Counting {
        inner: response.take(DOWNLOAD_LIMIT),
        read: 0,
        total,
        cancel,
        progress,
    };
    // All Deck reports count. A two-year window keeps 776 games instead of
    // 2,604 (October 2026 export), and the Deck itself dates from 2022.
    let apps = read_export(flate2::read::GzDecoder::new(reader), i64::MIN).map_err(|error| {
        if cancel.load(Ordering::Relaxed) {
            anyhow::anyhow!("Cancelled")
        } else {
            error
        }
    })?;
    ensure!(
        !apps.is_empty(),
        "The ProtonDB export has no Steam Deck reports"
    );
    Ok(ProtonData {
        source,
        checked_at: now,
        apps,
    })
}

struct Counting<'a, R> {
    inner: R,
    read: u64,
    total: Option<u64>,
    cancel: &'a AtomicBool,
    progress: Progress<'a>,
}

impl<R: Read> Read for Counting<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("Cancelled"));
        }
        let count = self.inner.read(buf)?;
        self.read += count as u64;
        (self.progress)(self.read, self.total);
        Ok(count)
    }
}

/// Read the single JSON file in an uncompressed tar stream, one report at a
/// time, and keep Steam Deck results per game.
pub fn read_export(mut tar: impl Read, cutoff: i64) -> Result<BTreeMap<u32, ProtonDeck>> {
    let mut header = [0u8; 512];
    tar.read_exact(&mut header)
        .context("The ProtonDB export is incomplete")?;
    let size_field = std::str::from_utf8(&header[124..136])
        .context("The ProtonDB export is invalid")?
        .trim_matches(|c: char| c == '\0' || c == ' ');
    let size = u64::from_str_radix(size_field, 8).context("The ProtonDB export is invalid")?;
    let json = std::io::BufReader::new(tar.take(size));
    let mut counts = HashMap::new();
    let mut reader = serde_json::Deserializer::from_reader(json);
    serde::Deserializer::deserialize_seq(
        &mut reader,
        Reports {
            cutoff,
            counts: &mut counts,
        },
    )
    .map_err(|error| anyhow::anyhow!("The ProtonDB export is invalid: {error}"))?;
    Ok(counts
        .into_iter()
        .filter(|(_, count)| count.reports >= u32::from(PROTON_MIN_REPORTS))
        .map(|(id, count)| (id, count.result()))
        .collect())
}

#[derive(Default)]
struct Count {
    reports: u32,
    runs: u32,
    /// (problem answers, all answers) for each question.
    performance: (u32, u32),
    battery: (u32, u32),
    readability: (u32, u32),
}

impl Count {
    fn result(&self) -> ProtonDeck {
        let percent = |(yes, all): (u32, u32)| (all > 0).then(|| (yes * 100 / all) as u8);
        ProtonDeck {
            reports: self.reports.min(u32::from(u16::MAX)) as u16,
            runs_percent: (self.runs * 100 / self.reports.max(1)) as u8,
            performance_percent: percent(self.performance),
            battery_percent: percent(self.battery),
            readability_percent: percent(self.readability),
        }
    }
}

/// Only the fields fit uses. Every field is optional and loosely typed, so
/// one odd community report is skipped instead of failing the whole import.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Report {
    app: Option<ReportApp>,
    responses: Responses,
    #[serde(rename = "systemInfo")]
    system: System,
    timestamp: Option<serde_json::Value>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ReportApp {
    steam: ReportSteam,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ReportSteam {
    #[serde(rename = "appId")]
    app_id: Option<serde_json::Value>,
}

/// "yes" answers to the fault questions mean a problem. Deck reports also
/// answer battery and readability questions; "yes" there reports a problem
/// too, judging by heavy games having far higher shares than light ones.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Responses {
    verdict: Option<serde_json::Value>,
    performance_faults: Option<serde_json::Value>,
    battery_performance: Option<serde_json::Value>,
    readability: Option<serde_json::Value>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct System {
    cpu: Option<serde_json::Value>,
    os: Option<serde_json::Value>,
}

fn text(value: &Option<serde_json::Value>) -> &str {
    value
        .as_ref()
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

impl System {
    /// LCD and OLED Decks report a custom AMD APU; SteamOS covers the rest.
    fn steam_deck(&self) -> bool {
        text(&self.cpu).starts_with("AMD Custom APU") || text(&self.os).starts_with("SteamOS")
    }
}

struct Reports<'a> {
    cutoff: i64,
    counts: &'a mut HashMap<u32, Count>,
}

impl<'de> serde::de::Visitor<'de> for Reports<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a list of ProtonDB reports")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(report) = seq.next_element::<Report>()? {
            let timestamp = report
                .timestamp
                .as_ref()
                .and_then(|t| t.as_i64())
                .unwrap_or(0);
            if timestamp < self.cutoff || !report.system.steam_deck() {
                continue;
            }
            let id = report.app.as_ref().and_then(|app| {
                let id = app.steam.app_id.as_ref()?;
                id.as_u64()
                    .and_then(|id| u32::try_from(id).ok())
                    .or_else(|| id.as_str()?.parse().ok())
            });
            let Some(id) = id else { continue };
            let count = self.counts.entry(id).or_default();
            let answers = &report.responses;
            count.reports += 1;
            count.runs += u32::from(text(&answers.verdict) == "yes");
            for (answer, tally) in [
                (&answers.performance_faults, &mut count.performance),
                (&answers.battery_performance, &mut count.battery),
                (&answers.readability, &mut count.readability),
            ] {
                match text(answer) {
                    "yes" => *tally = (tally.0 + 1, tally.1 + 1),
                    "no" => tally.1 += 1,
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tar(json: &str) -> Vec<u8> {
        let mut header = [0u8; 512];
        header[..20].copy_from_slice(b"reports_piiremoved.j");
        header[124..136].copy_from_slice(format!("{:011o}\0", json.len()).as_bytes());
        let mut bytes = header.to_vec();
        bytes.extend_from_slice(json.as_bytes());
        bytes.resize(bytes.len().div_ceil(512) * 512 + 1024, 0);
        bytes
    }

    fn report(app: &str, deck: bool, verdict: &str, performance: &str, timestamp: i64) -> String {
        let cpu = if deck {
            "AMD Custom APU 0405"
        } else {
            "AMD Ryzen 7 7700"
        };
        format!(
            r#"{{"app":{{"steam":{{"appId":"{app}"}},"title":"Game"}},"responses":{{"verdict":"{verdict}","performanceFaults":"{performance}","extra":{{"a":[1,2]}}}},"systemInfo":{{"cpu":"{cpu}","os":"Linux"}},"timestamp":{timestamp}}}"#
        )
    }

    #[test]
    fn export_keeps_recent_deck_reports_with_enough_evidence() {
        let mut reports = Vec::new();
        for index in 0..6 {
            let verdict = if index == 0 { "no" } else { "yes" };
            let performance = if index < 3 { "yes" } else { "no" };
            reports.push(report("620", true, verdict, performance, 2_000));
        }
        // Desktop, old, and thin evidence do not count.
        reports.push(report("620", false, "no", "yes", 2_000));
        reports.push(report("620", true, "no", "yes", 10));
        reports.push(report("70", true, "yes", "no", 2_000));
        reports.push(r#"{"broken": true}"#.into());
        let json = format!("[{}]", reports.join(","));
        let apps = read_export(tar(&json).as_slice(), 1_000).unwrap();
        assert_eq!(apps.len(), 1);
        let portal = apps[&620];
        assert_eq!(portal.reports, 6);
        assert_eq!(portal.runs_percent, 83);
        assert_eq!(portal.performance_percent, Some(50));
        assert_eq!(portal.battery_percent, None);
    }

    #[test]
    fn export_names_give_dates() {
        assert_eq!(export_date("reports_oct1_2026.tar.gz"), Some((2026, 10, 1)));
        assert_eq!(export_date("reports_sep2_2022.tar.gz"), Some((2022, 9, 2)));
        assert_eq!(export_date("notes.txt"), None);
        // 2026-10-01T00:00:00Z
        assert_eq!(
            export_seconds("reports_oct1_2026.tar.gz"),
            Some(1_790_812_800)
        );
        let data = ProtonData {
            source: "reports_oct1_2026.tar.gz".into(),
            checked_at: 0,
            apps: BTreeMap::new(),
        };
        assert_eq!(data.export_label(), "October 1, 2026");
    }
}
