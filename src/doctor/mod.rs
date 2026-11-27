//! Modem Doctor — comprehensive hardware & connectivity health checks.
//! Auto-fixes are only executed after explicit user consent.

use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DoctorSeverity {
    Healthy,
    Warning,
    Error,
    Critical,
    Unknown,
    Unsupported,
}

impl DoctorSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Critical => "Critical",
            Self::Unknown => "Unknown",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub id: String,
    pub title: String,
    pub severity: DoctorSeverity,
    pub explanation: String,
    pub probable_cause: String,
    pub evidence: String,
    pub suggested_fix: String,
    pub manual_steps: String,
    pub safe_auto_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub timestamp_unix: u64,
    pub overall: DoctorSeverity,
    pub checks: Vec<DoctorCheck>,
}

pub fn run_doctor(modems: &[ModemInfo]) -> DoctorReport {
    let mut checks = Vec::new();

    // 1. ModemManager availability
    let mm_active = Command::new("systemctl")
        .args(["is-active", "--quiet", "ModemManager"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    checks.push(DoctorCheck {
        id: "modemmanager".into(),
        title: "ModemManager Service".into(),
        severity: if mm_active {
            DoctorSeverity::Healthy
        } else {
            DoctorSeverity::Critical
        },
        explanation: if mm_active {
            "ModemManager daemon is active.".into()
        } else {
            "ModemManager daemon is not running.".into()
        },
        probable_cause: if mm_active {
            String::new()
        } else {
            "Service stopped or not installed.".into()
        },
        evidence: format!("systemctl is-active ModemManager => {mm_active}"),
        suggested_fix: if mm_active {
            String::new()
        } else {
            "Start ModemManager service.".into()
        },
        manual_steps: "sudo systemctl enable --now ModemManager".into(),
        safe_auto_fix: None,
    });

    // 2. NetworkManager availability
    let nm_active = Command::new("systemctl")
        .args(["is-active", "--quiet", "NetworkManager"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    checks.push(DoctorCheck {
        id: "networkmanager".into(),
        title: "NetworkManager Service".into(),
        severity: if nm_active {
            DoctorSeverity::Healthy
        } else {
            DoctorSeverity::Warning
        },
        explanation: if nm_active {
            "NetworkManager is active for APN/routing management.".into()
        } else {
            "NetworkManager is inactive; APN and failover automation will be limited.".into()
        },
        probable_cause: String::new(),
        evidence: format!("systemctl is-active NetworkManager => {nm_active}"),
        suggested_fix: if nm_active {
            String::new()
        } else {
            "Enable NetworkManager.".into()
        },
        manual_steps: "sudo systemctl enable --now NetworkManager".into(),
        safe_auto_fix: None,
    });

    // 3. RFKill / Airplane mode check
    let rfkill_blocked = check_wwan_rfkill_blocked();
    checks.push(DoctorCheck {
        id: "rfkill".into(),
        title: "WWAN RFKill / Airplane Mode".into(),
        severity: if rfkill_blocked {
            DoctorSeverity::Error
        } else {
            DoctorSeverity::Healthy
        },
        explanation: if rfkill_blocked {
            "WWAN radio is blocked by RFKill or Airplane Mode.".into()
        } else {
            "WWAN radio is unblocked.".into()
        },
        probable_cause: if rfkill_blocked {
            "Hardware/software radio switch disabled WWAN.".into()
        } else {
            String::new()
        },
        evidence: format!("rfkill wwan blocked = {rfkill_blocked}"),
        suggested_fix: if rfkill_blocked {
            "Unblock WWAN via rfkill.".into()
        } else {
            String::new()
        },
        manual_steps: "rfkill unblock wwan".into(),
        safe_auto_fix: if rfkill_blocked {
            Some("rfkill-unblock-wwan".into())
        } else {
            None
        },
    });

    // 4. Modem-specific checks
    if modems.is_empty() {
        checks.push(DoctorCheck {
            id: "modem_detection".into(),
            title: "Cellular Modem Detection".into(),
            severity: DoctorSeverity::Warning,
            explanation: "No cellular modem reported by ModemManager.".into(),
            probable_cause:
                "Modem disabled in BIOS, missing MHI/USB driver, or USB cable disconnected.".into(),
            evidence: "0 modems on D-Bus".into(),
            suggested_fix: "Check lsusb/lspci and BIOS WWAN enablement.".into(),
            manual_steps: "lsusb && lspci -nn | grep -i -E 'wwan|modem|qualcomm|intel|mediatek'"
                .into(),
            safe_auto_fix: None,
        });
    } else {
        for (idx, m) in modems.iter().enumerate() {
            let prefix = format!("Modem {} ({})", idx + 1, m.display_name());
            // SIM check
            let sim_ok = !m.sim_path.is_empty() && m.sim_path != "/";
            checks.push(DoctorCheck {
                id: format!("sim_{idx}"),
                title: format!("{prefix}: SIM Detection"),
                severity: if !sim_ok {
                    DoctorSeverity::Error
                } else if m.unlock_required > 1 {
                    DoctorSeverity::Warning
                } else {
                    DoctorSeverity::Healthy
                },
                explanation: if !sim_ok {
                    "No SIM card detected in active slot.".into()
                } else if m.unlock_required > 1 {
                    format!("SIM requires unlock: {}", m.sim_lock_label())
                } else {
                    "SIM detected and unlocked.".into()
                },
                probable_cause: if !sim_ok {
                    "SIM not seated or wrong SIM slot selected.".into()
                } else {
                    String::new()
                },
                evidence: format!(
                    "sim_path={}, unlock_required={}",
                    m.sim_path, m.unlock_required
                ),
                suggested_fix: if m.unlock_required > 1 {
                    "Unlock SIM PIN in SIM & eSIM page.".into()
                } else {
                    String::new()
                },
                manual_steps: "Check SIM tray or switch primary SIM slot.".into(),
                safe_auto_fix: None,
            });

            // FCC Lock check
            let fcc = crate::fcc::inspect_fcc_lock(m);
            checks.push(DoctorCheck {
                id: format!("fcc_{idx}"),
                title: format!("{prefix}: FCC Lock"),
                severity: match fcc.state {
                    crate::fcc::FccLockState::Locked => DoctorSeverity::Error,
                    crate::fcc::FccLockState::Unlocked => DoctorSeverity::Healthy,
                    _ => DoctorSeverity::Unknown,
                },
                explanation: fcc.state.label().into(),
                probable_cause: if fcc.state == crate::fcc::FccLockState::Locked {
                    "OEM BIOS/firmware enforces FCC power lock until ModemManager fcc-unlock runs."
                        .into()
                } else {
                    String::new()
                },
                evidence: format!(
                    "script_present={}, state={:?}",
                    fcc.official_script_present, fcc.state
                ),
                suggested_fix: if fcc.state == crate::fcc::FccLockState::Locked {
                    "Enable official ModemManager fcc-unlock script.".into()
                } else {
                    String::new()
                },
                manual_steps: fcc.official_procedure,
                safe_auto_fix: None,
            });

            // Registration & Connectivity
            checks.push(DoctorCheck {
                id: format!("reg_{idx}"),
                title: format!("{prefix}: Network Registration & Data"),
                severity: if m.state == 11 {
                    DoctorSeverity::Healthy
                } else if m.state >= 8 {
                    DoctorSeverity::Warning
                } else {
                    DoctorSeverity::Error
                },
                explanation: format!("Modem state: {} ({})", m.state_label(), m.operator_label()),
                probable_cause: if m.state < 8 {
                    "Weak signal, SIM not provisioned, or band lock mismatch.".into()
                } else {
                    String::new()
                },
                evidence: format!("state={}, signal={}%", m.state, m.signal_percent),
                suggested_fix: if m.state < 8 {
                    "Reset bands to Auto or check APN settings.".into()
                } else {
                    String::new()
                },
                manual_steps: "mmcli -m <idx>".into(),
                safe_auto_fix: None,
            });
        }
    }

    // 5. DNS check
    let dns_ok = fs::read_to_string("/etc/resolv.conf")
        .map(|s| s.lines().any(|l| l.trim_start().starts_with("nameserver")))
        .unwrap_or(false);
    checks.push(DoctorCheck {
        id: "dns_config".into(),
        title: "DNS Resolver Configuration".into(),
        severity: if dns_ok {
            DoctorSeverity::Healthy
        } else {
            DoctorSeverity::Warning
        },
        explanation: if dns_ok {
            "At least one nameserver is configured in /etc/resolv.conf.".into()
        } else {
            "No nameserver entry found in /etc/resolv.conf.".into()
        },
        probable_cause: String::new(),
        evidence: format!("resolv.conf has nameserver = {dns_ok}"),
        suggested_fix: if dns_ok {
            String::new()
        } else {
            "Verify systemd-resolved or NetworkManager DNS.".into()
        },
        manual_steps: "resolvectl status".into(),
        safe_auto_fix: None,
    });

    // 6. Default Gateway & Routing Table check
    let route_out = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    checks.push(DoctorCheck {
        id: "default_gateway".into(),
        title: "Default Gateway & Routing".into(),
        severity: if route_out.is_empty() {
            DoctorSeverity::Warning
        } else {
            DoctorSeverity::Healthy
        },
        explanation: if route_out.is_empty() {
            "No default IPv4 route is currently present.".into()
        } else {
            format!(
                "Default route active: {}",
                route_out.lines().next().unwrap_or("")
            )
        },
        probable_cause: String::new(),
        evidence: route_out.clone(),
        suggested_fix: if route_out.is_empty() {
            "Activate a mobile broadband or Wi-Fi connection.".into()
        } else {
            String::new()
        },
        manual_steps: "ip route show".into(),
        safe_auto_fix: None,
    });

    // 7. MTU & Interface check for active WWAN ports
    for m in modems {
        for iface in &m.net_ports {
            let mtu_str = fs::read_to_string(format!("/sys/class/net/{iface}/mtu"))
                .unwrap_or_default()
                .trim()
                .to_string();
            if let Ok(mtu) = mtu_str.parse::<u32>() {
                let ok = (1280..=1500).contains(&mtu);
                checks.push(DoctorCheck {
                    id: format!("mtu_{iface}"),
                    title: format!("MTU ({iface})"),
                    severity: if ok {
                        DoctorSeverity::Healthy
                    } else {
                        DoctorSeverity::Warning
                    },
                    explanation: format!("Interface {iface} MTU is {mtu}."),
                    probable_cause: if ok {
                        String::new()
                    } else {
                        "Unusual MTU may cause TLS/packet fragmentation issues.".into()
                    },
                    evidence: format!("mtu={mtu}"),
                    suggested_fix: if ok {
                        String::new()
                    } else {
                        "Set MTU between 1280 and 1500 (typically 1420-1500 for LTE/5G).".into()
                    },
                    manual_steps: format!("ip link set dev {iface} mtu 1420"),
                    safe_auto_fix: None,
                });
            }
        }
    }

    // 8. Conflicting services check (e.g., ofono competing with ModemManager)
    let ofono_active = Command::new("systemctl")
        .args(["is-active", "--quiet", "ofono"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    checks.push(DoctorCheck {
        id: "conflicting_services".into(),
        title: "Conflicting Telephony Services".into(),
        severity: if ofono_active {
            DoctorSeverity::Warning
        } else {
            DoctorSeverity::Healthy
        },
        explanation: if ofono_active {
            "ofono is active and may conflict with ModemManager for modem ports.".into()
        } else {
            "No conflicting telephony daemon (ofono) detected.".into()
        },
        probable_cause: String::new(),
        evidence: format!("ofono active = {ofono_active}"),
        suggested_fix: if ofono_active {
            "Disable ofono if ModemManager manages your WWAN card.".into()
        } else {
            String::new()
        },
        manual_steps: "sudo systemctl disable --now ofono".into(),
        safe_auto_fix: None,
    });

    // 9. Suspend/Resume power service readiness
    let logind_ok = Path::new("/run/systemd/seats").exists();
    checks.push(DoctorCheck {
        id: "suspend_resume".into(),
        title: "Suspend / Resume Sync Readiness".into(),
        severity: if logind_ok {
            DoctorSeverity::Healthy
        } else {
            DoctorSeverity::Unknown
        },
        explanation: if logind_ok {
            "systemd-logind PrepareForSleep integration is available.".into()
        } else {
            "systemd-logind runtime directory not detected.".into()
        },
        probable_cause: String::new(),
        evidence: format!("/run/systemd/seats exists = {logind_ok}"),
        suggested_fix: String::new(),
        manual_steps: "systemctl status systemd-logind".into(),
        safe_auto_fix: None,
    });

    let overall = checks
        .iter()
        .map(|c| c.severity)
        .filter(|s| {
            matches!(
                s,
                DoctorSeverity::Healthy
                    | DoctorSeverity::Warning
                    | DoctorSeverity::Error
                    | DoctorSeverity::Critical
            )
        })
        .max()
        .unwrap_or(DoctorSeverity::Unknown);

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    DoctorReport {
        timestamp_unix: ts,
        overall,
        checks,
    }
}

fn check_wwan_rfkill_blocked() -> bool {
    let Ok(entries) = fs::read_dir("/sys/class/rfkill") else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let kind = fs::read_to_string(p.join("type")).unwrap_or_default();
        if kind.trim() == "wwan" {
            let soft = fs::read_to_string(p.join("soft")).unwrap_or_default();
            let hard = fs::read_to_string(p.join("hard")).unwrap_or_default();
            if soft.trim() == "1" || hard.trim() == "1" {
                return true;
            }
        }
    }
    false
}

/// Executes only explicitly whitelisted, safe, non-destructive fixes after user consent.
pub fn apply_safe_auto_fix(fix_id: &str) -> Result<String, String> {
    match fix_id {
        "rfkill-unblock-wwan" => {
            let out = Command::new("rfkill")
                .args(["unblock", "wwan"])
                .output()
                .map_err(|e| format!("Failed to run rfkill: {e}"))?;
            if out.status.success() {
                Ok("Unblocked WWAN radio via rfkill.".into())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
        }
        _ => Err("Unknown or non-automatic fix ID.".into()),
    }
}

pub fn has_mhi_sysfs() -> bool {
    Path::new("/sys/bus/mhi").exists()
}
