//! Advanced Network Mode Manager (separated from Band Lock).
//! Filters modes strictly by what the hardware and ModemManager API actually report.

use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkModePreset {
    Automatic,
    Only2G,
    Only3G,
    Only4G,
    Only5G,
    Preferred4G,
    Preferred5G,
    LteAnd5g,
}

impl NetworkModePreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Only2G => "2G Only",
            Self::Only3G => "3G Only",
            Self::Only4G => "4G LTE Only",
            Self::Only5G => "5G NR Only",
            Self::Preferred4G => "4G Preferred",
            Self::Preferred5G => "5G Preferred",
            Self::LteAnd5g => "4G + 5G",
        }
    }

    pub fn mmcli_arg(self) -> &'static str {
        match self {
            Self::Automatic => "2g|3g|4g|5g",
            Self::Only2G => "2g",
            Self::Only3G => "3g",
            Self::Only4G => "4g",
            Self::Only5G => "5g",
            Self::Preferred4G => "2g|3g|4g",
            Self::Preferred5G => "3g|4g|5g",
            Self::LteAnd5g => "4g|5g",
        }
    }

    pub fn mmcli_preferred(self) -> Option<&'static str> {
        match self {
            Self::Preferred4G => Some("4g"),
            Self::Preferred5G | Self::LteAnd5g => Some("5g"),
            _ => None,
        }
    }
}

/// Returns only the network mode presets actually supported by the modem's capabilities.
pub fn supported_presets(modem: &ModemInfo) -> Vec<NetworkModePreset> {
    const GSM_UMTS: u32 = 1 << 2;
    const LTE: u32 = 1 << 3;
    const NR5G: u32 = 1 << 6;

    let caps = modem.current_capabilities;
    let has_2g_3g = caps & GSM_UMTS != 0;
    let has_4g = caps & LTE != 0;
    let has_5g = caps & NR5G != 0;

    let mut out = vec![NetworkModePreset::Automatic];
    if has_2g_3g {
        out.push(NetworkModePreset::Only2G);
        out.push(NetworkModePreset::Only3G);
    }
    if has_4g {
        out.push(NetworkModePreset::Only4G);
        if has_2g_3g {
            out.push(NetworkModePreset::Preferred4G);
        }
    }
    if has_5g {
        out.push(NetworkModePreset::Only5G);
        if has_4g {
            out.push(NetworkModePreset::Preferred5G);
            out.push(NetworkModePreset::LteAnd5g);
        }
    }
    out
}

pub fn apply_network_mode(modem_index: &str, preset: NetworkModePreset) -> Result<(), String> {
    let allowed_flag = format!("--set-allowed-modes={}", preset.mmcli_arg());
    let mut args = vec!["-m".to_string(), modem_index.to_string(), allowed_flag];
    if let Some(pref) = preset.mmcli_preferred() {
        args.push(format!("--set-preferred-mode={pref}"));
    }
    let out = Command::new("mmcli")
        .args(&args)
        .output()
        .map_err(|e| format!("mmcli failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedNetworkModes {
    pub per_sim: HashMap<String, NetworkModePreset>,
}

fn modes_path() -> PathBuf {
    crate::core::settings::data_home().join("modemdeck/network_modes.json")
}

pub fn save_mode_for_sim(sim_hash: &str, preset: NetworkModePreset) -> Result<(), String> {
    let path = modes_path();
    let mut store: SavedNetworkModes = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    store.per_sim.insert(sim_hash.to_string(), preset);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}
