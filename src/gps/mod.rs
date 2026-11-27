//! GPS / GNSS Manager.
//! Disabled by default to preserve battery and user privacy.

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpsFix {
    pub enabled: bool,
    pub has_fix: bool,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude_m: Option<f64>,
    pub accuracy_m: Option<f64>,
    pub speed_mps: Option<f64>,
    pub satellites: Option<u32>,
    pub nmea_raw: String,
    pub timestamp_unix: u64,
}

pub fn set_gps_enabled(modem_index: &str, enable: bool) -> Result<(), String> {
    let flag = if enable {
        "--location-enable-gps-raw"
    } else {
        "--location-disable-gps-raw"
    };
    let out = Command::new("mmcli")
        .args(["-m", modem_index, flag])
        .output()
        .map_err(|e| format!("mmcli failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn read_location(modem_index: &str) -> Result<GpsFix, String> {
    let out = Command::new("mmcli")
        .args(["-m", modem_index, "--location-get"])
        .output()
        .map_err(|e| format!("mmcli failed: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    Ok(parse_mmcli_location(&raw))
}

pub fn parse_mmcli_location(raw: &str) -> GpsFix {
    let mut fix = GpsFix {
        timestamp_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        ..GpsFix::default()
    };
    let mut nmea_lines = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("latitude:") {
            fix.latitude = v.trim().parse::<f64>().ok();
        } else if let Some(v) = t.strip_prefix("longitude:") {
            fix.longitude = v.trim().parse::<f64>().ok();
        } else if let Some(v) = t.strip_prefix("altitude:") {
            fix.altitude_m = v.trim().trim_end_matches('m').trim().parse::<f64>().ok();
        } else if t.starts_with("$GP") || t.starts_with("$GN") {
            nmea_lines.push(t.to_string());
        }
    }
    fix.has_fix = fix.latitude.is_some() && fix.longitude.is_some();
    fix.enabled = fix.has_fix || !nmea_lines.is_empty();
    fix.nmea_raw = nmea_lines.join("\n");
    fix
}

pub fn export_gpx(fixes: &[GpsFix]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <gpx version=\"1.1\" creator=\"ModemDeck by MilMit\">\n  <trk>\n    <name>ModemDeck Track</name>\n    <trkseg>\n",
    );
    for fix in fixes {
        if let (Some(lat), Some(lon)) = (fix.latitude, fix.longitude) {
            out.push_str(&format!("      <trkpt lat=\"{lat}\" lon=\"{lon}\">"));
            if let Some(alt) = fix.altitude_m {
                out.push_str(&format!("<ele>{alt}</ele>"));
            }
            out.push_str("</trkpt>\n");
        }
    }
    out.push_str("    </trkseg>\n  </trk>\n</gpx>\n");
    out
}

pub fn export_csv(fixes: &[GpsFix]) -> String {
    let mut out =
        String::from("timestamp_unix,latitude,longitude,altitude_m,accuracy_m,speed_mps\n");
    for f in fixes {
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            f.timestamp_unix,
            f.latitude.map(|v| v.to_string()).unwrap_or_default(),
            f.longitude.map(|v| v.to_string()).unwrap_or_default(),
            f.altitude_m.map(|v| v.to_string()).unwrap_or_default(),
            f.accuracy_m.map(|v| v.to_string()).unwrap_or_default(),
            f.speed_mps.map(|v| v.to_string()).unwrap_or_default(),
        ));
    }
    out
}
