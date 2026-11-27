//! Firmware Information & Official Update Center (fwupd / LVFS).
//! Never downloads or flashes unofficial firmware.

use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareStatus {
    pub current_version: String,
    pub hardware_revision: String,
    pub fwupd_available: bool,
    pub lvfs_updates_available: Vec<String>,
    pub compatibility_warnings: Vec<String>,
    pub official_instructions: String,
}

pub fn inspect_firmware(modem: &ModemInfo) -> FirmwareStatus {
    let fwupd_available = crate::platform::command_exists("fwupdmgr");
    let mut lvfs_updates = Vec::new();
    if fwupd_available
        && let Ok(out) = Command::new("fwupdmgr")
            .args(["get-updates", "--json"])
            .output()
        && out.status.success()
        && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout)
        && let Some(devices) = val.get("Devices").and_then(|d| d.as_array())
    {
        for dev in devices {
            let name = dev.get("Name").and_then(|v| v.as_str()).unwrap_or("");
            let ver = dev
                .get("Releases")
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
                .and_then(|r| r.get("Version"))
                .and_then(|v| v.as_str());
            if let Some(v) = ver
                && (name.to_ascii_lowercase().contains("modem")
                    || name.to_ascii_lowercase().contains("fibocom")
                    || name.to_ascii_lowercase().contains("quectel")
                    || name.to_ascii_lowercase().contains("sierra")
                    || name.to_ascii_lowercase().contains("dw58")
                    || name.to_ascii_lowercase().contains("dw59")
                    || name.to_ascii_lowercase().contains("foxconn"))
            {
                lvfs_updates.push(format!("{name} -> {v}"));
            }
        }
    }

    let hw_db = crate::hardware::HardwareDatabase::global();
    let entry = hw_db.lookup(
        &modem.manufacturer,
        &modem.model,
        modem.usb_vid,
        modem.usb_pid,
        modem.pci_vid,
        modem.pci_pid,
    );
    let mut warnings = Vec::new();
    if let Some(e) = entry {
        if !e.firmware_restrictions.is_empty() {
            warnings.push(e.firmware_restrictions.clone());
        }
        warnings.extend(e.known_issues.clone());
    }

    FirmwareStatus {
        current_version: modem.revision.clone(),
        hardware_revision: modem.hardware_revision.clone(),
        fwupd_available,
        lvfs_updates_available: lvfs_updates,
        compatibility_warnings: warnings,
        official_instructions: "Use `fwupdmgr refresh && fwupdmgr get-updates` for LVFS-signed updates, or your laptop OEM's official BIOS/WWAN capsule update.".into(),
    }
}
