//! Evidence-based Internet Diagnostics (DNS, IPv4/IPv6, TCP/TLS/HTTP, Domestic vs International).
//! Never claims certainty about root cause without supporting evidence.

use serde::{Deserialize, Serialize};
use std::{
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbableDiagnosis {
    Healthy,
    WeakSignal,
    DnsFailure,
    RoutingOrGatewayIssue,
    Ipv6Issue,
    DestinationSpecificIssue,
    PossibleRegionalDisruption,
    Inconclusive,
}

impl ProbableDiagnosis {
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "Healthy Connectivity",
            Self::WeakSignal => "Possible Weak Signal / Radio Issue",
            Self::DnsFailure => "Possible DNS Resolution Failure",
            Self::RoutingOrGatewayIssue => "Possible Routing / Gateway Problem",
            Self::Ipv6Issue => "Possible IPv6-only Connectivity Issue",
            Self::DestinationSpecificIssue => "Possible Destination-Specific Issue",
            Self::PossibleRegionalDisruption => "Possible Regional / International Disruption",
            Self::Inconclusive => "Inconclusive (Insufficient Evidence)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointProbe {
    pub label: String,
    pub category: String, // "domestic" | "international"
    pub host: String,
    pub port: u16,
    pub dns_ms: Option<u128>,
    pub tcp_ms: Option<u128>,
    pub reachable: bool,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternetDiagReport {
    pub interface: String,
    pub probes: Vec<EndpointProbe>,
    pub diagnosis: ProbableDiagnosis,
    pub evidence_summary: String,
}

pub fn probe_tcp_endpoint(label: &str, category: &str, host: &str, port: u16) -> EndpointProbe {
    let dns_start = Instant::now();
    let addrs = match (host, port).to_socket_addrs() {
        Ok(iter) => iter.collect::<Vec<_>>(),
        Err(e) => {
            return EndpointProbe {
                label: label.into(),
                category: category.into(),
                host: host.into(),
                port,
                dns_ms: None,
                tcp_ms: None,
                reachable: false,
                error: format!("DNS failed: {e}"),
            };
        }
    };
    let dns_ms = Some(dns_start.elapsed().as_millis());
    let Some(addr) = addrs.first().copied() else {
        return EndpointProbe {
            label: label.into(),
            category: category.into(),
            host: host.into(),
            port,
            dns_ms,
            tcp_ms: None,
            reachable: false,
            error: "DNS returned zero addresses.".into(),
        };
    };

    let tcp_start = Instant::now();
    match TcpStream::connect_timeout(&addr, Duration::from_secs(4)) {
        Ok(_) => EndpointProbe {
            label: label.into(),
            category: category.into(),
            host: host.into(),
            port,
            dns_ms,
            tcp_ms: Some(tcp_start.elapsed().as_millis()),
            reachable: true,
            error: String::new(),
        },
        Err(e) => EndpointProbe {
            label: label.into(),
            category: category.into(),
            host: host.into(),
            port,
            dns_ms,
            tcp_ms: None,
            reachable: false,
            error: format!("TCP connect failed: {e}"),
        },
    }
}

pub fn classify_probes(
    probes: &[EndpointProbe],
    signal_percent: u32,
) -> (ProbableDiagnosis, String) {
    if probes.is_empty() {
        return (
            ProbableDiagnosis::Inconclusive,
            "No probes executed.".into(),
        );
    }
    let any_dns_ok = probes.iter().any(|p| p.dns_ms.is_some());
    let dom_ok = probes
        .iter()
        .filter(|p| p.category == "domestic")
        .any(|p| p.reachable);
    let dom_total = probes.iter().filter(|p| p.category == "domestic").count();
    let intl_ok = probes
        .iter()
        .filter(|p| p.category == "international")
        .any(|p| p.reachable);
    let intl_total = probes
        .iter()
        .filter(|p| p.category == "international")
        .count();
    let all_ok = probes.iter().all(|p| p.reachable);

    if all_ok {
        return (
            ProbableDiagnosis::Healthy,
            "All domestic and international probes succeeded.".into(),
        );
    }
    if !any_dns_ok {
        return (
            ProbableDiagnosis::DnsFailure,
            "All probes failed during DNS resolution.".into(),
        );
    }
    if dom_total > 0 && dom_ok && intl_total > 0 && !intl_ok {
        return (
            ProbableDiagnosis::PossibleRegionalDisruption,
            "Domestic destinations are reachable while international destinations failed.".into(),
        );
    }
    if signal_percent > 0 && signal_percent < 20 && !dom_ok && !intl_ok {
        return (
            ProbableDiagnosis::WeakSignal,
            format!(
                "Both domestic and international probes failed with low radio signal ({signal_percent}%)."
            ),
        );
    }
    if dom_ok || intl_ok {
        return (
            ProbableDiagnosis::DestinationSpecificIssue,
            "Some destinations succeeded while others failed.".into(),
        );
    }
    (
        ProbableDiagnosis::RoutingOrGatewayIssue,
        "DNS resolved but all TCP probes failed.".into(),
    )
}

pub fn run_quick_diagnostics(interface: &str, signal_percent: u32) -> InternetDiagReport {
    let targets = [
        ("Iran Domestic (Aparat)", "domestic", "aparat.com", 443),
        ("Iran Domestic (Snapp)", "domestic", "snapp.ir", 443),
        (
            "International (Cloudflare)",
            "international",
            "1.1.1.1",
            443,
        ),
        ("International (Quad9)", "international", "9.9.9.9", 443),
    ];
    let probes: Vec<EndpointProbe> = targets
        .iter()
        .map(|(label, cat, host, port)| probe_tcp_endpoint(label, cat, host, *port))
        .collect();
    let (diagnosis, evidence_summary) = classify_probes(&probes, signal_percent);
    InternetDiagReport {
        interface: interface.to_string(),
        probes,
        diagnosis,
        evidence_summary,
    }
}
