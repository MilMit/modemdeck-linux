use serde::{Deserialize, Serialize};
use std::{
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::core::settings::StabilityThresholds;
use crate::model::ModemInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StabilityEventKind {
    Disconnect,
    HighLatency,
    PacketLoss,
    SignalDegradation,
    DnsFailure,
    InternetFailure,
    RegistrationFailure,
    BandChanged,
    CellChanged,
    OperatorChanged,
    CaStateChanged,
    ModemReset,
}

impl StabilityEventKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnect => "Disconnected",
            Self::HighLatency => "High Latency",
            Self::PacketLoss => "Packet Loss",
            Self::SignalDegradation => "Signal Degradation",
            Self::DnsFailure => "DNS Failure",
            Self::InternetFailure => "Internet Connectivity Failure",
            Self::RegistrationFailure => "Registration Failure",
            Self::BandChanged => "Band Changed",
            Self::CellChanged => "Cell Changed",
            Self::OperatorChanged => "Operator Changed",
            Self::CaStateChanged => "CA State Changed",
            Self::ModemReset => "Unexpected Modem Reset",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityAlert {
    pub timestamp_unix: u64,
    pub kind: StabilityEventKind,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectivityProbe {
    pub dns_ok: bool,
    pub internet_ok: bool,
    pub latency_ms: Option<f64>,
    pub packet_loss_percent: f64,
}

/// Multi-target connectivity check so a single external server outage is NOT
/// misreported as a modem or internet failure.
pub fn probe_internet_health(timeout: Duration) -> ConnectivityProbe {
    let targets = [("1.1.1.1", 443), ("8.8.8.8", 443), ("9.9.9.9", 443)];
    let dns_ok = "one.one.one.one:443".to_socket_addrs().is_ok()
        || "dns.google:443".to_socket_addrs().is_ok();

    let mut rtts = Vec::new();
    let mut failures = 0usize;
    for (host, port) in targets {
        let addr: SocketAddr = format!("{host}:{port}").parse().unwrap();
        let t0 = Instant::now();
        if TcpStream::connect_timeout(&addr, timeout).is_ok() {
            rtts.push(t0.elapsed().as_secs_f64() * 1000.0);
        } else {
            failures += 1;
        }
    }

    let internet_ok = !rtts.is_empty();
    let latency_ms = if !rtts.is_empty() {
        Some(rtts.iter().sum::<f64>() / rtts.len() as f64)
    } else {
        None
    };
    let packet_loss_percent = (failures as f64 / targets.len() as f64) * 100.0;

    ConnectivityProbe {
        dns_ok,
        internet_ok,
        latency_ms,
        packet_loss_percent,
    }
}

#[derive(Default)]
pub struct StabilityTracker {
    pub alerts: Vec<StabilityAlert>,
    last_notify_unix: u64,
    prev_state: Option<i32>,
    prev_band: Option<u32>,
    prev_pci: Option<u32>,
    prev_operator: String,
    prev_ca: Option<bool>,
}

impl StabilityTracker {
    pub fn evaluate(
        &mut self,
        modem: &ModemInfo,
        probe: Option<&ConnectivityProbe>,
        thresholds: &StabilityThresholds,
    ) -> Vec<StabilityAlert> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut new_alerts = Vec::new();

        if let Some(prev) = self.prev_state {
            if prev == 11 && modem.state != 11 {
                new_alerts.push(StabilityAlert {
                    timestamp_unix: now,
                    kind: StabilityEventKind::Disconnect,
                    message: format!("Modem left Connected state (now {}).", modem.state_label()),
                });
            }
            if prev >= 6 && modem.state == 1 {
                new_alerts.push(StabilityAlert {
                    timestamp_unix: now,
                    kind: StabilityEventKind::ModemReset,
                    message: "Unexpected modem re-initialization detected.".into(),
                });
            }
            if modem.state == 3 || modem.state == -1 {
                new_alerts.push(StabilityAlert {
                    timestamp_unix: now,
                    kind: StabilityEventKind::RegistrationFailure,
                    message: format!("Modem state is {}.", modem.state_label()),
                });
            }
        }
        self.prev_state = Some(modem.state);

        let cur_band = modem
            .detected_serving_lte_band()
            .or_else(|| modem.detected_serving_nr_band());
        if let (Some(p), Some(c)) = (self.prev_band, cur_band)
            && p != c
        {
            new_alerts.push(StabilityAlert {
                timestamp_unix: now,
                kind: StabilityEventKind::BandChanged,
                message: format!("Serving band changed: B{p} → B{c}."),
            });
        }
        if cur_band.is_some() {
            self.prev_band = cur_band;
        }

        if let (Some(p), Some(c)) = (self.prev_pci, modem.vendor.pci)
            && p != c
        {
            new_alerts.push(StabilityAlert {
                timestamp_unix: now,
                kind: StabilityEventKind::CellChanged,
                message: format!("Serving cell PCI changed: {p} → {c}."),
            });
        }
        if modem.vendor.pci.is_some() {
            self.prev_pci = modem.vendor.pci;
        }

        let op = modem.operator_label();
        if !self.prev_operator.is_empty() && self.prev_operator != op && op != "No operator" {
            new_alerts.push(StabilityAlert {
                timestamp_unix: now,
                kind: StabilityEventKind::OperatorChanged,
                message: format!("Operator changed: {} → {}.", self.prev_operator, op),
            });
        }
        if op != "No operator" {
            self.prev_operator = op;
        }

        let ca_now = modem.carrier_aggregation_active();
        if let Some(ca_prev) = self.prev_ca
            && ca_prev != ca_now
        {
            new_alerts.push(StabilityAlert {
                timestamp_unix: now,
                kind: StabilityEventKind::CaStateChanged,
                message: if ca_now {
                    "Carrier Aggregation became active (4G+).".into()
                } else {
                    "Carrier Aggregation became inactive.".into()
                },
            });
        }
        self.prev_ca = Some(ca_now);

        let (_, sig) = modem.preferred_signal();
        if let Some(rsrp) = sig.rsrp
            && rsrp < thresholds.min_rsrp_dbm
        {
            new_alerts.push(StabilityAlert {
                timestamp_unix: now,
                kind: StabilityEventKind::SignalDegradation,
                message: format!(
                    "RSRP dropped to {rsrp:.1} dBm (below {:.1} dBm).",
                    thresholds.min_rsrp_dbm
                ),
            });
        }
        if let Some(snr) = sig.snr
            && snr < thresholds.min_snr_db
        {
            new_alerts.push(StabilityAlert {
                timestamp_unix: now,
                kind: StabilityEventKind::SignalDegradation,
                message: format!(
                    "SNR dropped to {snr:.1} dB (below {:.1} dB).",
                    thresholds.min_snr_db
                ),
            });
        }

        if let Some(p) = probe {
            if !p.dns_ok {
                new_alerts.push(StabilityAlert {
                    timestamp_unix: now,
                    kind: StabilityEventKind::DnsFailure,
                    message: "DNS resolution failed across test resolvers.".into(),
                });
            }
            if !p.internet_ok {
                new_alerts.push(StabilityAlert {
                    timestamp_unix: now,
                    kind: StabilityEventKind::InternetFailure,
                    message: "All external connectivity probes failed.".into(),
                });
            }
            if let Some(lat) = p.latency_ms
                && lat > thresholds.high_latency_ms as f64
            {
                new_alerts.push(StabilityAlert {
                    timestamp_unix: now,
                    kind: StabilityEventKind::HighLatency,
                    message: format!("High latency detected: {lat:.0} ms."),
                });
            }
            if p.packet_loss_percent > thresholds.packet_loss_percent {
                new_alerts.push(StabilityAlert {
                    timestamp_unix: now,
                    kind: StabilityEventKind::PacketLoss,
                    message: format!("Packet loss detected: {:.0}%.", p.packet_loss_percent),
                });
            }
        }

        self.alerts.extend(new_alerts.clone());
        if self.alerts.len() > 200 {
            let drain = self.alerts.len() - 200;
            self.alerts.drain(0..drain);
        }
        new_alerts
    }

    pub fn should_notify(&mut self, thresholds: &StabilityThresholds) -> bool {
        if thresholds.silent_mode {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now.saturating_sub(self.last_notify_unix) >= thresholds.notification_cooldown_secs {
            self.last_notify_unix = now;
            true
        } else {
            false
        }
    }
}
