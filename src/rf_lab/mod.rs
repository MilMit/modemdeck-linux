//! Advanced RF Intelligence, Hardware Link Inspector, Anomaly Detector,
//! Auto-Band Autopilot, and Offline HTML Diagnostic Health Card.

use crate::{
    doctor,
    model::ModemInfo,
    storage::history_db::{self, HistoryFilter, TimeRange},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalReport {
    pub max_temp_c: Option<f64>,
    pub modem_zone_temp_c: Option<f64>,
    pub throttling_risk: String,
}

pub fn inspect_thermal() -> ThermalReport {
    let mut max_temp: Option<f64> = None;
    let mut modem_temp: Option<f64> = None;
    if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
        for entry in entries.flatten() {
            let p = entry.path();
            let kind = fs::read_to_string(p.join("type"))
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let temp_mc = fs::read_to_string(p.join("temp"))
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok());
            if let Some(mc) = temp_mc {
                let c = mc / 1000.0;
                if (0.0..=125.0).contains(&c) {
                    max_temp = Some(max_temp.map_or(c, |m| m.max(c)));
                    if kind.contains("wwan") || kind.contains("modem") || kind.contains("sdx") {
                        modem_temp = Some(c);
                    }
                }
            }
        }
    }
    let ref_t = modem_temp.or(max_temp);
    let risk = match ref_t {
        Some(t) if t >= 82.0 => "High (Risk of thermal throttling / USB/PCIe reset)",
        Some(t) if t >= 68.0 => "Moderate (Warm under sustained load)",
        Some(_) => "Normal",
        None => "Unavailable",
    }
    .to_string();
    ThermalReport {
        max_temp_c: max_temp,
        modem_zone_temp_c: modem_temp,
        throttling_risk: risk,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusLinkInspection {
    pub bus_type: String,
    pub speed_label: String,
    pub bottleneck_warning: Option<String>,
    pub runtime_pm: String,
}

pub fn inspect_bus_link(modem: &ModemInfo) -> BusLinkInspection {
    for iface in &modem.net_ports {
        let dev = Path::new("/sys/class/net").join(iface).join("device");
        if !dev.exists() {
            continue;
        }
        let pm = fs::read_to_string(dev.join("power/control"))
            .unwrap_or_else(|_| "unknown".into())
            .trim()
            .to_string();

        // Check PCIe link speed
        if let Ok(pci_speed) = fs::read_to_string(dev.join("current_link_speed")) {
            return BusLinkInspection {
                bus_type: "PCIe / MHI".into(),
                speed_label: pci_speed.trim().to_string(),
                bottleneck_warning: None,
                runtime_pm: pm,
            };
        }

        // Check USB speed on device or parent
        for p in [dev.clone(), dev.join("..")] {
            if let Ok(usb_speed) = fs::read_to_string(p.join("speed")) {
                let spd = usb_speed.trim();
                let bottleneck = if spd == "480" || spd == "12" {
                    Some(format!(
                        "Modem is running at USB 2.0 ({spd} Mbps) which can bottleneck LTE-A/5G speeds; check USB 3.0 port/cable."
                    ))
                } else {
                    None
                };
                return BusLinkInspection {
                    bus_type: "USB".into(),
                    speed_label: format!("{spd} Mbps"),
                    bottleneck_warning: bottleneck,
                    runtime_pm: pm,
                };
            }
        }
    }

    BusLinkInspection {
        bus_type: if !modem.pci_vid.is_none() {
            "PCIe".into()
        } else {
            "Unknown".into()
        },
        speed_label: "Not exposed by sysfs".into(),
        bottleneck_warning: None,
        runtime_pm: "unknown".into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfAnomalyReport {
    pub anomaly_detected: bool,
    pub level: String,
    pub summary: String,
    pub disclaimer: String,
}

/// Detects unexpected 2G GSM downgrades or abrupt cell/signal jumps from recent history.
pub fn detect_cell_anomalies(modem: &ModemInfo) -> RfAnomalyReport {
    let records = history_db::query(
        TimeRange::LastHour,
        &HistoryFilter {
            modem_uid: Some(modem.stable_id()),
            ..HistoryFilter::default()
        },
    );
    let mut saw_4g_5g = false;
    for r in &records {
        if r.band.starts_with('B') || r.band.starts_with('n') || r.rsrp.is_some() {
            saw_4g_5g = true;
            break;
        }
    }
    let now_net = modem.network_label();
    let is_2g_now = now_net.starts_with("2G");

    let (anomaly, level, summary) = if saw_4g_5g && is_2g_now && modem.signal_percent >= 80 {
        (
            true,
            "Warning",
            "Sudden downgrade from 4G/5G to 2G GSM with unusually high signal strength detected.",
        )
    } else if saw_4g_5g && is_2g_now {
        (
            true,
            "Notice",
            "Modem downgraded from 4G/5G to 2G GSM in the last hour.",
        )
    } else {
        (
            false,
            "Normal",
            "No unexpected 2G downgrade or abrupt RF anomaly observed in recent history.",
        )
    };

    RfAnomalyReport {
        anomaly_detected: anomaly,
        level: level.into(),
        summary: summary.into(),
        disclaimer: "Note: RF changes can also result from normal carrier maintenance or indoor coverage; this is a heuristic indicator, not proof of an attack.".into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaEfficiencyReport {
    pub total_bw_mhz: Option<f64>,
    pub efficiency_label: String,
    pub note: String,
}

pub fn evaluate_ca_efficiency(
    modem: &ModemInfo,
    measured_dl_mbps: Option<f64>,
) -> CaEfficiencyReport {
    let bw = modem.ca_component_bandwidth_mhz();
    match (bw, measured_dl_mbps) {
        (Some(mhz), Some(mbps)) if mhz > 0.0 => {
            let ratio = mbps / mhz;
            let label = if ratio >= 2.5 {
                "High Efficiency (Low Congestion)"
            } else if ratio >= 1.0 {
                "Moderate Efficiency"
            } else {
                "Low Efficiency (Possible Tower Congestion)"
            };
            CaEfficiencyReport {
                total_bw_mhz: Some(mhz),
                efficiency_label: label.into(),
                note: format!("{mbps:.1} Mbps across {mhz:.0} MHz ({ratio:.2} Mbps/MHz)."),
            }
        }
        (Some(mhz), None) => CaEfficiencyReport {
            total_bw_mhz: Some(mhz),
            efficiency_label: "Bandwidth Known (Run Speed Test to Score)".into(),
            note: format!("Aggregated carrier bandwidth: {mhz:.0} MHz."),
        },
        _ => CaEfficiencyReport {
            total_bw_mhz: None,
            efficiency_label: "Unavailable".into(),
            note: "Aggregated component bandwidth not reported by this modem.".into(),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotRecommendation {
    pub best_band: String,
    pub profile: String,
    pub reason: String,
}

/// Suggests the best band from recorded 7-day history for the current hour window.
pub fn autopilot_recommend(modem: &ModemInfo, profile_mode: &str) -> AutopilotRecommendation {
    let records = history_db::query(
        TimeRange::Last7Days,
        &HistoryFilter {
            modem_uid: Some(modem.stable_id()),
            ..HistoryFilter::default()
        },
    );
    let mut by_band: HashMap<String, (f64, usize)> = HashMap::new();
    for r in &records {
        if r.band.trim().is_empty() || r.band == "—" {
            continue;
        }
        let score = r.signal_percent as f64 + r.snr.unwrap_or(0.0) * 1.5;
        let entry = by_band.entry(r.band.clone()).or_insert((0.0, 0));
        entry.0 += score;
        entry.1 += 1;
    }
    if let Some((band, (sum, cnt))) = by_band
        .into_iter()
        .max_by(|a, b| (a.1.0 / a.1.1 as f64).total_cmp(&(b.1.0 / b.1.1 as f64)))
    {
        return AutopilotRecommendation {
            best_band: band.clone(),
            profile: profile_mode.into(),
            reason: format!(
                "Selected {band} from {cnt} historical samples (avg score {:.1}) for {profile_mode}.",
                sum / cnt as f64
            ),
        };
    }
    AutopilotRecommendation {
        best_band: "Auto".into(),
        profile: profile_mode.into(),
        reason: "Insufficient multi-band history yet; keeping Auto.".into(),
    }
}

/// Generates a single-file offline HTML diagnostic health card (privacy-safe: no IMEI/IMSI/ICCID).
pub fn export_html_health_card(modem: &ModemInfo, dest: &Path) -> Result<(), String> {
    let doc = doctor::run_doctor(std::slice::from_ref(modem));
    let bus = inspect_bus_link(modem);
    let therm = inspect_thermal();
    let (_, snap) = modem.preferred_snapshot();
    let checks_html: String = doc
        .checks
        .iter()
        .map(|c| {
            format!(
                "<tr><td>{}</td><td><b>{}</b></td><td>{}</td></tr>",
                html_escape(&c.title),
                html_escape(c.severity.label()),
                html_escape(&c.explanation)
            )
        })
        .collect();

    let html = format!(
        "<!doctype html><html lang='en'><head><meta charset='utf-8'>\
         <meta name='viewport' content='width=device-width,initial-scale=1'>\
         <title>ModemDeck Health Card</title>\
         <style>body{{font-family:system-ui,sans-serif;background:#0b0f17;color:#e2e8f0;margin:24px;}}\
         .card{{background:#141b29;border:1px solid #293548;border-radius:14px;padding:18px;margin-bottom:16px;}}\
         h1,h2{{margin-top:0;color:#38bdf8;}}table{{width:100%;border-collapse:collapse;}}\
         td,th{{text-align:left;padding:8px;border-bottom:1px solid #243044;}}</style></head><body>\
         <div class='card'><h1>ModemDeck Health Card (Privacy-Safe)</h1>\
         <p>IMEI, IMSI, ICCID, EID, and credentials are intentionally excluded.</p>\
         <p><b>Modem:</b> {} | <b>Operator:</b> {} | <b>Network:</b> {} ({}%)</p>\
         <p><b>RSRP:</b> {:?} | <b>RSRQ:</b> {:?} | <b>SINR:</b> {:?}</p>\
         <p><b>Bus Link:</b> {} ({}) | <b>Thermal:</b> {}</p></div>\
         <div class='card'><h2>Modem Doctor ({})</h2><table><tr><th>Check</th><th>Status</th><th>Details</th></tr>{}</table></div>\
         </body></html>",
        html_escape(&modem.display_name()),
        html_escape(&modem.operator_label()),
        html_escape(&modem.network_label()),
        modem.signal_percent,
        snap.signal.rsrp,
        snap.signal.rsrq,
        snap.signal.snr,
        html_escape(&bus.bus_type),
        html_escape(&bus.speed_label),
        html_escape(&therm.throttling_risk),
        html_escape(doc.overall.label()),
        checks_html
    );
    fs::write(dest, html).map_err(|e| e.to_string())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
