//! Antenna Analyzer & Positioning Assistant.
//! Never fabricates per-antenna metrics when hardware only reports aggregate signal.

use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AntennaMetrics {
    pub per_antenna_supported: bool,
    pub main_rsrp_dbm: Option<f64>,
    pub diversity_rsrp_dbm: Option<f64>,
    pub mimo_rank: Option<String>,
    pub modulation: Option<String>,
    pub cqi: Option<u32>,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
    pub rssi: Option<f64>,
    pub tx_power_dbm: Option<f64>,
    pub temperature_c: Option<f64>,
    pub note: String,
}

impl AntennaMetrics {
    pub fn from_modem(modem: &ModemInfo) -> Self {
        let (_, snap) = modem.preferred_snapshot();
        let mimo = (!modem.vendor.mimo.trim().is_empty()).then(|| modem.vendor.mimo.clone());
        let modulation =
            (!modem.vendor.modulation.trim().is_empty()).then(|| modem.vendor.modulation.clone());
        Self {
            per_antenna_supported: false,
            main_rsrp_dbm: None,
            diversity_rsrp_dbm: None,
            mimo_rank: mimo,
            modulation,
            cqi: None,
            rsrp: snap.signal.rsrp,
            rsrq: snap.signal.rsrq,
            sinr: snap.signal.snr,
            rssi: snap.signal.rssi,
            tx_power_dbm: None,
            temperature_c: None,
            note: "ModemManager/driver reports aggregate RF metrics; per-antenna RX diversity chains are not exposed separately by this modem.".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSample {
    pub label: String,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
    pub score: f64,
    pub timestamp_unix: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PositioningSession {
    pub baseline: Option<PositionSample>,
    pub positions: Vec<PositionSample>,
}

impl PositioningSession {
    pub fn record(&mut self, label: &str, rsrp: Option<f64>, rsrq: Option<f64>, sinr: Option<f64>) {
        let score = compute_position_score(rsrp, rsrq, sinr);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let sample = PositionSample {
            label: label.to_string(),
            rsrp,
            rsrq,
            sinr,
            score,
            timestamp_unix: ts,
        };
        if self.baseline.is_none() {
            self.baseline = Some(sample.clone());
        }
        self.positions.push(sample);
    }

    pub fn best_position(&self) -> Option<&PositionSample> {
        self.positions
            .iter()
            .max_by(|a, b| a.score.total_cmp(&b.score))
    }

    pub fn average_score(&self) -> Option<f64> {
        if self.positions.is_empty() {
            return None;
        }
        let sum: f64 = self.positions.iter().map(|p| p.score).sum();
        Some(sum / self.positions.len() as f64)
    }

    pub fn fluctuation_range(&self) -> Option<f64> {
        if self.positions.len() < 2 {
            return None;
        }
        let min = self
            .positions
            .iter()
            .map(|p| p.score)
            .fold(f64::INFINITY, f64::min);
        let max = self
            .positions
            .iter()
            .map(|p| p.score)
            .fold(f64::NEG_INFINITY, f64::max);
        Some(max - min)
    }
}

pub fn compute_position_score(rsrp: Option<f64>, rsrq: Option<f64>, sinr: Option<f64>) -> f64 {
    let mut total = 0.0;
    let mut weights = 0.0;
    if let Some(v) = rsrp {
        let norm = ((v + 130.0) / 60.0).clamp(0.0, 1.0) * 100.0;
        total += norm * 0.5;
        weights += 0.5;
    }
    if let Some(v) = sinr {
        let norm = ((v + 5.0) / 30.0).clamp(0.0, 1.0) * 100.0;
        total += norm * 0.35;
        weights += 0.35;
    }
    if let Some(v) = rsrq {
        let norm = ((v + 20.0) / 15.0).clamp(0.0, 1.0) * 100.0;
        total += norm * 0.15;
        weights += 0.15;
    }
    if weights > 0.0 { total / weights } else { 0.0 }
}
