//! Network Failover Policy & Health Evaluator (WWAN / Wi-Fi / Ethernet / Multi-modem).
//! Never changes OS routing without explicit user consent.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FailoverPolicy {
    #[default]
    PreferWifi,
    PreferEthernet,
    PreferCellular,
    LowestLatency,
    MostStable,
    UserDefined,
}

impl FailoverPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::PreferWifi => "Prefer Wi-Fi",
            Self::PreferEthernet => "Prefer Ethernet",
            Self::PreferCellular => "Prefer Mobile Broadband",
            Self::LowestLatency => "Lowest Latency",
            Self::MostStable => "Most Stable",
            Self::UserDefined => "User-Defined Priority",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FailoverConfig {
    pub enabled: bool,
    pub policy: FailoverPolicy,
    pub auto_failback: bool,
    pub cooldown_secs: u64,
    pub min_failure_secs: u64,
    pub metered_aware: bool,
    pub last_switch_unix: u64,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default until user opts in
            policy: FailoverPolicy::PreferWifi,
            auto_failback: true,
            cooldown_secs: 60,
            min_failure_secs: 15,
            metered_aware: true,
            last_switch_unix: 0,
        }
    }
}

impl FailoverConfig {
    pub fn can_switch(&self, now_unix: u64) -> bool {
        self.enabled && now_unix.saturating_sub(self.last_switch_unix) >= self.cooldown_secs
    }
}

pub fn apply_failover_policy(policy: FailoverPolicy) -> Result<String, String> {
    let (wifi_metric, eth_metric, gsm_metric) = match policy {
        FailoverPolicy::PreferEthernet => ("300", "50", "500"),
        FailoverPolicy::PreferWifi => ("100", "200", "500"),
        FailoverPolicy::PreferCellular => ("600", "600", "50"),
        _ => ("150", "100", "400"),
    };
    let out = std::process::Command::new("nmcli")
        .args(["-t", "-f", "UUID,TYPE", "connection", "show", "--active"])
        .output()
        .map_err(|e| format!("nmcli unavailable: {e}"))?;
    if !out.status.success() {
        return Err("Could not query active NetworkManager connections.".into());
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let mut updated = 0usize;
    for line in raw.lines() {
        let Some((uuid, kind)) = line.split_once(':') else {
            continue;
        };
        let metric = match kind {
            "802-11-wireless" => wifi_metric,
            "802-3-ethernet" => eth_metric,
            "gsm" => gsm_metric,
            _ => continue,
        };
        let _ = std::process::Command::new("nmcli")
            .args([
                "connection",
                "modify",
                "uuid",
                uuid,
                "ipv4.route-metric",
                metric,
            ])
            .status();
        updated += 1;
    }
    Ok(format!(
        "Updated route metrics on {updated} active connection(s) for {}.",
        policy.label()
    ))
}
