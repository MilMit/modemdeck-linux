//! Data Package & Usage Manager + Lightweight Per-App Socket Monitor.
//! Clearly distinguishes local OS counters from real operator-reported remaining quota.

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DataPlanConfig {
    pub sim_hash: String,
    pub operator_name: String,
    pub cycle_days: u32,
    pub quota_mb: Option<u64>,
    pub alert_percent: u8,
    pub operator_reported_remaining_mb: Option<u64>,
    pub expiration_date: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppTrafficEntry {
    pub pid: u32,
    pub process_name: String,
    pub connection_count: usize,
    pub estimated_rx_bytes: u64,
    pub estimated_tx_bytes: u64,
}

/// Reads active TCP/UDP socket counts from `/proc/<pid>/fd` and enriches
/// with per-socket `bytes_received` / `bytes_sent` from `ss -tinp` when available (no packet capture).
pub fn sample_active_app_sockets() -> Vec<AppTrafficEntry> {
    let mut ss_stats: HashMap<String, (u64, u64)> = HashMap::new();
    if let Ok(out) = std::process::Command::new("ss").args(["-tinp"]).output()
        && out.status.success()
    {
        let raw = String::from_utf8_lossy(&out.stdout);
        let mut current_proc = String::new();
        for line in raw.lines() {
            if line.contains("users:((\"") {
                if let Some(start) = line.find("users:((\"") {
                    let rest = &line[start + 9..];
                    if let Some(end) = rest.find('"') {
                        current_proc = rest[..end].to_string();
                    }
                }
            } else if !current_proc.is_empty() {
                let rx = extract_ss_metric(line, "bytes_received:");
                let tx = extract_ss_metric(line, "bytes_sent:");
                if rx > 0 || tx > 0 {
                    let e = ss_stats.entry(current_proc.clone()).or_insert((0, 0));
                    e.0 += rx;
                    e.1 += tx;
                }
            }
        }
    }

    let mut result = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return result;
    };
    let mut by_name: HashMap<String, (u32, usize)> = HashMap::new();

    for entry in entries.flatten() {
        let fname = entry.file_name();
        let Ok(pid) = fname.to_string_lossy().parse::<u32>() else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = fs::read_dir(fd_dir) else {
            continue;
        };
        let mut sockets = 0usize;
        for fd in fds.flatten() {
            if let Ok(target) = fs::read_link(fd.path())
                && target.to_string_lossy().starts_with("socket:[")
            {
                sockets += 1;
            }
        }
        if sockets > 0 {
            let comm = fs::read_to_string(entry.path().join("comm"))
                .unwrap_or_else(|_| format!("pid-{pid}"))
                .trim()
                .to_string();
            let item = by_name.entry(comm).or_insert((pid, 0));
            item.1 += sockets;
        }
    }

    for (process_name, (pid, connection_count)) in by_name {
        let (rx, tx) = ss_stats.get(&process_name).copied().unwrap_or((0, 0));
        result.push(AppTrafficEntry {
            pid,
            process_name,
            connection_count,
            estimated_rx_bytes: rx,
            estimated_tx_bytes: tx,
        });
    }
    result.sort_by_key(|a| {
        std::cmp::Reverse((
            a.estimated_rx_bytes + a.estimated_tx_bytes,
            a.connection_count,
        ))
    });
    result.truncate(25);
    result
}

fn extract_ss_metric(line: &str, key: &str) -> u64 {
    line.split_whitespace()
        .find_map(|tok| tok.strip_prefix(key))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
}
