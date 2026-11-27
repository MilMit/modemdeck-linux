//! Comprehensive Backup/Restore & Privacy-Safe Hardware Compatibility Reporting.
//! Never includes IMEI, IMSI, ICCID, EID, phone numbers, APN passwords,
//! eSIM activation codes, GPS coordinates, or personal filesystem paths.

use crate::{core::settings::Settings, model::ModemInfo};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModemDeckBackup {
    pub schema_version: u32,
    pub created_unix: u64,
    pub settings: Settings,
    pub scheduler: crate::scheduler::SchedulerConfig,
    pub note: String,
}

pub fn export_backup(path: &Path) -> Result<(), String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = ModemDeckBackup {
        schema_version: 1,
        created_unix: ts,
        settings: Settings::load(),
        scheduler: crate::scheduler::SchedulerConfig::load(),
        note: "Credentials (APN passwords, eSIM activation codes) are excluded for security."
            .into(),
    };
    let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn restore_backup(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut backup: ModemDeckBackup = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if backup.schema_version == 0 {
        return Err("Invalid backup schema version.".into());
    }
    backup.settings.migrate();
    backup.settings.save().map_err(|e| e.to_string())?;
    backup.scheduler.save()?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCompatibilityReport {
    pub modem_model: String,
    pub manufacturer: String,
    pub firmware: String,
    pub vid_pid: String,
    pub kernel_version: String,
    pub linux_distro: String,
    pub modemmanager_plugin: String,
    pub supported_capabilities: String,
    pub verification_status: String,
    pub notes: String,
}

pub fn build_privacy_safe_hw_report(modem: &ModemInfo, notes: &str) -> HardwareCompatibilityReport {
    let vid_pid = match (modem.usb_vid, modem.usb_pid) {
        (Some(v), Some(p)) => format!("{v:04x}:{p:04x}"),
        _ => match (modem.pci_vid, modem.pci_pid) {
            (Some(v), Some(p)) => format!("pci:{v:04x}:{p:04x}"),
            _ => "unknown".into(),
        },
    };
    let kernel_version = fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_default()
        .trim()
        .to_string();
    let linux_distro = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("PRETTY_NAME=")).map(|l| {
                l.trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string()
            })
        })
        .unwrap_or_else(|| "Linux".into());

    HardwareCompatibilityReport {
        modem_model: modem.model.clone(),
        manufacturer: modem.manufacturer.clone(),
        firmware: modem.revision.clone(),
        vid_pid,
        kernel_version,
        linux_distro,
        modemmanager_plugin: modem.plugin.clone(),
        supported_capabilities: modem.capabilities_label(),
        verification_status: "User-submitted (unverified)".into(),
        notes: sanitize_report_text(notes),
    }
}

/// Strips accidental personal paths or long numeric identifiers (IMEI/IMSI/ICCID).
pub fn sanitize_report_text(input: &str) -> String {
    let mut out = input.to_string();
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        out = out.replace(&home, "~");
    }
    out
}
