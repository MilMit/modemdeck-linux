use crate::analyzer::signal_score::{ConfidenceLevel, RadioTech, evaluate_signal};
use crate::model::{ModemInfo, RadioSignal, SignalSnapshot, band_label, classify_mm_band};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BandTestMode {
    Quick,
    Standard,
    Deep,
    Custom,
    RadioOnly,
    InternetPerformance,
    Stability,
    CarrierAggregation,
}

impl BandTestMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Quick => "Quick Test",
            Self::Standard => "Standard Test",
            Self::Deep => "Deep Test",
            Self::Custom => "Custom Test",
            Self::RadioOnly => "Radio-only Test",
            Self::InternetPerformance => "Internet Performance Test",
            Self::Stability => "Stability Test",
            Self::CarrierAggregation => "Carrier Aggregation Test",
        }
    }

    pub fn includes_speed_test(self) -> bool {
        matches!(
            self,
            Self::Standard | Self::Deep | Self::InternetPerformance
        )
    }

    pub fn includes_latency_test(self) -> bool {
        !matches!(self, Self::RadioOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BandTestOutcome {
    Passed,
    BandNotVerified,
    NoCoverage,
    Failed,
    Cancelled,
    SkippedDataCap,
}

impl BandTestOutcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "Passed",
            Self::BandNotVerified => "Band Not Verified",
            Self::NoCoverage => "Unavailable / No Coverage",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::SkippedDataCap => "Stopped (Data Cap)",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricStats {
    pub avg: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub median: Option<f64>,
    pub std_dev: Option<f64>,
    pub p10: Option<f64>,
    pub p90: Option<f64>,
}

impl MetricStats {
    pub fn from_samples(samples: &[f64]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = sorted.len();
        let sum: f64 = sorted.iter().sum();
        let avg = sum / n as f64;
        let min = sorted[0];
        let max = sorted[n - 1];
        let median = if n.is_multiple_of(2) {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        } else {
            sorted[n / 2]
        };
        let variance = sorted.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / n as f64;
        let p10 = sorted[(n as f64 * 0.10).floor() as usize];
        let p90 = sorted[((n as f64 * 0.90).floor() as usize).min(n - 1)];
        Self {
            avg: Some(avg),
            min: Some(min),
            max: Some(max),
            median: Some(median),
            std_dev: Some(variance.sqrt()),
            p10: Some(p10),
            p90: Some(p90),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandTestResult {
    pub mm_band_enum: u32,
    pub band_label: String,
    pub technology: String,
    pub outcome: BandTestOutcome,
    pub verified_serving_band: bool,
    pub rsrp: MetricStats,
    pub rsrq: MetricStats,
    pub rssi: MetricStats,
    pub snr: MetricStats,
    pub download_mbps: Option<f64>,
    pub upload_mbps: Option<f64>,
    pub latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss_percent: Option<f64>,
    pub uptime_percent: f64,
    pub disconnect_count: u32,
    pub registration_time_secs: Option<f64>,
    pub reconnection_time_secs: Option<f64>,
    pub pci: Option<u32>,
    pub earfcn: Option<u32>,
    pub nrarfcn: Option<u32>,
    pub bandwidth: String,
    pub operator: String,
    pub ca_active: bool,
    pub ca_layout: String,
    pub valid_samples: usize,
    pub missing_samples: usize,
    pub confidence: ConfidenceLevel,
    pub score: Option<u8>,
    pub stability_score: u8,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BandScanSummary {
    pub best_overall: Option<String>,
    pub best_signal: Option<String>,
    pub fastest_download: Option<String>,
    pub fastest_upload: Option<String>,
    pub lowest_latency: Option<String>,
    pub most_stable: Option<String>,
    pub best_gaming: Option<String>,
    pub best_streaming: Option<String>,
    pub best_work: Option<String>,
}

pub fn summarize_results(results: &[BandTestResult]) -> BandScanSummary {
    let valid: Vec<&BandTestResult> = results
        .iter()
        .filter(|r| r.outcome == BandTestOutcome::Passed && r.valid_samples > 0)
        .collect();
    if valid.is_empty() {
        return BandScanSummary::default();
    }

    let best_overall = valid
        .iter()
        .filter_map(|r| r.score.map(|s| (s, &r.band_label)))
        .max_by_key(|(s, _)| *s)
        .map(|(_, l)| l.clone());

    let best_signal = valid
        .iter()
        .filter_map(|r| r.rsrp.avg.map(|v| (v, &r.band_label)))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, l)| l.clone());

    let fastest_download = valid
        .iter()
        .filter_map(|r| r.download_mbps.map(|v| (v, &r.band_label)))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, l)| l.clone());

    let fastest_upload = valid
        .iter()
        .filter_map(|r| r.upload_mbps.map(|v| (v, &r.band_label)))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, l)| l.clone());

    let lowest_latency = valid
        .iter()
        .filter_map(|r| r.latency_ms.map(|v| (v, &r.band_label)))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, l)| l.clone());

    let most_stable = valid
        .iter()
        .max_by_key(|r| r.stability_score)
        .map(|r| r.band_label.clone());

    let best_gaming = valid
        .iter()
        .filter(|r| r.latency_ms.is_some())
        .min_by(|a, b| {
            let sa = a.latency_ms.unwrap_or(999.0)
                + a.jitter_ms.unwrap_or(0.0) * 2.0
                + a.packet_loss_percent.unwrap_or(0.0) * 10.0;
            let sb = b.latency_ms.unwrap_or(999.0)
                + b.jitter_ms.unwrap_or(0.0) * 2.0
                + b.packet_loss_percent.unwrap_or(0.0) * 10.0;
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|r| r.band_label.clone());

    let best_streaming = valid
        .iter()
        .filter(|r| r.download_mbps.is_some())
        .max_by(|a, b| {
            let sa = a.download_mbps.unwrap_or(0.0) * (a.stability_score as f64 / 100.0);
            let sb = b.download_mbps.unwrap_or(0.0) * (b.stability_score as f64 / 100.0);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|r| r.band_label.clone());

    let best_work = valid
        .iter()
        .max_by(|a, b| {
            let sa = (a.stability_score as f64) + a.upload_mbps.unwrap_or(0.0)
                - a.packet_loss_percent.unwrap_or(0.0) * 5.0;
            let sb = (b.stability_score as f64) + b.upload_mbps.unwrap_or(0.0)
                - b.packet_loss_percent.unwrap_or(0.0) * 5.0;
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|r| r.band_label.clone());

    BandScanSummary {
        best_overall,
        best_signal,
        fastest_download,
        fastest_upload,
        lowest_latency,
        most_stable,
        best_gaming,
        best_streaming,
        best_work,
    }
}

pub fn verify_serving_band(requested_mm_band: u32, modem: &ModemInfo) -> Option<bool> {
    let info = classify_mm_band(requested_mm_band);
    let target_num = info.number?;
    if let Some(serving_lte) = modem.detected_serving_lte_band() {
        return Some(serving_lte == target_num);
    }
    if let Some(serving_nr) = modem.detected_serving_nr_band() {
        return Some(serving_nr == target_num);
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub fn build_band_result_from_samples(
    mm_band_enum: u32,
    modem: &ModemInfo,
    samples: &[RadioSignal],
    missing_samples: usize,
    disconnect_count: u32,
    uptime_percent: f64,
    registration_time_secs: Option<f64>,
    reconnection_time_secs: Option<f64>,
    download_mbps: Option<f64>,
    upload_mbps: Option<f64>,
    latency_ms: Option<f64>,
    jitter_ms: Option<f64>,
    packet_loss_percent: Option<f64>,
    outcome: BandTestOutcome,
    verified_serving_band: bool,
    note: String,
) -> BandTestResult {
    let rsrp_vals: Vec<f64> = samples.iter().filter_map(|s| s.rsrp).collect();
    let rsrq_vals: Vec<f64> = samples.iter().filter_map(|s| s.rsrq).collect();
    let rssi_vals: Vec<f64> = samples.iter().filter_map(|s| s.rssi).collect();
    let snr_vals: Vec<f64> = samples.iter().filter_map(|s| s.snr).collect();

    let rsrp = MetricStats::from_samples(&rsrp_vals);
    let rsrq = MetricStats::from_samples(&rsrq_vals);
    let rssi = MetricStats::from_samples(&rssi_vals);
    let snr = MetricStats::from_samples(&snr_vals);

    let avg_signal = RadioSignal {
        rsrp: rsrp.avg,
        rsrq: rsrq.avg,
        rssi: rssi.avg,
        snr: snr.avg,
    };
    let (_, snap) = modem.preferred_snapshot();
    let avg_snap = SignalSnapshot {
        signal: avg_signal,
        updated_unix: snap.updated_unix,
        freshness: snap.freshness,
        context_key: snap.context_key.clone(),
    };
    let tech = if modem.is_5g_active() {
        RadioTech::Nr5g
    } else {
        RadioTech::Lte
    };
    let score_report = evaluate_signal(tech, &avg_snap, samples);

    let stability_raw = (uptime_percent
        - (disconnect_count as f64 * 15.0)
        - packet_loss_percent.unwrap_or(0.0) * 2.0)
        .round()
        .clamp(0.0, 100.0) as u8;

    BandTestResult {
        mm_band_enum,
        band_label: band_label(mm_band_enum),
        technology: modem.network_label(),
        outcome,
        verified_serving_band,
        rsrp,
        rsrq,
        rssi,
        snr,
        download_mbps,
        upload_mbps,
        latency_ms,
        jitter_ms,
        packet_loss_percent,
        uptime_percent,
        disconnect_count,
        registration_time_secs,
        reconnection_time_secs,
        pci: modem.vendor.pci,
        earfcn: modem.vendor.earfcn,
        nrarfcn: modem.vendor.nrarfcn,
        bandwidth: modem.vendor.bandwidth.clone(),
        operator: modem.operator_label(),
        ca_active: modem.carrier_aggregation_active(),
        ca_layout: modem.ca_layout_label(),
        valid_samples: samples.len(),
        missing_samples,
        confidence: score_report.confidence,
        score: score_report.score,
        stability_score: stability_raw,
        note,
    }
}
