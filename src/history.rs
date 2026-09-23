use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io, time::{SystemTime, UNIX_EPOCH}};

const MAX_SAMPLES_PER_MODEM: usize = 180;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignalSample {
    pub timestamp: u64,
    pub signal_percent: u32,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub snr: Option<f64>,
}

pub type HistoryStore = HashMap<String, Vec<SignalSample>>;

fn history_path() -> std::path::PathBuf { crate::runtime_status::runtime_dir().join("modemdeck-history.json") }

pub fn record(modems: &[ModemInfo]) {
    let path = history_path(); let mut store = load_store();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0);
    for modem in modems {
        let key = history_key(modem); let (_, signal) = modem.preferred_signal();
        let sample = SignalSample { timestamp: now, signal_percent: modem.signal_percent, rsrp: signal.rsrp, rsrq: signal.rsrq, snr: signal.snr };
        let samples = store.entry(key).or_default();
        if samples.last().map(|previous| previous.timestamp == now && previous.signal_percent == sample.signal_percent).unwrap_or(false) { continue; }
        samples.push(sample);
        if samples.len() > MAX_SAMPLES_PER_MODEM { let remove = samples.len() - MAX_SAMPLES_PER_MODEM; samples.drain(0..remove); }
    }
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    if let Ok(payload) = serde_json::to_vec(&store) { let temp = path.with_extension("json.tmp"); if fs::write(&temp, payload).is_ok() { let _ = fs::rename(temp, path); } }
}

pub fn load_for(modem: &ModemInfo) -> Vec<SignalSample> { load_store().remove(&history_key(modem)).unwrap_or_default() }
pub fn clear() -> io::Result<()> { match fs::remove_file(history_path()) { Ok(()) => Ok(()), Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()), Err(error) => Err(error) } }
fn load_store() -> HistoryStore { fs::read(history_path()).ok().and_then(|payload| serde_json::from_slice::<HistoryStore>(&payload).ok()).unwrap_or_default() }
fn history_key(modem: &ModemInfo) -> String { if !modem.device_uid.trim().is_empty() { modem.device_uid.clone() } else { modem.object_path.clone() } }
