use crate::hotspot::wifi_ap;
use crate::model::ModemInfo;
use std::{fs, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationLevel {
    Verified,
    Unverified,
    BackendRequired,
}

impl VerificationLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Verified => "Verified",
            Self::Unverified => "Unverified",
            Self::BackendRequired => "Backend Required",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UplinkSource {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub interface: String,
    pub state: String,
    pub health: String,
    pub note: String,
    pub available: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VpnInspection {
    pub detected: bool,
    pub tunnel_name: String,
    pub tunnel_iface: String,
    pub full_tunnel: bool,
    pub dns_leak_risk: bool,
    pub ipv6_leak_risk: bool,
    pub verification_label: String,
    pub summary: String,
    pub fail_closed_supported: bool,
}

fn default_route_iface() -> String {
    let Ok(out) = Command::new("ip")
        .args(["-4", "route", "show", "default"])
        .output()
    else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(pos) = parts.iter().position(|p| *p == "dev")
            && let Some(dev) = parts.get(pos + 1)
        {
            return (*dev).to_string();
        }
    }
    String::new()
}

/// Inspects active uplinks on Linux (Selected Modem, Ethernet, Active Wi-Fi, Active VPN).
pub fn detect_uplinks(selected_modem: Option<&ModemInfo>) -> (String, Vec<UplinkSource>) {
    let def_iface = default_route_iface();
    let wifi_caps = wifi_ap::detect_capabilities();

    let mut out = vec![UplinkSource {
        id: "auto".into(),
        kind: "Auto".into(),
        name: "Automatic System Default".into(),
        interface: if def_iface.is_empty() {
            "—".into()
        } else {
            def_iface.clone()
        },
        state: if def_iface.is_empty() {
            "Offline".into()
        } else {
            "Active".into()
        },
        health: if def_iface.is_empty() {
            "Offline".into()
        } else {
            "Healthy".into()
        },
        note: "Follows OS default routing table; does not isolate AP traffic.".into(),
        available: !def_iface.is_empty(),
    }];

    // Selected Cellular Modem
    if let Some(m) = selected_modem {
        let iface = m.net_ports.first().cloned().unwrap_or_else(|| "—".into());
        let is_conn = m.state == 11 || m.state_label() == "Connected";
        out.push(UplinkSource {
            id: "cellular".into(),
            kind: "Cellular".into(),
            name: format!("{} ({})", m.display_name(), m.operator_label()),
            interface: iface,
            state: m.state_label().to_string(),
            health: if is_conn { "Healthy" } else { "Warning" }.into(),
            note: if is_conn {
                "Cellular data bearer is active.".into()
            } else {
                "Modem is not currently in Connected state.".into()
            },
            available: is_conn,
        });
    } else {
        out.push(UplinkSource {
            id: "cellular".into(),
            kind: "Cellular".into(),
            name: "Cellular Modem".into(),
            interface: "—".into(),
            state: "Offline".into(),
            health: "Offline".into(),
            note: "No cellular modem selected or detected.".into(),
            available: false,
        });
    }

    // Active NetworkManager connections (Ethernet, Wi-Fi, VPN)
    let mut eth_found = false;
    let mut wifi_found = false;
    let mut vpn_found = false;

    if let Ok(nm) = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "NAME,TYPE,DEVICE,STATE",
            "connection",
            "show",
            "--active",
        ])
        .output()
    {
        for line in String::from_utf8_lossy(&nm.stdout).lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let (name, ctype, dev, state) = (parts[0], parts[1], parts[2], parts[3]);
            if ctype.contains("ethernet") && !eth_found {
                eth_found = true;
                out.push(UplinkSource {
                    id: "ethernet".into(),
                    kind: "Ethernet".into(),
                    name: name.into(),
                    interface: dev.into(),
                    state: state.into(),
                    health: "Healthy".into(),
                    note: "Wired uplink available.".into(),
                    available: true,
                });
            } else if ctype.contains("wireless")
                && !name.eq_ignore_ascii_case("Hotspot")
                && !wifi_found
            {
                wifi_found = true;
                let note = if wifi_caps.concurrent_sta_ap {
                    "Wi-Fi card supports concurrent STA+AP.".into()
                } else {
                    "Card lacks STA+AP concurrency: starting Hotspot will disconnect this Wi-Fi."
                        .into()
                };
                out.push(UplinkSource {
                    id: "wifi".into(),
                    kind: "Wi-Fi".into(),
                    name: name.into(),
                    interface: dev.into(),
                    state: state.into(),
                    health: if wifi_caps.concurrent_sta_ap {
                        "Healthy"
                    } else {
                        "Warning"
                    }
                    .into(),
                    note,
                    available: wifi_caps.concurrent_sta_ap,
                });
            } else if (ctype == "vpn"
                || ctype == "wireguard"
                || dev.starts_with("tun")
                || dev.starts_with("wg"))
                && !vpn_found
            {
                vpn_found = true;
                out.push(UplinkSource {
                    id: "vpn".into(),
                    kind: "VPN".into(),
                    name: name.into(),
                    interface: dev.into(),
                    state: state.into(),
                    health: "Unverified".into(),
                    note: "Tunnel detected; AP client routing through VPN is Unverified unless nftables/policy routing is active.".into(),
                    available: true,
                });
            }
        }
    }

    if !eth_found {
        out.push(UplinkSource {
            id: "ethernet".into(),
            kind: "Ethernet".into(),
            name: "Ethernet".into(),
            interface: "—".into(),
            state: "Disconnected".into(),
            health: "Offline".into(),
            note: "No active wired Ethernet connection.".into(),
            available: false,
        });
    }
    if !wifi_found {
        out.push(UplinkSource {
            id: "wifi".into(),
            kind: "Wi-Fi".into(),
            name: "Active Wi-Fi".into(),
            interface: wifi_caps.wifi_device.unwrap_or_else(|| "—".into()),
            state: "Disconnected".into(),
            health: "Offline".into(),
            note: "No active Wi-Fi client uplink.".into(),
            available: false,
        });
    }
    if !vpn_found {
        // Also check non-NM kernel tunnels (tun*, wg*)
        let kernel_tun = detect_kernel_tunnel();
        if let Some(tun) = kernel_tun {
            out.push(UplinkSource {
                id: "vpn".into(),
                kind: "VPN".into(),
                name: format!("Kernel Tunnel ({tun})"),
                interface: tun,
                state: "Active".into(),
                health: "Unverified".into(),
                note: "Non-NetworkManager tunnel detected; client traffic verification required."
                    .into(),
                available: true,
            });
        } else {
            out.push(UplinkSource {
                id: "vpn".into(),
                kind: "VPN".into(),
                name: "VPN Tunnel".into(),
                interface: "—".into(),
                state: "Inactive".into(),
                health: "Offline".into(),
                note: "No active NetworkManager VPN or kernel tun/wg tunnel.".into(),
                available: false,
            });
        }
    }

    (def_iface, out)
}

fn detect_kernel_tunnel() -> Option<String> {
    let entries = fs::read_dir("/sys/class/net").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("tun") || name.starts_with("wg") || name.starts_with("ppp") {
            let oper = fs::read_to_string(entry.path().join("operstate")).unwrap_or_default();
            if oper.trim() == "up" || oper.trim() == "unknown" {
                return Some(name);
            }
        }
    }
    None
}

/// Inspects active VPN tunnel, Full vs Split tunnel routing, and DNS/IPv6 leak risks.
pub fn inspect_vpn_path() -> VpnInspection {
    let mut insp = VpnInspection {
        verification_label: VerificationLevel::Unverified.label().into(),
        ..Default::default()
    };

    let tun_iface = detect_kernel_tunnel();
    let mut nm_vpn_name = String::new();
    if let Ok(nm) = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "NAME,TYPE,DEVICE",
            "connection",
            "show",
            "--active",
        ])
        .output()
    {
        for line in String::from_utf8_lossy(&nm.stdout).lines() {
            let p: Vec<&str> = line.split(':').collect();
            if p.len() >= 3 && (p[1] == "vpn" || p[1] == "wireguard") {
                nm_vpn_name = p[0].to_string();
                if insp.tunnel_iface.is_empty() && !p[2].is_empty() && p[2] != "--" {
                    insp.tunnel_iface = p[2].to_string();
                }
                break;
            }
        }
    }
    if insp.tunnel_iface.is_empty()
        && let Some(t) = tun_iface
    {
        insp.tunnel_iface = t;
    }

    if insp.tunnel_iface.is_empty() && nm_vpn_name.is_empty() {
        insp.summary = "No active VPN tunnel detected.".into();
        return insp;
    }

    insp.detected = true;
    insp.tunnel_name = if nm_vpn_name.is_empty() {
        format!("Tunnel ({})", insp.tunnel_iface)
    } else {
        nm_vpn_name
    };

    // Check IPv4 routing table for Full-Tunnel vs Split-Tunnel
    let routes = Command::new("ip")
        .args(["-4", "route", "show"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let has_def_tun = routes.lines().any(|l| {
        (l.starts_with("default ") || l.starts_with("0.0.0.0/1 "))
            && !insp.tunnel_iface.is_empty()
            && l.contains(&insp.tunnel_iface)
    });
    insp.full_tunnel = has_def_tun;

    // Check IPv6 default route outside tunnel
    let v6_routes = Command::new("ip")
        .args(["-6", "route", "show", "default"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    insp.ipv6_leak_risk = v6_routes.lines().any(|l| {
        !l.trim().is_empty() && (insp.tunnel_iface.is_empty() || !l.contains(&insp.tunnel_iface))
    });

    // Check /etc/resolv.conf for local/ISP DNS outside tunnel
    let resolv = fs::read_to_string("/etc/resolv.conf").unwrap_or_default();
    insp.dns_leak_risk = resolv
        .lines()
        .filter(|l| l.trim_start().starts_with("nameserver "))
        .any(|l| l.contains("192.168.") || l.contains("10.") || l.contains("172.16."));

    insp.fail_closed_supported = Command::new("which")
        .arg("nft")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    insp.verification_label = VerificationLevel::Unverified.label().into();
    insp.summary = format!(
        "{} ({}) • {} • DNS leak risk: {} • IPv6 leak risk: {} • Client path: Unverified (requires nftables/policy routing)",
        insp.tunnel_name,
        if insp.tunnel_iface.is_empty() {
            "—"
        } else {
            &insp.tunnel_iface
        },
        if insp.full_tunnel {
            "Full-Tunnel"
        } else {
            "Split-Tunnel"
        },
        if insp.dns_leak_risk {
            "Possible"
        } else {
            "Low"
        },
        if insp.ipv6_leak_risk {
            "Possible (IPv6 default outside tunnel)"
        } else {
            "Low"
        }
    );
    insp
}

/// Applies Linux nftables Fail-Closed kill-switch on Hotspot forward chain (requires nft + pkexec).
pub fn apply_vpn_fail_closed(
    ap_iface: &str,
    vpn_iface: &str,
    enable: bool,
) -> Result<String, String> {
    if ap_iface.trim().is_empty() {
        return Err("Hotspot interface is not active.".into());
    }
    if !enable {
        let out = Command::new("pkexec")
            .args(["nft", "delete", "table", "inet", "modemdeck_killswitch"])
            .output()
            .map_err(|e| format!("Failed to invoke pkexec nft: {e}"))?;
        if out.status.success() {
            return Ok("Removed nftables Fail-Closed kill-switch.".into());
        }
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    if vpn_iface.trim().is_empty() {
        return Err("No active VPN tunnel interface for Fail-Closed rule.".into());
    }
    let rules = format!(
        "add table inet modemdeck_killswitch; \
         flush table inet modemdeck_killswitch; \
         add chain inet modemdeck_killswitch forward {{ type filter hook forward priority -10; policy accept; }}; \
         add rule inet modemdeck_killswitch forward iifname \"{ap_iface}\" oifname != \"{vpn_iface}\" drop"
    );
    let out = Command::new("pkexec")
        .args(["nft", &rules])
        .output()
        .map_err(|e| format!("Failed to invoke pkexec nft: {e}"))?;
    if out.status.success() {
        Ok(format!(
            "Fail-Closed active: {ap_iface} forwarding restricted strictly to {vpn_iface}."
        ))
    } else {
        Err(format!(
            "Backend/privilege required for Fail-Closed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
