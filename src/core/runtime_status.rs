use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RuntimeStatus {
    pub modem_name: String,
    pub operator: String,
    pub state: String,
    pub network: String,
    pub signal_percent: u32,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
    pub band: String,
    pub carrier_aggregation: bool,
    pub lock_mode: String,
    pub internet_health: String,
    pub rx_rate_bps: u64,
    pub tx_rate_bps: u64,
    pub data_usage_bytes: u64,
    pub unread_sms: u32,
    pub updated_unix: u64,
}

pub fn runtime_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("modemdeck-runtime"))
}

pub fn status_path() -> PathBuf {
    runtime_dir().join("modemdeck-status.json")
}

pub fn status_from_modems(modems: &[ModemInfo]) -> RuntimeStatus {
    let modem = modems
        .iter()
        .find(|modem| modem.state == 11)
        .or_else(|| modems.first());
    let Some(modem) = modem else {
        return RuntimeStatus {
            state: "No modem".into(),
            network: "—".into(),
            lock_mode: "—".into(),
            internet_health: "Unknown".into(),
            updated_unix: now_unix(),
            ..RuntimeStatus::default()
        };
    };
    let band = modem
        .detected_serving_lte_band()
        .map(|value| format!("B{value}"))
        .or_else(|| {
            modem
                .detected_serving_nr_band()
                .map(|value| format!("n{value}"))
        })
        .unwrap_or_default();

    let (_, snap) = modem.preferred_snapshot();
    let traffic_sample = crate::services::traffic::infer_interface(&modem.net_ports)
        .and_then(|iface| crate::services::traffic::sample(&iface));
    let (rx_rate_bps, tx_rate_bps, data_usage_bytes) = traffic_sample
        .map(|t| (t.rx_bps as u64, t.tx_bps as u64, t.today_rx + t.today_tx))
        .unwrap_or((0, 0, 0));

    let unread_sms = crate::services::communications::list_sms(&modem.object_path)
        .map(|msgs| {
            msgs.iter()
                .filter(|m| m.state == 3) // MM_SMS_STATE_RECEIVED
                .count() as u32
        })
        .unwrap_or(0);

    RuntimeStatus {
        modem_name: modem.display_name(),
        operator: modem.operator_label(),
        state: modem.state_label().into(),
        network: modem.network_label(),
        signal_percent: modem.signal_percent,
        rsrp: snap.signal.rsrp,
        rsrq: snap.signal.rsrq,
        sinr: snap.signal.snr,
        band,
        carrier_aggregation: modem.carrier_aggregation_active(),
        lock_mode: "Auto".into(),
        internet_health: if modem.state == 11 {
            "Connected"
        } else {
            "Offline"
        }
        .into(),
        rx_rate_bps,
        tx_rate_bps,
        data_usage_bytes,
        unread_sms,
        updated_unix: now_unix(),
    }
}

pub fn publish(status: &RuntimeStatus) -> io::Result<()> {
    let path = status_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    let payload =
        serde_json::to_vec(status).map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(&temp, payload)?;
    fs::rename(temp, path)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
