use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{env, fs, io, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub modem_name: String,
    pub operator: String,
    pub state: String,
    pub network: String,
    pub signal_percent: u32,
    pub band: String,
    pub carrier_aggregation: bool,
    pub updated_unix: u64,
}

pub fn runtime_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| env::temp_dir().join("modemdeck-runtime"))
}

pub fn status_path() -> PathBuf { runtime_dir().join("modemdeck-status.json") }

pub fn status_from_modems(modems: &[ModemInfo]) -> RuntimeStatus {
    let modem = modems.iter().find(|modem| modem.state == 11).or_else(|| modems.first());
    let Some(modem) = modem else {
        return RuntimeStatus { state:"No modem".into(), network:"—".into(), updated_unix:now_unix(), ..RuntimeStatus::default() };
    };
    let band = modem.vendor.serving_band.map(|value| format!("B{value}")).or_else(|| {
        modem.serving_cells.iter().find_map(|cell| {
            cell.band.map(|band| match cell.cell_type { 6 => format!("n{band}"), _ => format!("B{band}") })
        })
    }).unwrap_or_default();

    RuntimeStatus {
        modem_name: modem.display_name(),
        operator: modem.operator_label(),
        state: modem.state_label().into(),
        network: modem.network_label(),
        signal_percent: modem.signal_percent,
        band,
        carrier_aggregation: modem.carrier_aggregation_active(),
        updated_unix: now_unix(),
    }
}

pub fn publish(status: &RuntimeStatus) -> io::Result<()> {
    let path = status_path();
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
    let temp=path.with_extension("json.tmp");
    let payload=serde_json::to_vec(status).map_err(|error|io::Error::new(io::ErrorKind::Other,error.to_string()))?;
    fs::write(&temp,payload)?;
    fs::rename(temp,path)
}

fn now_unix()->u64{SystemTime::now().duration_since(UNIX_EPOCH).map(|duration|duration.as_secs()).unwrap_or(0)}
