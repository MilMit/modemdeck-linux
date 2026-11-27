use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::core::settings::SpeedTestServer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingMode {
    Direct,
    VpnActive,
    Unverified,
}

impl RoutingMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Direct => "Direct (Modem Interface)",
            Self::VpnActive => "VPN Active (Tunnel Detected)",
            Self::Unverified => "Unverified Route",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedTestConfig {
    pub interface: String,
    pub server: SpeedTestServer,
    pub run_download: bool,
    pub run_upload: bool,
    pub data_cap_mb: u64,
    pub probe_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpeedTestResult {
    pub interface: String,
    pub local_ip: String,
    pub routing_mode: String,
    pub vpn_interfaces: Vec<String>,
    pub server_name: String,
    pub ipv4_ok: bool,
    pub ipv6_ok: bool,
    pub dns_ok: bool,
    pub dns_ms: Option<f64>,
    pub tcp_ok: bool,
    pub https_ok: bool,
    pub idle_latency_ms: Option<f64>,
    pub loaded_latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss_percent: Option<f64>,
    pub download_mbps: Option<f64>,
    pub upload_mbps: Option<f64>,
    pub bytes_used: u64,
    pub stopped_by_cap: bool,
    pub cancelled: bool,
    pub error: Option<String>,
}

pub fn default_servers() -> Vec<SpeedTestServer> {
    vec![
        SpeedTestServer {
            name: "Cloudflare Edge (International)".into(),
            category: "international".into(),
            download_url: "https://speed.cloudflare.com/__down?bytes=10000000".into(),
            upload_url: "https://speed.cloudflare.com/__up".into(),
            ping_host: "1.1.1.1".into(),
            ping_port: 443,
        },
        SpeedTestServer {
            name: "Hetzner Nuremberg (International)".into(),
            category: "international".into(),
            download_url: "https://nbg1-speed.hetzner.com/10MB.bin".into(),
            upload_url: String::new(),
            ping_host: "nbg1-speed.hetzner.com".into(),
            ping_port: 443,
        },
        SpeedTestServer {
            name: "ArvanCloud / Domestic Probe".into(),
            category: "domestic".into(),
            download_url: "https://www.arvancloud.ir/favicon.ico".into(),
            upload_url: String::new(),
            ping_host: "185.143.233.120".into(),
            ping_port: 443,
        },
    ]
}

pub fn detect_vpn_interfaces() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("tun")
                || lower.starts_with("tap")
                || lower.starts_with("wg")
                || lower.starts_with("ppp")
                || lower.contains("tailscale")
                || lower.contains("nordlynx")
                || lower.contains("proton")
            {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

pub fn interface_ip_addresses(interface: &str) -> (Option<IpAddr>, Option<IpAddr>) {
    let ipv4 = parse_ip_from_cmd(interface, "-4");
    let ipv6 = parse_ip_from_cmd(interface, "-6");
    (ipv4, ipv6)
}

fn parse_ip_from_cmd(interface: &str, family_flag: &str) -> Option<IpAddr> {
    let out = Command::new("ip")
        .args([
            "-o",
            family_flag,
            "addr",
            "show",
            "dev",
            interface,
            "scope",
            "global",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for (idx, part) in parts.iter().enumerate() {
            if (*part == "inet" || *part == "inet6") && idx + 1 < parts.len() {
                let addr_cidr = parts[idx + 1];
                if let Some(ip_str) = addr_cidr.split('/').next()
                    && let Ok(ip) = ip_str.parse::<IpAddr>()
                {
                    return Some(ip);
                }
            }
        }
    }
    None
}

pub fn run_speed_test(config: &SpeedTestConfig, cancel: &Arc<AtomicBool>) -> SpeedTestResult {
    let mut result = SpeedTestResult {
        interface: config.interface.clone(),
        server_name: config.server.name.clone(),
        ..SpeedTestResult::default()
    };

    if config.interface.trim().is_empty() {
        result.routing_mode = RoutingMode::Unverified.label().into();
        result.error = Some("No cellular network interface was detected for this modem.".into());
        return result;
    }

    let (ipv4, ipv6) = interface_ip_addresses(&config.interface);
    result.ipv4_ok = ipv4.is_some();
    result.ipv6_ok = ipv6.is_some();
    let Some(local_ip) = ipv4.or(ipv6) else {
        result.routing_mode = RoutingMode::Unverified.label().into();
        result.error = Some(format!(
            "Interface '{}' has no global IPv4/IPv6 address. Traffic cannot be guaranteed to pass through the selected modem.",
            config.interface
        ));
        return result;
    };
    result.local_ip = local_ip.to_string();

    let vpns = detect_vpn_interfaces();
    result.vpn_interfaces = vpns.clone();
    result.routing_mode = if vpns.is_empty() {
        RoutingMode::Direct.label().into()
    } else {
        RoutingMode::VpnActive.label().into()
    };

    // DNS check
    let dns_start = Instant::now();
    let resolved: Vec<SocketAddr> =
        format!("{}:{}", config.server.ping_host, config.server.ping_port)
            .to_socket_addrs()
            .map(|iter| iter.collect())
            .unwrap_or_default();
    if !resolved.is_empty() {
        result.dns_ok = true;
        result.dns_ms = Some(dns_start.elapsed().as_secs_f64() * 1000.0);
    }

    // TCP Latency / Jitter / Packet Loss (works even when ICMP is blocked)
    let target_addr = resolved
        .into_iter()
        .find(|a| a.is_ipv4() == local_ip.is_ipv4());
    if let Some(target) = target_addr {
        let probes = config.probe_count.clamp(3, 20);
        let mut rtts = Vec::new();
        let mut failures = 0usize;
        for _ in 0..probes {
            if cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                return result;
            }
            let t0 = Instant::now();
            match tcp_probe_bound(local_ip, target, Duration::from_secs(3)) {
                Ok(()) => rtts.push(t0.elapsed().as_secs_f64() * 1000.0),
                Err(_) => failures += 1,
            }
        }
        result.tcp_ok = !rtts.is_empty();
        result.packet_loss_percent = Some((failures as f64 / probes as f64) * 100.0);
        if !rtts.is_empty() {
            let avg = rtts.iter().sum::<f64>() / rtts.len() as f64;
            result.idle_latency_ms = Some(avg);
            if rtts.len() >= 2 {
                let mut diffs = Vec::with_capacity(rtts.len() - 1);
                for w in rtts.windows(2) {
                    diffs.push((w[1] - w[0]).abs());
                }
                result.jitter_ms = Some(diffs.iter().sum::<f64>() / diffs.len() as f64);
            }
        }
    }

    let cap_bytes = config.data_cap_mb.saturating_mul(1024 * 1024);

    let client = match reqwest::blocking::Client::builder()
        .local_address(local_ip)
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("ModemDeck/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            result.error = Some(format!("Could not bind HTTP client to {local_ip}: {e}"));
            return result;
        }
    };

    // Download test
    if config.run_download && !config.server.download_url.is_empty() {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            return result;
        }
        let t0 = Instant::now();
        match client.get(&config.server.download_url).send() {
            Ok(mut resp) if resp.status().is_success() => {
                result.https_ok = true;
                let mut buf = [0u8; 32 * 1024];
                let mut downloaded: u64 = 0;
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        result.cancelled = true;
                        break;
                    }
                    if cap_bytes > 0 && result.bytes_used.saturating_add(downloaded) >= cap_bytes {
                        result.stopped_by_cap = true;
                        break;
                    }
                    match resp.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            downloaded = downloaded.saturating_add(n as u64);
                        }
                        Err(_) => break,
                    }
                }
                let secs = t0.elapsed().as_secs_f64();
                result.bytes_used = result.bytes_used.saturating_add(downloaded);
                if secs > 0.05 && downloaded > 0 {
                    result.download_mbps = Some((downloaded as f64 * 8.0) / (secs * 1_000_000.0));
                }
            }
            Ok(resp) => {
                result.error = Some(format!("Download server returned HTTP {}", resp.status()));
            }
            Err(e) => {
                result.error = Some(format!("Download test failed: {e}"));
            }
        }
    }

    // Loaded latency check during/after transfer
    if let Some(target) = target_addr {
        let t0 = Instant::now();
        if tcp_probe_bound(local_ip, target, Duration::from_secs(3)).is_ok() {
            result.loaded_latency_ms = Some(t0.elapsed().as_secs_f64() * 1000.0);
        }
    }

    // Upload test
    if config.run_upload
        && !config.server.upload_url.is_empty()
        && !result.stopped_by_cap
        && !result.cancelled
    {
        let remaining = if cap_bytes > 0 {
            cap_bytes.saturating_sub(result.bytes_used)
        } else {
            4 * 1024 * 1024
        };
        let upload_size = (remaining as usize).min(4 * 1024 * 1024);
        if upload_size >= 64 * 1024 {
            let payload = vec![0x5Au8; upload_size];
            let t0 = Instant::now();
            match client.post(&config.server.upload_url).body(payload).send() {
                Ok(resp) if resp.status().is_success() => {
                    result.https_ok = true;
                    let secs = t0.elapsed().as_secs_f64();
                    result.bytes_used = result.bytes_used.saturating_add(upload_size as u64);
                    if secs > 0.05 {
                        result.upload_mbps =
                            Some((upload_size as f64 * 8.0) / (secs * 1_000_000.0));
                    }
                }
                Ok(resp) => {
                    if result.error.is_none() {
                        result.error =
                            Some(format!("Upload server returned HTTP {}", resp.status()));
                    }
                }
                Err(e) => {
                    if result.error.is_none() {
                        result.error = Some(format!("Upload test failed: {e}"));
                    }
                }
            }
        } else if cap_bytes > 0 {
            result.stopped_by_cap = true;
        }
    }

    result
}

fn tcp_probe_bound(local_ip: IpAddr, target: SocketAddr, timeout: Duration) -> std::io::Result<()> {
    let mut stream = TcpStream::connect_timeout(&target, timeout)?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.write(&[]);
    let _ = local_ip;
    Ok(())
}

pub fn estimate_test_mb(
    run_download: bool,
    run_upload: bool,
    bands_count: usize,
    cap_mb: u64,
) -> u64 {
    let per_band = match (run_download, run_upload) {
        (true, true) => 14,
        (true, false) => 10,
        (false, true) => 4,
        (false, false) => 0,
    };
    let est = (per_band as u64).saturating_mul(bands_count.max(1) as u64);
    if cap_mb > 0 { est.min(cap_mb) } else { est }
}
