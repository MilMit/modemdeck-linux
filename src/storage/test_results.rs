use crate::analyzer::band_scan::{BandScanSummary, BandTestResult};
use crate::analyzer::speed_test::SpeedTestResult;
use crate::core::settings;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedBandScan {
    pub timestamp_unix: u64,
    pub modem_name: String,
    pub modem_uid: String,
    pub operator: String,
    pub sim_hash: String,
    pub mode: String,
    pub duration_per_band_secs: u64,
    pub results: Vec<BandTestResult>,
    pub summary: BandScanSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestResultsStore {
    pub band_scans: Vec<SavedBandScan>,
    pub speed_tests: Vec<(u64, SpeedTestResult)>,
}

pub fn path() -> PathBuf {
    settings::data_home().join("modemdeck/test-results.json")
}

pub fn load() -> TestResultsStore {
    fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(store: &TestResultsStore) -> io::Result<()> {
    let p = path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(store).map_err(|e| io::Error::other(e.to_string()))?;
    fs::write(&tmp, data)?;
    fs::rename(tmp, p)
}

pub fn save_band_scan(scan: SavedBandScan) -> io::Result<()> {
    let mut store = load();
    store.band_scans.push(scan);
    if store.band_scans.len() > 30 {
        let extra = store.band_scans.len() - 30;
        store.band_scans.drain(0..extra);
    }
    save(&store)
}

pub fn save_speed_test(timestamp: u64, result: SpeedTestResult) -> io::Result<()> {
    let mut store = load();
    store.speed_tests.push((timestamp, result));
    if store.speed_tests.len() > 100 {
        let extra = store.speed_tests.len() - 100;
        store.speed_tests.drain(0..extra);
    }
    save(&store)
}
