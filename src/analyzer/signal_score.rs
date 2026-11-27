use crate::model::{RadioSignal, SignalFreshness, SignalSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadioTech {
    Lte,
    Nr5g,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalGrade {
    Excellent,
    VeryGood,
    Good,
    Fair,
    Poor,
    VeryPoor,
    Unknown,
}

impl SignalGrade {
    pub fn label(self) -> &'static str {
        match self {
            Self::Excellent => "Excellent",
            Self::VeryGood => "Very Good",
            Self::Good => "Good",
            Self::Fair => "Fair",
            Self::Poor => "Poor",
            Self::VeryPoor => "Very Poor",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
    None,
}

impl ConfidenceLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::None => "None",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalScoreReport {
    pub tech: RadioTech,
    pub score: Option<u8>,
    pub grade: SignalGrade,
    pub confidence: ConfidenceLevel,
    pub freshness: SignalFreshness,
    pub strength_label: &'static str,
    pub quality_label: &'static str,
    pub interference_label: &'static str,
    pub reasons: Vec<String>,
    pub disclaimer: &'static str,
}

/// Evaluate radio link quality for LTE or 5G NR without confusing raw antenna
/// power (RSRP/RSSI) with interference-free link quality (SNR/SINR & RSRQ).
pub fn evaluate_signal(
    tech: RadioTech,
    snapshot: &SignalSnapshot,
    recent_samples: &[RadioSignal],
) -> SignalScoreReport {
    let signal = &snapshot.signal;
    if !signal.has_any() || snapshot.freshness == SignalFreshness::Unavailable {
        return SignalScoreReport {
            tech,
            score: None,
            grade: SignalGrade::Unknown,
            confidence: ConfidenceLevel::None,
            freshness: SignalFreshness::Unavailable,
            strength_label: "Unknown",
            quality_label: "Unknown",
            interference_label: "Unknown",
            reasons: vec![
                "No extended radio metrics (RSRP/RSRQ/SNR/RSSI) are currently available.".into(),
            ],
            disclaimer: "Radio score reflects RF link quality only, not actual internet throughput.",
        };
    }

    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;
    let mut reasons = Vec::new();

    let (rsrp_w, rsrq_w, snr_w, rssi_w) = match tech {
        RadioTech::Lte => (0.28, 0.28, 0.34, 0.10),
        RadioTech::Nr5g => (0.26, 0.26, 0.38, 0.10),
    };

    let rsrp_sub = signal.rsrp.map(|v| score_rsrp(tech, v));
    if let Some(s) = rsrp_sub {
        weighted_sum += s * rsrp_w;
        total_weight += rsrp_w;
    }

    let rsrq_sub = signal.rsrq.map(|v| score_rsrq(tech, v));
    if let Some(s) = rsrq_sub {
        weighted_sum += s * rsrq_w;
        total_weight += rsrq_w;
    }

    let snr_sub = signal.snr.map(|v| score_snr(tech, v));
    if let Some(s) = snr_sub {
        weighted_sum += s * snr_w;
        total_weight += snr_w;
    }

    let rssi_sub = signal.rssi.map(score_rssi);
    if let Some(s) = rssi_sub {
        weighted_sum += s * rssi_w;
        total_weight += rssi_w;
    }

    let mut base_score = if total_weight > 0.0 {
        weighted_sum / total_weight
    } else {
        0.0
    };

    // Critical interference & quality guardrails:
    // Strong RSRP with poor SNR or RSRQ must NOT be rated as high quality.
    if let Some(snr) = signal.snr {
        if snr < 0.0 {
            base_score = base_score.min(35.0);
            reasons.push(format!(
                "Severe interference / negative SNR ({snr:.1} dB) caps link quality."
            ));
        } else if snr < 3.0 {
            base_score = base_score.min(52.0);
            reasons.push(format!(
                "High interference (SNR {snr:.1} dB) limits throughput despite signal strength."
            ));
        } else if snr >= 18.0 {
            reasons.push(format!("Clean RF environment (SNR {snr:.1} dB)."));
        }
    }

    if let Some(rsrq) = signal.rsrq {
        if rsrq < -17.0 {
            base_score = base_score.min(44.0);
            reasons.push(format!(
                "Heavy cell load or interference (RSRQ {rsrq:.1} dB)."
            ));
        } else if rsrq < -14.0 {
            base_score = base_score.min(64.0);
            reasons.push(format!(
                "Moderate cell congestion/interference (RSRQ {rsrq:.1} dB)."
            ));
        }
    }

    if let Some(rsrp) = signal.rsrp {
        if rsrp < -112.0 {
            base_score = base_score.min(38.0);
            reasons.push(format!("Weak reference signal (RSRP {rsrp:.1} dBm)."));
        } else if rsrp >= -85.0 && signal.snr.map(|s| s < 5.0).unwrap_or(false) {
            reasons.push(format!(
                "Strong RSRP ({rsrp:.1} dBm) paired with low SNR indicates noisy/congested tower."
            ));
        }
    }

    // Penalize high signal fluctuation across recent samples
    if recent_samples.len() >= 3 {
        let rsrp_vals: Vec<f64> = recent_samples.iter().filter_map(|s| s.rsrp).collect();
        if rsrp_vals.len() >= 3 {
            let std_dev = stddev(&rsrp_vals);
            if std_dev > 6.0 {
                let penalty = ((std_dev - 6.0) * 1.5).min(15.0);
                base_score = (base_score - penalty).max(0.0);
                reasons.push(format!(
                    "Unstable signal fluctuation (σ = {std_dev:.1} dB)."
                ));
            }
        }
    }

    // Freshness adjustment
    match snapshot.freshness {
        SignalFreshness::Fresh => {}
        SignalFreshness::Cached => {
            base_score = (base_score - 4.0).max(0.0);
            reasons.push("Using recently cached signal sample.".into());
        }
        SignalFreshness::Stale => {
            base_score = (base_score - 12.0).max(0.0);
            reasons.push("Signal sample is stale; refresh recommended.".into());
        }
        SignalFreshness::Unavailable => {}
    }

    let final_score = base_score.round().clamp(0.0, 100.0) as u8;
    let grade = match final_score {
        88..=100 => SignalGrade::Excellent,
        75..=87 => SignalGrade::VeryGood,
        60..=74 => SignalGrade::Good,
        42..=59 => SignalGrade::Fair,
        24..=41 => SignalGrade::Poor,
        _ => SignalGrade::VeryPoor,
    };

    let available = signal.available_count();
    let confidence = if available >= 3 && snapshot.freshness == SignalFreshness::Fresh {
        ConfidenceLevel::High
    } else if available >= 2 && snapshot.freshness != SignalFreshness::Stale {
        ConfidenceLevel::Medium
    } else {
        if available < 2 {
            reasons.push("Limited metrics reported by modem; confidence is low.".into());
        }
        ConfidenceLevel::Low
    };

    if reasons.is_empty() {
        reasons.push("Balanced RSRP, RSRQ, and SNR readings.".into());
    }

    SignalScoreReport {
        tech,
        score: Some(final_score),
        grade,
        confidence,
        freshness: snapshot.freshness,
        strength_label: signal
            .rsrp
            .map(|v| rsrp_grade(tech, v))
            .unwrap_or("Unknown"),
        quality_label: signal
            .rsrq
            .map(|v| rsrq_grade(tech, v))
            .unwrap_or("Unknown"),
        interference_label: signal.snr.map(interference_grade).unwrap_or("Unknown"),
        reasons,
        disclaimer: "Radio score reflects RF link quality only, not actual internet throughput.",
    }
}

fn score_rsrp(tech: RadioTech, v: f64) -> f64 {
    let (best, worst) = match tech {
        RadioTech::Lte => (-75.0, -120.0),
        RadioTech::Nr5g => (-75.0, -122.0),
    };
    linear_clamp(v, worst, best) * 100.0
}

fn score_rsrq(tech: RadioTech, v: f64) -> f64 {
    let (best, worst) = match tech {
        RadioTech::Lte => (-6.0, -21.0),
        RadioTech::Nr5g => (-6.0, -22.0),
    };
    linear_clamp(v, worst, best) * 100.0
}

fn score_snr(tech: RadioTech, v: f64) -> f64 {
    let (best, worst) = match tech {
        RadioTech::Lte => (25.0, -5.0),
        RadioTech::Nr5g => (28.0, -5.0),
    };
    linear_clamp(v, worst, best) * 100.0
}

fn score_rssi(v: f64) -> f64 {
    linear_clamp(v, -105.0, -60.0) * 100.0
}

fn linear_clamp(val: f64, min: f64, max: f64) -> f64 {
    ((val - min) / (max - min)).clamp(0.0, 1.0)
}

pub fn rsrp_grade(tech: RadioTech, value: f64) -> &'static str {
    match tech {
        RadioTech::Lte => {
            if value >= -80.0 {
                "Excellent"
            } else if value >= -90.0 {
                "Good"
            } else if value >= -100.0 {
                "Fair"
            } else if value >= -110.0 {
                "Poor"
            } else {
                "Very Poor"
            }
        }
        RadioTech::Nr5g => {
            if value >= -82.0 {
                "Excellent"
            } else if value >= -92.0 {
                "Good"
            } else if value >= -102.0 {
                "Fair"
            } else if value >= -112.0 {
                "Poor"
            } else {
                "Very Poor"
            }
        }
    }
}

pub fn rsrq_grade(_tech: RadioTech, value: f64) -> &'static str {
    if value >= -10.0 {
        "Excellent"
    } else if value >= -14.0 {
        "Good"
    } else if value >= -18.0 {
        "Fair"
    } else {
        "Poor"
    }
}

pub fn interference_grade(value: f64) -> &'static str {
    if value >= 20.0 {
        "Very low"
    } else if value >= 13.0 {
        "Low"
    } else if value >= 3.0 {
        "Moderate"
    } else if value >= 0.0 {
        "High"
    } else {
        "Severe"
    }
}

fn stddev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    var.sqrt()
}
