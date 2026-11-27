use crate::analyzer::band_scan::BandTestResult;
use crate::analyzer::signal_score::ConfidenceLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OptimizationProfile {
    #[default]
    Balanced,
    MaximumDownload,
    MaximumUpload,
    Gaming,
    Streaming,
    VideoCalls,
    StableConnection,
    Custom,
}

impl OptimizationProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "Balanced",
            Self::MaximumDownload => "Maximum Download",
            Self::MaximumUpload => "Maximum Upload",
            Self::Gaming => "Gaming",
            Self::Streaming => "Streaming",
            Self::VideoCalls => "Video Calls",
            Self::StableConnection => "Stable Connection",
            Self::Custom => "Custom",
        }
    }

    pub fn weights(self) -> SelectionWeights {
        match self {
            Self::Balanced => SelectionWeights {
                signal: 0.25,
                download: 0.25,
                upload: 0.15,
                latency: 0.20,
                stability: 0.15,
            },
            Self::MaximumDownload => SelectionWeights {
                signal: 0.15,
                download: 0.55,
                upload: 0.05,
                latency: 0.10,
                stability: 0.15,
            },
            Self::MaximumUpload => SelectionWeights {
                signal: 0.15,
                download: 0.10,
                upload: 0.50,
                latency: 0.10,
                stability: 0.15,
            },
            Self::Gaming => SelectionWeights {
                signal: 0.15,
                download: 0.10,
                upload: 0.10,
                latency: 0.45,
                stability: 0.20,
            },
            Self::Streaming => SelectionWeights {
                signal: 0.15,
                download: 0.45,
                upload: 0.05,
                latency: 0.10,
                stability: 0.25,
            },
            Self::VideoCalls => SelectionWeights {
                signal: 0.15,
                download: 0.15,
                upload: 0.25,
                latency: 0.25,
                stability: 0.20,
            },
            Self::StableConnection => SelectionWeights {
                signal: 0.30,
                download: 0.10,
                upload: 0.10,
                latency: 0.15,
                stability: 0.35,
            },
            Self::Custom => SelectionWeights::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SelectionWeights {
    pub signal: f64,
    pub download: f64,
    pub upload: f64,
    pub latency: f64,
    pub stability: f64,
}

impl Default for SelectionWeights {
    fn default() -> Self {
        Self {
            signal: 0.25,
            download: 0.25,
            upload: 0.15,
            latency: 0.20,
            stability: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchGuardRules {
    pub hysteresis_points: f64,
    pub min_improvement_percent: f64,
    pub min_interval_secs: u64,
    pub cooldown_secs: u64,
    pub max_switches_per_hour: u32,
    pub min_confidence: ConfidenceLevel,
}

impl Default for SwitchGuardRules {
    fn default() -> Self {
        Self {
            hysteresis_points: 8.0,
            min_improvement_percent: 12.0,
            min_interval_secs: 600,
            cooldown_secs: 300,
            max_switches_per_hour: 3,
            min_confidence: ConfidenceLevel::Medium,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BandRecommendation {
    pub mm_band_enum: u32,
    pub band_label: String,
    pub composite_score: f64,
    pub reason: String,
}

pub fn recommend_best_band(
    results: &[BandTestResult],
    weights: SelectionWeights,
    min_confidence: ConfidenceLevel,
) -> Option<BandRecommendation> {
    let min_rank = confidence_rank(min_confidence);
    let mut best: Option<BandRecommendation> = None;

    for r in results {
        if confidence_rank(r.confidence) < min_rank {
            continue;
        }
        let sig = r.score.unwrap_or(0) as f64;
        let dl = r.download_mbps.unwrap_or(0.0).min(200.0) / 2.0; // 0..100
        let ul = r.upload_mbps.unwrap_or(0.0).min(100.0); // 0..100
        let lat = r
            .latency_ms
            .map(|ms| (100.0 - (ms / 3.0)).clamp(0.0, 100.0))
            .unwrap_or(50.0);
        let stab = r.stability_score as f64;

        let total = sig * weights.signal
            + dl * weights.download
            + ul * weights.upload
            + lat * weights.latency
            + stab * weights.stability;

        if best
            .as_ref()
            .map(|b| total > b.composite_score)
            .unwrap_or(true)
        {
            best = Some(BandRecommendation {
                mm_band_enum: r.mm_band_enum,
                band_label: r.band_label.clone(),
                composite_score: total,
                reason: format!(
                    "Score {:.1}/100 (Signal {}/100, Stability {}/100{}{})",
                    total,
                    r.score.unwrap_or(0),
                    r.stability_score,
                    r.download_mbps
                        .map(|v| format!(", DL {v:.1} Mbps"))
                        .unwrap_or_default(),
                    r.latency_ms
                        .map(|v| format!(", Ping {v:.0} ms"))
                        .unwrap_or_default(),
                ),
            });
        }
    }
    best
}

fn confidence_rank(c: ConfidenceLevel) -> u8 {
    match c {
        ConfidenceLevel::None => 0,
        ConfidenceLevel::Low => 1,
        ConfidenceLevel::Medium => 2,
        ConfidenceLevel::High => 3,
    }
}
