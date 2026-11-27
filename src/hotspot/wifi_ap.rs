use serde::{Deserialize, Serialize};
use std::{fs, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotspotBand {
    Auto,
    Bg24Ghz,
    A5Ghz,
    Ghz6,
}

impl HotspotBand {
    pub fn nmcli_value(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Bg24Ghz => Some("bg"),
            Self::A5Ghz => Some("a"),
            Self::Ghz6 => Some("6GHz"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WifiApCapabilities {
    pub wifi_device: Option<String>,
    pub ap_supported: bool,
    pub concurrent_sta_ap: bool,
    pub active_wifi_connected: bool,
    pub band_5ghz_supported: bool,
    pub band_6ghz_supported: bool,
    pub wpa3_supported: bool,
    pub status_note: String,
}

#[derive(Debug, Clone, Default)]
pub struct ClientEntry {
    pub mac: String,
    pub ip: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Default)]
pub struct HotspotStatus {
    pub active: bool,
    pub ssid: String,
    pub interface: String,
    pub connected_clients: Vec<String>,
    pub client_entries: Vec<ClientEntry>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

pub fn detect_capabilities() -> WifiApCapabilities {
    let mut caps = WifiApCapabilities::default();
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "device"])
        .output()
    else {
        caps.status_note = "NetworkManager (nmcli) is not available.".into();
        return caps;
    };

    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 && parts[1] == "wifi" {
            caps.wifi_device = Some(parts[0].to_string());
            caps.active_wifi_connected = parts[2] == "connected";
            break;
        }
    }

    let Some(ref dev) = caps.wifi_device else {
        caps.status_note = "No Wi-Fi adapter was detected by NetworkManager.".into();
        return caps;
    };

    if let Ok(iw_out) = Command::new("iw").args(["list"]).output() {
        let iw_text = String::from_utf8_lossy(&iw_out.stdout);
        caps.ap_supported = iw_text.contains("* AP");
        caps.concurrent_sta_ap = iw_text.contains("valid interface combinations")
            && iw_text.contains("AP")
            && iw_text.contains("managed");
        caps.band_5ghz_supported = iw_text.contains("Band 2:");
        caps.band_6ghz_supported = iw_text.contains("Band 4:") || iw_text.contains("5955");
        caps.wpa3_supported = iw_text.contains("SAE");
    } else {
        // Fallback: NetworkManager reports wifi device; assume standard 2.4 GHz AP
        caps.ap_supported = true;
    }

    if !caps.ap_supported {
        caps.status_note =
            format!("Wi-Fi adapter '{dev}' does not report AP (Hotspot) mode support.");
    } else if caps.active_wifi_connected && !caps.concurrent_sta_ap {
        caps.status_note = format!(
            "Wi-Fi adapter '{dev}' is connected to a Wi-Fi network and does not report concurrent STA+AP support; enabling Hotspot may disconnect Wi-Fi."
        );
    } else {
        caps.status_note = format!("Wi-Fi adapter '{dev}' is ready for Mobile Hotspot.");
    }
    caps
}

pub fn start_hotspot(
    ssid: &str,
    password: &str,
    band: HotspotBand,
    channel: Option<u32>,
    wpa3: bool,
) -> Result<String, String> {
    let ssid = ssid.trim();
    if ssid.is_empty() {
        return Err("Hotspot SSID cannot be empty.".into());
    }
    if password.len() < 8 {
        return Err("Hotspot password must be at least 8 characters.".into());
    }
    let caps = detect_capabilities();
    let dev = caps
        .wifi_device
        .ok_or_else(|| "No Wi-Fi device found.".to_string())?;
    if !caps.ap_supported {
        return Err(caps.status_note);
    }

    let mut args = vec![
        "device", "wifi", "hotspot", "ifname", &dev, "ssid", ssid, "password", password,
    ];
    let band_str;
    if let Some(b) = band.nmcli_value() {
        band_str = b.to_string();
        args.extend(["band", &band_str]);
    }
    let chan_str;
    if let Some(ch) = channel
        && ch > 0
    {
        chan_str = ch.to_string();
        args.extend(["channel", &chan_str]);
    }

    let out = Command::new("nmcli")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start hotspot via nmcli: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            "NetworkManager rejected hotspot creation.".into()
        } else {
            err
        });
    }

    if wpa3 && caps.wpa3_supported {
        let _ = Command::new("nmcli")
            .args([
                "connection",
                "modify",
                "Hotspot",
                "802-11-wireless-security.key-mgmt",
                "sae",
            ])
            .output();
    }
    Ok(format!("Hotspot '{ssid}' started on {dev}."))
}

pub fn stop_hotspot() -> Result<(), String> {
    let out = Command::new("nmcli")
        .args(["connection", "down", "Hotspot"])
        .output()
        .map_err(|e| format!("Failed to stop hotspot: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            "Could not stop Hotspot.".into()
        } else {
            err
        })
    }
}

pub fn current_status() -> HotspotStatus {
    let mut status = HotspotStatus::default();
    let Ok(out) = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "NAME,TYPE,DEVICE",
            "connection",
            "show",
            "--active",
        ])
        .output()
    else {
        return status;
    };
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("Hotspot") {
            status.active = true;
            status.interface = parts[2].to_string();
            break;
        }
    }
    if status.active && !status.interface.is_empty() {
        if let Ok(iw_out) = Command::new("iw")
            .args(["dev", &status.interface, "station", "dump"])
            .output()
        {
            for line in String::from_utf8_lossy(&iw_out.stdout).lines() {
                if let Some(mac) = line.trim().strip_prefix("Station ")
                    && let Some(addr) = mac.split_whitespace().next()
                {
                    status.connected_clients.push(addr.to_string());
                }
            }
        }
        let arp = fs::read_to_string("/proc/net/arp").unwrap_or_default();
        for mac in &status.connected_clients {
            let ip = arp
                .lines()
                .skip(1)
                .find_map(|l| {
                    let cols: Vec<&str> = l.split_whitespace().collect();
                    if cols.len() >= 4 && cols[3].eq_ignore_ascii_case(mac) {
                        Some(cols[0].to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "—".into());
            status.client_entries.push(ClientEntry {
                mac: mac.clone(),
                ip,
                hostname: "Limited ID (no lease name)".into(),
            });
        }
        status.rx_bytes = read_stat(&status.interface, "rx_bytes");
        status.tx_bytes = read_stat(&status.interface, "tx_bytes");
    }
    status
}

fn read_stat(iface: &str, name: &str) -> u64 {
    fs::read_to_string(format!("/sys/class/net/{iface}/statistics/{name}"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}
