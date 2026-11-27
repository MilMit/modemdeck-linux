//! Connection & Diagnostics Scheduler.
//! Tasks that consume data or may interrupt connections are disabled by default.

use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledAction {
    Connect,
    Disconnect,
    EnableMonitoring,
    DisableMonitoring,
    RadioDiagnosticsOnly,
    BandQualityTest,
    DataUsageReport,
}

impl ScheduledAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connect => "Connect WWAN",
            Self::Disconnect => "Disconnect WWAN",
            Self::EnableMonitoring => "Enable Background Monitoring",
            Self::DisableMonitoring => "Disable Background Monitoring",
            Self::RadioDiagnosticsOnly => "Radio-Only Diagnostics",
            Self::BandQualityTest => "Band Quality Test",
            Self::DataUsageReport => "Data Usage Report",
        }
    }

    pub fn default_enabled(self) -> bool {
        !matches!(
            self,
            Self::Disconnect | Self::BandQualityTest | Self::Connect
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub action: ScheduledAction,
    pub hour: u8,
    pub minute: u8,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedulerConfig {
    pub quiet_hours_enabled: bool,
    pub quiet_start_hour: u8,
    pub quiet_end_hour: u8,
    pub tasks: Vec<ScheduledTask>,
}

fn scheduler_path() -> PathBuf {
    crate::core::settings::config_home().join("modemdeck/scheduler.json")
}

impl SchedulerConfig {
    pub fn load() -> Self {
        fs::read_to_string(scheduler_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = scheduler_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }
}
