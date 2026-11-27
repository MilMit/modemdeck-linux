//! Cross-platform abstraction layer for ModemDeck.
//!
//! Keeps core domain logic (operator identification, hardware compatibility,
//! signal scoring, band analysis, usage tracking, diagnostics, and UI models)
//! decoupled from OS-specific APIs such as Linux D-Bus/ModemManager/NetworkManager
//! or future Windows WinRT/MBIM/SetupAPI backends.

use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformOs {
    Linux,
    Windows,
    MacOS,
    Other,
}

impl PlatformOs {
    pub fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOS
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Self::Other
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linux => "Linux",
            Self::Windows => "Windows",
            Self::MacOS => "macOS",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub os: PlatformOs,
    pub modem_manager_dbus: bool,
    pub network_manager: bool,
    pub mbim_native: bool,
    pub qmi_native: bool,
    pub fwupd_available: bool,
    pub lpac_available: bool,
    pub secret_store_available: bool,
    pub systemd_user_available: bool,
}

impl Default for PlatformCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

impl PlatformCapabilities {
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                os: PlatformOs::Linux,
                modem_manager_dbus: command_exists("mmcli"),
                network_manager: command_exists("nmcli"),
                mbim_native: command_exists("mbimcli")
                    || std::path::Path::new("/dev/cdc-wdm0").exists(),
                qmi_native: command_exists("qmicli")
                    || std::path::Path::new("/dev/wwan0qmi0").exists(),
                fwupd_available: command_exists("fwupdmgr"),
                lpac_available: command_exists("lpac"),
                secret_store_available: command_exists("secret-tool") || command_exists("nmcli"),
                systemd_user_available: command_exists("systemctl"),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                os: PlatformOs::current(),
                modem_manager_dbus: false,
                network_manager: false,
                mbim_native: false,
                qmi_native: false,
                fwupd_available: false,
                lpac_available: false,
                secret_store_available: false,
                systemd_user_available: false,
            }
        }
    }
}

/// Core modem operations contract shared across Linux and future Windows backends.
pub trait ModemBackend {
    fn list_modems(&self) -> Result<Vec<ModemInfo>, String>;
    fn set_bands(&self, modem_id: &str, bands: &[u32]) -> Result<(), String>;
    fn unlock_sim_pin(&self, sim_id: &str, pin: &str) -> Result<(), String>;
    fn set_power_state(&self, modem_id: &str, enabled: bool) -> Result<(), String>;
}

/// OS networking / APN / routing contract.
pub trait NetworkBackend {
    fn default_interface(&self) -> Option<String>;
    fn list_wwan_interfaces(&self) -> Vec<String>;
    fn is_metered(&self, iface: &str) -> bool;
}

/// OS credential / secret store contract (libsecret on Linux, Credential Manager on Windows).
pub trait SecretStoreBackend {
    fn store_secret(&self, key: &str, value: &str) -> Result<(), String>;
    fn read_secret(&self, key: &str) -> Result<Option<String>, String>;
    fn delete_secret(&self, key: &str) -> Result<(), String>;
}

/// Firmware metadata and update contract (fwupd/LVFS on Linux, Windows Update/SetupAPI on Windows).
pub trait FirmwareBackend {
    fn backend_name(&self) -> &'static str;
    fn is_available(&self) -> bool;
}

/// Linux SecretStore using `secret-tool` (Freedesktop Secret Service) when available.
pub struct LinuxSecretStore;

impl SecretStoreBackend for LinuxSecretStore {
    fn store_secret(&self, key: &str, value: &str) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            use std::process::{Command, Stdio};
            let mut child = Command::new("secret-tool")
                .args([
                    "store",
                    "--label",
                    &format!("ModemDeck: {key}"),
                    "application",
                    "net.milmit.ModemDeck",
                    "key",
                    key,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("secret-tool unavailable: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(value.as_bytes());
            }
            let out = child
                .wait_with_output()
                .map_err(|e| format!("secret-tool failed: {e}"))?;
            if out.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (key, value);
            Err("Linux secret store is only available on Linux.".into())
        }
    }

    fn read_secret(&self, key: &str) -> Result<Option<String>, String> {
        #[cfg(target_os = "linux")]
        {
            let out = std::process::Command::new("secret-tool")
                .args(["lookup", "application", "net.milmit.ModemDeck", "key", key])
                .output()
                .map_err(|e| format!("secret-tool unavailable: {e}"))?;
            if out.status.success() {
                let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
                Ok(if val.is_empty() { None } else { Some(val) })
            } else {
                Ok(None)
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = key;
            Ok(None)
        }
    }

    fn delete_secret(&self, key: &str) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let out = std::process::Command::new("secret-tool")
                .args(["clear", "application", "net.milmit.ModemDeck", "key", key])
                .output()
                .map_err(|e| format!("secret-tool unavailable: {e}"))?;
            if out.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = key;
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
pub mod windows_stub {
    //! Future Windows backend integration boundary.
    //! Intended APIs: WinRT `Windows.Networking.NetworkOperators.MobileBroadbandAccount`,
    //! MBIM Device Services, SetupAPI / CfgMgr32, Windows Credential Manager.
    pub fn not_implemented_yet() -> &'static str {
        "Windows backend is planned; see docs/WINDOWS_PORTING.md."
    }
}

pub fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let p = dir.join(name);
                p.is_file()
            })
        })
        .unwrap_or(false)
}
