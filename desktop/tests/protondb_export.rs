//! Optional check against a real ProtonDB export file. Set
//! GAMESYNC_PROTONDB_EXPORT to a downloaded `reports_*.tar.gz`.
use std::io::BufReader;

#[test]
#[ignore = "Needs a downloaded ProtonDB export"]
fn real_export_separates_heavy_and_light_games() {
    let Some(path) = std::env::var_os("GAMESYNC_PROTONDB_EXPORT") else {
        return;
    };
    let file = BufReader::new(std::fs::File::open(path).unwrap());
    let started = std::time::Instant::now();
    let apps =
        gamesync_desktop::protondb::read_export(flate2::read::GzDecoder::new(file), 0).unwrap();
    eprintln!("{} games in {:?}", apps.len(), started.elapsed());
    let cyberpunk = apps[&1091500];
    let balatro = apps[&2379780];
    eprintln!("Cyberpunk {cyberpunk:?}\nBalatro {balatro:?}");
    assert!(apps.len() > 1000);
    assert!(cyberpunk.performance_percent > balatro.performance_percent);
    assert!(cyberpunk.readability_percent > balatro.readability_percent);
}

#[test]
#[ignore = "Downloads the ProtonDB export from GitHub (about 70 MB)"]
fn newest_export_downloads_and_imports() {
    use gamesync_desktop::protondb;
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut last = 0;
    let mut progress = |read: u64, _: Option<u64>| last = read;
    let data = protondb::fetch(None, &cancel, &mut progress, 0).unwrap();
    eprintln!("{} · {} games · {last} bytes", data.source, data.apps.len());
    assert!(data.source.starts_with("reports_"));
    assert!(data.apps.len() > 1000 && last > 10_000_000);
    assert!(data.apps.contains_key(&2379780));
}
