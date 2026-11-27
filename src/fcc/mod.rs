//! Official FCC Lock detection and ModemManager fcc-unlock guidance.
//! Never uses undocumented radio-limit bypasses or unauthorized commands.

use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FccLockState {
    Unlocked,
    Locked,
    NotApplicable,
    Unknown,
}

impl FccLockState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unlocked => "Unlocked",
            Self::Locked => "FCC Locked (Low-Power)",
            Self::NotApplicable => "Not Applicable",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FccLockReport {
    pub state: FccLockState,
    pub official_script_present: bool,
    pub script_path: String,
    pub missing_dependencies: Vec<String>,
    pub official_procedure: String,
    pub can_guide_unlock: bool,
}

const MM_FCC_DIRS: &[&str] = &[
    "/etc/ModemManager/fcc-unlock.d",
    "/usr/lib/ModemManager/fcc-unlock.d",
    "/usr/lib/x86_64-linux-gnu/ModemManager/fcc-unlock.d",
    "/usr/share/ModemManager/fcc-unlock.available.d",
];

pub fn inspect_fcc_lock(modem: &ModemInfo) -> FccLockReport {
    let state = if modem.fcc_locked == Some(true) || modem.state == 3 && modem.unlock_required == 1
    {
        if modem.fcc_locked == Some(true) {
            FccLockState::Locked
        } else {
            FccLockState::Unknown
        }
    } else if modem.state >= 6 {
        FccLockState::Unlocked
    } else {
        FccLockState::Unknown
    };

    let vid_pid = match (modem.usb_vid, modem.usb_pid) {
        (Some(v), Some(p)) => Some(format!("{v:04x}:{p:04x}")),
        _ => match (modem.pci_vid, modem.pci_pid) {
            (Some(v), Some(p)) => Some(format!("{v:04x}:{p:04x}")),
            _ => None,
        },
    };

    let mut script_path = String::new();
    let mut official_script_present = false;
    if let Some(ref id) = vid_pid {
        for dir in MM_FCC_DIRS {
            let candidate = Path::new(dir).join(id);
            if candidate.exists() {
                official_script_present = !dir.contains("available.d");
                script_path = candidate.display().to_string();
                if official_script_present {
                    break;
                }
            }
        }
    }

    let mut missing = Vec::new();
    if !crate::platform::command_exists("mmcli") {
        missing.push("mmcli (ModemManager CLI)".into());
    }
    if !crate::platform::command_exists("qmicli")
        && modem.qmi_ports.is_empty()
        && !modem.mbim_ports.is_empty()
    {
        // Optional depending on modem
    }

    let official_procedure = if let Some(ref id) = vid_pid {
        format!(
            "Official ModemManager FCC-unlock procedure for {id}:\n\
             1. Verify an official vendor script exists in /usr/share/ModemManager/fcc-unlock.available.d/{id}.\n\
             2. Enable it by symlinking into /etc/ModemManager/fcc-unlock.d/{id} (requires administrator privilege).\n\
             3. Restart ModemManager (`sudo systemctl restart ModemManager`) and verify modem reaches Enabled/Registered state."
        )
    } else {
        "Identify the modem USB/PCI VID:PID first, then check /usr/share/ModemManager/fcc-unlock.available.d/.".into()
    };

    FccLockReport {
        state,
        official_script_present,
        script_path,
        missing_dependencies: missing,
        official_procedure,
        can_guide_unlock: vid_pid.is_some(),
    }
}

pub fn check_mm_power_state(modem_index: &str) -> Result<String, String> {
    let out = Command::new("mmcli")
        .args(["-m", modem_index])
        .output()
        .map_err(|e| format!("Failed to run mmcli: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Enables the official ModemManager fcc-unlock script via `pkexec` after explicit user consent.
pub fn enable_official_fcc_unlock(vid_pid: &str) -> Result<String, String> {
    if !vid_pid.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
        return Err("Invalid VID:PID format.".into());
    }
    let avail = format!("/usr/share/ModemManager/fcc-unlock.available.d/{vid_pid}");
    if !Path::new(&avail).exists() {
        return Err(format!(
            "No official ModemManager unlock script found at {avail}."
        ));
    }
    let dest = format!("/etc/ModemManager/fcc-unlock.d/{vid_pid}");
    let out = Command::new("pkexec")
        .args([
            "sh",
            "-c",
            &format!(
                "mkdir -p /etc/ModemManager/fcc-unlock.d && ln -sf '{avail}' '{dest}' && systemctl restart ModemManager"
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to launch pkexec: {e}"))?;
    if out.status.success() {
        Ok(format!("Official FCC-unlock enabled for {vid_pid}."))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
