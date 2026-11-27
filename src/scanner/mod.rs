//! 3GPP Network Scanner (Automatic & Manual Operator Selection + Scan History).

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkAvailability {
    Available,
    Current,
    Forbidden,
    Unknown,
}

impl NetworkAvailability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::Current => "Current",
            Self::Forbidden => "Forbidden",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedNetwork {
    pub operator_name: String,
    pub operator_code: String,
    pub country: String,
    pub technology: String,
    pub availability: NetworkAvailability,
    pub roaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub timestamp_unix: u64,
    pub modem_id: String,
    pub networks: Vec<ScannedNetwork>,
    pub note: String,
}

pub fn scan_disconnect_warning() -> &'static str {
    "Scanning for visible cellular networks may temporarily interrupt an active mobile broadband data session."
}

/// Parses `mmcli -m <idx> --3gpp-scan` output into structured `ScannedNetwork` entries.
pub fn parse_mmcli_scan_output(raw: &str, home_mcc: &str) -> Vec<ScannedNetwork> {
    let mut list = Vec::new();
    let db = crate::operators::OperatorDatabase::global();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        // Typical mmcli output: "43235 - MTN Irancell (lte, current)"
        let (code, rest) = match trimmed.split_once('-') {
            Some((c, r)) => (c.trim(), r.trim()),
            None => continue,
        };
        let (name_part, meta_part) = match rest.rsplit_once('(') {
            Some((n, m)) => (n.trim(), m.trim_end_matches(')').to_ascii_lowercase()),
            None => (rest, String::new()),
        };

        let availability = if meta_part.contains("current") {
            NetworkAvailability::Current
        } else if meta_part.contains("forbidden") {
            NetworkAvailability::Forbidden
        } else if meta_part.contains("available") {
            NetworkAvailability::Available
        } else {
            NetworkAvailability::Unknown
        };

        let tech = if meta_part.contains("5gnr") && meta_part.contains("lte") {
            "5G NSA"
        } else if meta_part.contains("5gnr") {
            "5G SA"
        } else if meta_part.contains("lte") {
            "4G LTE"
        } else if meta_part.contains("umts") || meta_part.contains("hspa") {
            "3G UMTS"
        } else if meta_part.contains("gsm") {
            "2G GSM"
        } else {
            "Unknown"
        }
        .to_string();

        let (mcc, mnc) = crate::operators::split_plmn(code).unwrap_or(("", ""));
        let country = if !mcc.is_empty() {
            db.by_mcc_mnc(mcc, mnc)
                .first()
                .map(|o| o.country.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let roaming = !home_mcc.is_empty() && !mcc.is_empty() && mcc != home_mcc;

        list.push(ScannedNetwork {
            operator_name: name_part.to_string(),
            operator_code: code.to_string(),
            country,
            technology: tech,
            availability,
            roaming,
        });
    }
    list
}

pub fn run_scan(modem_index: &str, home_mcc: &str) -> Result<ScanResult, String> {
    let out = Command::new("mmcli")
        .args(["-m", modem_index, "--3gpp-scan", "--timeout=45"])
        .output()
        .map_err(|e| format!("Failed to run mmcli scan: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let networks = parse_mmcli_scan_output(&raw, home_mcc);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(ScanResult {
        timestamp_unix: ts,
        modem_id: modem_index.to_string(),
        networks,
        note: "Visibility in scan does not guarantee SIM registration authorization.".into(),
    })
}

pub fn register_automatic(modem_index: &str) -> Result<(), String> {
    let out = Command::new("mmcli")
        .args(["-m", modem_index, "--3gpp-register-home"])
        .output()
        .map_err(|e| format!("mmcli failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn register_manual(modem_index: &str, operator_code: &str) -> Result<(), String> {
    if !operator_code.chars().all(|c| c.is_ascii_digit()) || operator_code.len() < 5 {
        return Err("Invalid MCC/MNC operator code.".into());
    }
    let out = Command::new("mmcli")
        .args([
            "-m",
            modem_index,
            &format!("--3gpp-register-in-operator={operator_code}"),
        ])
        .output()
        .map_err(|e| format!("mmcli failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
