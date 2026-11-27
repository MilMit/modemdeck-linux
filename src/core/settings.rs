use serde::{Deserialize, Serialize};
use std::{env, fs, io, path::PathBuf, process::Command};

pub const CURRENT_SETTINGS_SCHEMA: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => Self::System,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SpeedTestServer {
    pub name: String,
    pub category: String, // "domestic" | "international" | "custom"
    pub download_url: String,
    pub upload_url: String,
    pub ping_host: String,
    pub ping_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StabilityThresholds {
    pub high_latency_ms: u32,
    pub packet_loss_percent: f64,
    pub min_rsrp_dbm: f64,
    pub min_snr_db: f64,
    pub notification_cooldown_secs: u64,
    pub silent_mode: bool,
    pub auto_recovery: bool,
}

impl Default for StabilityThresholds {
    fn default() -> Self {
        Self {
            high_latency_ms: 250,
            packet_loss_percent: 10.0,
            min_rsrp_dbm: -112.0,
            min_snr_db: 0.0,
            notification_cooldown_secs: 120,
            silent_mode: false,
            auto_recovery: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub background_monitor: bool,
    pub notifications: bool,
    pub signal_refresh_seconds: u32,
    pub traffic_refresh_seconds: u32,
    pub language: String,
    pub theme: String,
    pub restore_profile_on_startup: bool,
    pub check_updates_on_startup: bool,
    pub speedtest_consent_given: bool,
    pub speedtest_data_cap_mb: u64,
    pub history_retention_days: u32,
    pub smart_selection_enabled: bool,
    pub smart_selection_profile: String,
    pub ui_mode: String,          // "basic" | "advanced" | "expert"
    pub font_size_preset: String, // "small" | "default" | "large" | "xlarge"
    pub close_to_tray: bool,
    pub start_with_system: bool,
    pub reduced_motion: bool,
    pub remote_management_enabled: bool,
    pub remote_management_port: u16,
    pub stability: StabilityThresholds,
    pub custom_speed_servers: Vec<SpeedTestServer>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SETTINGS_SCHEMA,
            background_monitor: false,
            notifications: true,
            signal_refresh_seconds: 10,
            traffic_refresh_seconds: 2,
            language: "system".into(),
            theme: "system".into(),
            restore_profile_on_startup: false,
            check_updates_on_startup: false,
            speedtest_consent_given: false,
            speedtest_data_cap_mb: 250,
            history_retention_days: 30,
            smart_selection_enabled: false,
            smart_selection_profile: "Balanced".into(),
            ui_mode: "advanced".into(),
            font_size_preset: "default".into(),
            close_to_tray: true,
            start_with_system: false,
            reduced_motion: false,
            remote_management_enabled: false,
            remote_management_port: 8787,
            stability: StabilityThresholds::default(),
            custom_speed_servers: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut settings = serde_json::from_str::<Self>(&text).unwrap_or_default();
        if settings.schema_version < CURRENT_SETTINGS_SCHEMA {
            settings.migrate();
            let _ = settings.save();
        }
        settings
    }

    pub fn migrate(&mut self) {
        if self.schema_version == 0 || self.schema_version == 1 {
            if self.theme.is_empty() {
                self.theme = "system".into();
            }
            if self.history_retention_days == 0 {
                self.history_retention_days = 30;
            }
            if self.speedtest_data_cap_mb == 0 {
                self.speedtest_data_cap_mb = 250;
            }
            if self.smart_selection_profile.is_empty() {
                self.smart_selection_profile = "Balanced".into();
            }
            self.schema_version = CURRENT_SETTINGS_SCHEMA;
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|e| io::Error::other(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(tmp, path)
    }

    pub fn normalized_refresh_seconds(&self) -> u32 {
        self.signal_refresh_seconds.clamp(5, 60)
    }

    pub fn normalized_traffic_refresh_seconds(&self) -> u32 {
        self.traffic_refresh_seconds.clamp(1, 30)
    }

    pub fn theme_mode(&self) -> ThemeMode {
        ThemeMode::from_str(&self.theme)
    }
}

pub fn settings_path() -> PathBuf {
    config_home().join("modemdeck/settings.json")
}

pub fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
}

pub fn data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"))
}

pub fn set_background_monitor_enabled(enabled: bool) -> Result<String, String> {
    let action = if enabled { "enable" } else { "disable" };
    let output = Command::new("systemctl")
        .args([
            "--user",
            action,
            "--now",
            "net.milmit.ModemDeck-monitor.service",
        ])
        .output()
        .map_err(|e| format!("Could not run systemctl --user: {e}"))?;
    if output.status.success() {
        Ok(if enabled {
            "Background monitor enabled.".into()
        } else {
            "Background monitor disabled.".into()
        })
    } else {
        let e = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if e.is_empty() {
            "systemctl --user returned an error.".into()
        } else {
            e
        })
    }
}

pub fn restart_background_monitor() -> Result<(), String> {
    let output = Command::new("systemctl")
        .args(["--user", "restart", "net.milmit.ModemDeck-monitor.service"])
        .output()
        .map_err(|e| format!("Could not restart background monitor: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let e = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if e.is_empty() {
            "Background monitor restart failed.".into()
        } else {
            e
        })
    }
}

pub fn system_service_installed() -> bool {
    PathBuf::from("/etc/systemd/system/modemdeck-monitor.service").exists()
        || PathBuf::from("/usr/lib/systemd/system/modemdeck-monitor.service").exists()
}

pub fn user_service_active() -> bool {
    Command::new("systemctl")
        .args([
            "--user",
            "is-active",
            "--quiet",
            "net.milmit.ModemDeck-monitor.service",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
