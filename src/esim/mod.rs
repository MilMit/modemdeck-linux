//! eSIM / eUICC Manager using `lpac` and ModemManager SIM/eUICC detection.
//! Never logs sensitive Activation Codes or EIDs in diagnostics/logs.

use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsimProfile {
    pub iccid_masked: String,
    pub nickname: String,
    pub provider: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsimCapabilityStatus {
    pub hardware_capable: Option<bool>,
    pub lpac_installed: bool,
    pub eid_detected: bool,
    pub reason: String,
}

pub fn detect_esim_capability(modem: &ModemInfo) -> EsimCapabilityStatus {
    let lpac_installed = crate::platform::command_exists("lpac");
    let lower = modem.model.to_ascii_lowercase();

    // Accurate hardware/firmware reality for Dell DW5821e and DW5829e
    if lower.contains("dw5821e") || lower.contains("t77w968") {
        let is_esim_sku = modem.usb_pid == Some(0x81e0) || modem.esim_supported == Some(true);
        return EsimCapabilityStatus {
            hardware_capable: Some(is_esim_sku),
            lpac_installed,
            eid_detected: !modem.eid.is_empty(),
            reason: if is_esim_sku {
                "DW5821e-eSIM (413c:81e0) hardware detected; requires lpac/QMI UIM APDU backend."
                    .into()
            } else {
                "Standard DW5821e (413c:81d7) uses physical SIM only; integrated eUICC is only present on DW5821e-eSIM SKU (413c:81e0).".into()
            },
        };
    }
    if lower.contains("dw5829e") {
        let is_esim_sku = modem.usb_pid == Some(0x81e6) || modem.esim_supported == Some(true);
        return EsimCapabilityStatus {
            hardware_capable: Some(is_esim_sku),
            lpac_installed,
            eid_detected: !modem.eid.is_empty(),
            reason: if is_esim_sku {
                "DW5829e-eSIM (413c:81e6) detected.".into()
            } else {
                "DW5829e physical-SIM SKU; eUICC requires 413c:81e6 SKU.".into()
            },
        };
    }

    EsimCapabilityStatus {
        hardware_capable: modem.esim_supported,
        lpac_installed,
        eid_detected: !modem.eid.is_empty(),
        reason: if !lpac_installed {
            "lpac (Local Profile Assistant) is not installed on this system.".into()
        } else if modem.esim_supported == Some(true) {
            "eUICC backend ready.".into()
        } else {
            "Modem has not reported an active eUICC interface.".into()
        },
    }
}

pub fn list_profiles_via_lpac() -> Result<Vec<EsimProfile>, String> {
    if !crate::platform::command_exists("lpac") {
        return Err("lpac is not installed.".into());
    }
    let out = Command::new("lpac")
        .args(["profile", "list"])
        .output()
        .map_err(|e| format!("Failed to run lpac: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(Vec::new());
    };
    let mut profiles = Vec::new();
    if let Some(arr) = json
        .get("payload")
        .and_then(|p| p.get("data"))
        .and_then(|d| d.as_array())
    {
        for item in arr {
            let iccid = item.get("iccid").and_then(|v| v.as_str()).unwrap_or("");
            let masked = if iccid.len() > 6 {
                format!("••••{}", &iccid[iccid.len() - 4..])
            } else {
                "••••".into()
            };
            profiles.push(EsimProfile {
                iccid_masked: masked,
                nickname: item
                    .get("profileNickname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                provider: item
                    .get("serviceProviderName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                enabled: item
                    .get("profileState")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case("enabled")),
            });
        }
    }
    Ok(profiles)
}

/// Validates an LPA activation code (`LPA:1$<smdp>$<matching_id>`) without logging it.
pub fn validate_activation_code(code: &str) -> Result<(), String> {
    let trimmed = code.trim();
    if !trimmed.starts_with("LPA:1$") || trimmed.split('$').count() < 3 {
        return Err(
            "Invalid LPA activation code format (expected LPA:1$<SM-DP+>$<MatchingID>).".into(),
        );
    }
    Ok(())
}

pub fn download_profile_via_lpac(activation_code: &str) -> Result<String, String> {
    validate_activation_code(activation_code)?;
    if !crate::platform::command_exists("lpac") {
        return Err("lpac is not installed.".into());
    }
    let out = Command::new("lpac")
        .args(["profile", "download", "-a", activation_code.trim()])
        .output()
        .map_err(|e| format!("Failed to run lpac: {e}"))?;
    if out.status.success() {
        Ok("eSIM profile downloaded successfully.".into())
    } else {
        Err("lpac profile download failed (details omitted to protect activation code).".into())
    }
}

pub fn set_profile_state_via_lpac(iccid_or_aid: &str, enable: bool) -> Result<(), String> {
    if !crate::platform::command_exists("lpac") {
        return Err("lpac is not installed.".into());
    }
    let action = if enable { "enable" } else { "disable" };
    let out = Command::new("lpac")
        .args(["profile", action, iccid_or_aid.trim()])
        .output()
        .map_err(|e| format!("lpac failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn delete_profile_via_lpac(iccid_or_aid: &str) -> Result<(), String> {
    if !crate::platform::command_exists("lpac") {
        return Err("lpac is not installed.".into());
    }
    let out = Command::new("lpac")
        .args(["profile", "delete", iccid_or_aid.trim()])
        .output()
        .map_err(|e| format!("lpac failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn switch_primary_sim_slot(modem_index: &str, slot: u32) -> Result<(), String> {
    let out = Command::new("mmcli")
        .args(["-m", modem_index, &format!("--set-primary-sim-slot={slot}")])
        .output()
        .map_err(|e| format!("mmcli failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
