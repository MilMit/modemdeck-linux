use crate::core::settings;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorBenchmarkRecord {
    pub operator_name: String,
    pub operator_code: String,
    pub sim_hash: String,
    pub modem_uid: String,
    pub timestamp_unix: u64,
    pub duration_secs: u64,
    pub best_band: String,
    pub avg_rsrp: Option<f64>,
    pub avg_snr: Option<f64>,
    pub avg_download_mbps: Option<f64>,
    pub avg_upload_mbps: Option<f64>,
    pub avg_ping_ms: Option<f64>,
    pub packet_loss_percent: Option<f64>,
    pub stability_score: u8,
    pub availability_percent: f64,
    pub ca_available: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparisonStore {
    pub records: Vec<OperatorBenchmarkRecord>,
}

pub fn known_iran_operators() -> &'static [(&'static str, &'static str)] {
    &[
        ("43235", "Irancell"),
        ("43211", "MCI (Hamrah-e Aval)"),
        ("43220", "Rightel"),
        ("43293", "SamanTel"),
        ("43208", "Shatel Mobile"),
    ]
}

pub fn resolve_operator_display(code: &str, name: &str) -> String {
    if !name.trim().is_empty() {
        return name.trim().to_string();
    }
    for (mccmnc, label) in known_iran_operators() {
        if code.trim() == *mccmnc {
            return (*label).to_string();
        }
    }
    if !code.trim().is_empty() {
        code.trim().to_string()
    } else {
        "Unknown Operator".into()
    }
}

pub fn path() -> PathBuf {
    settings::data_home().join("modemdeck/operator-comparison.json")
}

pub fn load() -> ComparisonStore {
    fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_record(record: OperatorBenchmarkRecord) -> io::Result<()> {
    let mut store = load();
    store.records.push(record);
    if store.records.len() > 200 {
        let drain = store.records.len() - 200;
        store.records.drain(0..drain);
    }
    let p = path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(&store).map_err(|e| io::Error::other(e.to_string()))?;
    fs::write(&tmp, data)?;
    fs::rename(tmp, p)
}
