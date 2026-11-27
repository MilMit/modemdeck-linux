//! Independent Command Line Interface for ModemDeck.
//! Shares the same core engines as the Slint GUI.

use crate::{
    core::settings::Settings,
    diagnostics, doctor,
    hardware::{HardwareDatabase, LaptopInfo},
    model::band_label,
    modem::ModemManagerClient,
    operators::OperatorDatabase,
    services::apn,
    storage::history_db,
};
use serde_json::json;

pub fn maybe_run_cli(args: &[String]) -> Option<i32> {
    if args.len() < 2 {
        return None;
    }
    let first = args[1].as_str();
    let known = matches!(
        first,
        "status"
            | "list-modems"
            | "modem"
            | "signal"
            | "bands"
            | "network"
            | "operator"
            | "apn"
            | "speedtest"
            | "doctor"
            | "history"
            | "diagnostics"
            | "completions"
            | "--help"
            | "-h"
            | "help"
    );
    if !known {
        return None;
    }
    Some(run_cli(&args[1..]))
}

pub fn run_cli(args: &[String]) -> i32 {
    let json_out = args.iter().any(|a| a == "--json");
    let modem_idx = args
        .windows(2)
        .find(|w| w[0] == "--modem")
        .and_then(|w| w[1].parse::<usize>().ok())
        .unwrap_or(0);

    let positional: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'))
        .collect();

    if args.iter().any(|a| a == "--help" || a == "-h") || positional.first() == Some(&"help") {
        print_help();
        return 0;
    }

    match positional.as_slice() {
        ["completions", shell] => {
            print_completions(shell);
            0
        }
        ["completions"] => {
            print_completions("bash");
            0
        }
        ["status"] => cmd_status(json_out, modem_idx),
        ["list-modems"] => cmd_list_modems(json_out),
        ["modem", "info"] => cmd_modem_info(json_out, modem_idx),
        ["signal"] => cmd_signal(json_out, modem_idx),
        ["bands", "list"] => cmd_bands_list(json_out, modem_idx),
        ["bands", "lock", raw] => cmd_bands_lock(modem_idx, raw),
        ["bands", "unlock"] => cmd_bands_unlock(modem_idx),
        ["bands", "test"] => {
            println!(
                "Band quality test requires interactive confirmation; use the Analyzer tab or pass --confirm."
            );
            0
        }
        ["network", "scan"] => cmd_network_scan(json_out, modem_idx),
        ["operator", "info"] => cmd_operator_info(json_out, modem_idx),
        ["apn", "list"] => cmd_apn_list(json_out),
        ["doctor"] => cmd_doctor(json_out),
        ["history"] => cmd_history(json_out),
        ["diagnostics", "export"] => cmd_diagnostics_export(),
        ["speedtest"] => cmd_speedtest(json_out, modem_idx),
        _ => {
            eprintln!("Unknown CLI command. Run `modemdeck --help` for usage.");
            2
        }
    }
}

fn load_modems() -> Result<(ModemManagerClient, Vec<crate::model::ModemInfo>), i32> {
    match ModemManagerClient::new() {
        Ok(client) => {
            let modems = client.modems();
            Ok((client, modems))
        }
        Err(e) => {
            eprintln!("Could not connect to ModemManager D-Bus: {e}");
            Err(1)
        }
    }
}

fn cmd_status(json_out: bool, idx: usize) -> i32 {
    let Ok((_, modems)) = load_modems() else {
        return 1;
    };
    let Some(m) = modems.get(idx) else {
        if json_out {
            println!("{}", json!({"modems": 0, "status": "no_modem"}));
        } else {
            println!("No modem found at index {idx}.");
        }
        return 0;
    };
    if json_out {
        println!(
            "{}",
            json!({
                "modem": m.display_name(),
                "state": m.state_label(),
                "operator": m.operator_label(),
                "network": m.network_label(),
                "signal_percent": m.signal_percent,
                "band": m.detected_serving_lte_band().map(|b| format!("B{b}")).or_else(|| m.detected_serving_nr_band().map(|b| format!("n{b}"))),
                "ca_active": m.carrier_aggregation_active(),
            })
        );
    } else {
        println!(
            "{} | {} | {} | {} ({}%)",
            m.display_name(),
            m.state_label(),
            m.operator_label(),
            m.network_label(),
            m.signal_percent
        );
    }
    0
}

fn cmd_list_modems(json_out: bool) -> i32 {
    let Ok((_, modems)) = load_modems() else {
        return 1;
    };
    if json_out {
        let items: Vec<_> = modems
            .iter()
            .enumerate()
            .map(|(i, m)| {
                json!({
                    "index": i,
                    "name": m.display_name(),
                    "state": m.state_label(),
                    "plugin": m.plugin,
                    "primary_port": m.primary_port,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&items).unwrap_or_default()
        );
    } else if modems.is_empty() {
        println!("No modems detected.");
    } else {
        for (i, m) in modems.iter().enumerate() {
            println!(
                "[{i}] {} ({}) - {}",
                m.display_name(),
                m.plugin,
                m.state_label()
            );
        }
    }
    0
}

fn cmd_modem_info(json_out: bool, idx: usize) -> i32 {
    let Ok((_, modems)) = load_modems() else {
        return 1;
    };
    let Some(m) = modems.get(idx) else {
        eprintln!("No modem at index {idx}.");
        return 1;
    };
    let laptop = LaptopInfo::detect();
    let hw_db = HardwareDatabase::global();
    let hw_match = hw_db.lookup(
        &m.manufacturer,
        &m.model,
        m.usb_vid,
        m.usb_pid,
        m.pci_vid,
        m.pci_pid,
    );
    if json_out {
        println!(
            "{}",
            json!({
                "modem": m.display_name(),
                "firmware": m.revision,
                "plugin": m.plugin,
                "kernel_driver": m.kernel_driver,
                "laptop": laptop.display_label(),
                "db_match": hw_match.map(|e| e.commercial_name.clone()),
                "verification_status": hw_match.map(|e| e.verification_status.label()),
            })
        );
    } else {
        println!("Modem: {}", m.display_name());
        println!("Firmware: {}", m.revision);
        println!("Plugin: {} | Driver: {}", m.plugin, m.kernel_driver);
        println!("Host Laptop: {}", laptop.display_label());
        if let Some(e) = hw_match {
            println!(
                "Hardware DB: {} ({} • {})",
                e.commercial_name,
                e.chipset,
                e.verification_status.label()
            );
        }
    }
    0
}

fn cmd_signal(json_out: bool, idx: usize) -> i32 {
    let Ok((_, modems)) = load_modems() else {
        return 1;
    };
    let Some(m) = modems.get(idx) else {
        eprintln!("No modem at index {idx}.");
        return 1;
    };
    let (tech, snap) = m.preferred_snapshot();
    if json_out {
        println!(
            "{}",
            json!({
                "tech": tech,
                "percent": m.signal_percent,
                "rsrp": snap.signal.rsrp,
                "rsrq": snap.signal.rsrq,
                "snr": snap.signal.snr,
                "rssi": snap.signal.rssi,
            })
        );
    } else {
        println!(
            "{tech} ({}%) | RSRP: {:?} | RSRQ: {:?} | SINR: {:?} | RSSI: {:?}",
            m.signal_percent, snap.signal.rsrp, snap.signal.rsrq, snap.signal.snr, snap.signal.rssi
        );
    }
    0
}

fn cmd_bands_list(json_out: bool, idx: usize) -> i32 {
    let Ok((_, modems)) = load_modems() else {
        return 1;
    };
    let Some(m) = modems.get(idx) else {
        eprintln!("No modem at index {idx}.");
        return 1;
    };
    if json_out {
        let supported: Vec<_> = m
            .supported_bands
            .iter()
            .map(|b| json!({"enum": b, "label": band_label(*b)}))
            .collect();
        let current: Vec<_> = m
            .current_bands
            .iter()
            .map(|b| json!({"enum": b, "label": band_label(*b)}))
            .collect();
        println!("{}", json!({"supported": supported, "current": current}));
    } else {
        println!(
            "Current: {}",
            m.current_bands
                .iter()
                .map(|b| band_label(*b))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "Supported: {}",
            m.supported_bands
                .iter()
                .map(|b| band_label(*b))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    0
}

fn cmd_bands_lock(idx: usize, raw: &str) -> i32 {
    let Ok((client, modems)) = load_modems() else {
        return 1;
    };
    let Some(m) = modems.get(idx) else {
        eprintln!("No modem at index {idx}.");
        return 1;
    };
    let bands: Vec<u32> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect();
    if bands.is_empty() {
        eprintln!("Provide comma-separated ModemManager band enum IDs.");
        return 2;
    }
    match client.set_bands_direct(&m.object_path, &bands) {
        Ok(()) => {
            println!("Bands updated.");
            0
        }
        Err(e) => {
            eprintln!("Band lock failed: {e}");
            1
        }
    }
}

fn cmd_bands_unlock(idx: usize) -> i32 {
    let Ok((client, modems)) = load_modems() else {
        return 1;
    };
    let Some(m) = modems.get(idx) else {
        eprintln!("No modem at index {idx}.");
        return 1;
    };
    match client.set_bands_direct(&m.object_path, &[256]) {
        Ok(()) => {
            println!("Bands reset to Auto.");
            0
        }
        Err(e) => {
            eprintln!("Band unlock failed: {e}");
            1
        }
    }
}

fn cmd_network_scan(json_out: bool, idx: usize) -> i32 {
    let Ok((_, modems)) = load_modems() else {
        return 1;
    };
    let Some(m) = modems.get(idx) else {
        eprintln!("No modem at index {idx}.");
        return 1;
    };
    let mm_idx = m.object_path.rsplit('/').next().unwrap_or("0");
    let home_mcc = crate::operators::split_plmn(&m.operator_code)
        .map(|(mcc, _)| mcc)
        .unwrap_or("");
    match crate::scanner::run_scan(mm_idx, home_mcc) {
        Ok(res) => {
            if json_out {
                println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
            } else {
                for net in &res.networks {
                    println!(
                        "{} ({}) | {} | {} | {}",
                        net.operator_name,
                        net.operator_code,
                        net.technology,
                        net.availability.label(),
                        if net.roaming { "Roaming" } else { "Home" }
                    );
                }
            }
            0
        }
        Err(e) => {
            eprintln!("Network scan failed: {e}");
            1
        }
    }
}

fn cmd_operator_info(json_out: bool, idx: usize) -> i32 {
    let Ok((_, modems)) = load_modems() else {
        return 1;
    };
    let db = OperatorDatabase::global();
    if let Some(m) = modems.get(idx) {
        let m_res = db.identify(
            &m.operator_code,
            &m.spn,
            &m.gid1,
            &m.registered_operator_code,
            &m.registered_operator_name,
            m.roaming,
            None,
        );
        if json_out {
            println!(
                "{}",
                json!({
                    "confidence": m_res.confidence.label(),
                    "roaming": m_res.roaming,
                    "sim_operator": m_res.sim_operator,
                    "registered_network": m_res.registered_network,
                    "db_operators": db.operator_count(),
                    "db_countries": db.country_count(),
                })
            );
        } else {
            println!(
                "Confidence: {} | DB: {} operators across {} countries",
                m_res.confidence.label(),
                db.operator_count(),
                db.country_count()
            );
            if let Some(op) = m_res.sim_operator {
                println!(
                    "SIM Operator: {} ({}) [{}-{}]",
                    op.name, op.country, op.mcc, op.mnc
                );
                if op.is_mvno {
                    println!("MVNO Host: {}", op.host_network);
                }
            }
        }
    } else if json_out {
        println!(
            "{}",
            json!({"db_operators": db.operator_count(), "db_countries": db.country_count()})
        );
    } else {
        println!(
            "Global Operator DB: {} operators across {} countries.",
            db.operator_count(),
            db.country_count()
        );
    }
    0
}

fn cmd_apn_list(json_out: bool) -> i32 {
    match apn::list() {
        Ok(list) => {
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&list).unwrap_or_default()
                );
            } else if list.is_empty() {
                println!("No GSM APN profiles configured in NetworkManager.");
            } else {
                for p in list {
                    println!("{} | APN: {} | Active: {}", p.name, p.apn, p.active);
                }
            }
            0
        }
        Err(e) => {
            eprintln!("APN list failed: {e}");
            1
        }
    }
}

fn cmd_doctor(json_out: bool) -> i32 {
    let modems = ModemManagerClient::new()
        .map(|c| c.modems())
        .unwrap_or_default();
    let report = doctor::run_doctor(&modems);
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!("Modem Doctor Overall: {}", report.overall.label());
        for c in &report.checks {
            println!("[{}] {} — {}", c.severity.label(), c.title, c.explanation);
        }
    }
    0
}

fn cmd_history(json_out: bool) -> i32 {
    let records = history_db::query(
        history_db::TimeRange::Last30Days,
        &history_db::HistoryFilter::default(),
    );
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&records).unwrap_or_default()
        );
    } else {
        println!("Recorded {} history samples (last 30 days).", records.len());
    }
    0
}

fn cmd_diagnostics_export() -> i32 {
    let modems = ModemManagerClient::new()
        .map(|c| c.modems())
        .unwrap_or_default();
    match diagnostics::export_privacy_safe(&modems, &Settings::load()) {
        Ok(path) => {
            println!("Exported privacy-safe diagnostics to {}", path.display());
            0
        }
        Err(e) => {
            eprintln!("Export failed: {e}");
            1
        }
    }
}

fn cmd_speedtest(json_out: bool, idx: usize) -> i32 {
    let modems = ModemManagerClient::new()
        .map(|c| c.modems())
        .unwrap_or_default();
    let iface = modems
        .get(idx)
        .and_then(|m| m.net_ports.first().cloned())
        .unwrap_or_default();
    let sig = modems.get(idx).map(|m| m.signal_percent).unwrap_or(0);
    let report = crate::netdiag::run_quick_diagnostics(&iface, sig);
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!("Diagnosis: {}", report.diagnosis.label());
        println!("Evidence: {}", report.evidence_summary);
        for p in &report.probes {
            println!(
                "  {} ({}) -> reachable={} dns={:?}ms tcp={:?}ms {}",
                p.label, p.host, p.reachable, p.dns_ms, p.tcp_ms, p.error
            );
        }
    }
    0
}

fn print_help() {
    println!(
        "ModemDeck CLI by MilMit ({})\n\
         Usage: modemdeck <command> [--json] [--modem <index>]\n\n\
         Commands:\n\
           status              Show active modem status\n\
           list-modems         List detected modems\n\
           modem info          Show modem & hardware DB details\n\
           signal              Show RF signal metrics\n\
           bands list          List supported and current bands\n\
           bands lock <ids>    Lock to comma-separated MM band enum IDs\n\
           bands unlock        Reset bands to Auto\n\
           network scan        Scan visible cellular networks\n\
           operator info       Identify SIM/network operator\n\
           apn list            List NetworkManager GSM APN profiles\n\
           speedtest           Run domestic/international connectivity probes\n\
           doctor              Run Modem Doctor health diagnostics\n\
           history             Show signal/band history count\n\
           diagnostics export  Export privacy-safe diagnostics report\n\
           completions <sh>    Print shell completions (bash|zsh|fish)",
        env!("CARGO_PKG_VERSION")
    );
}

fn print_completions(shell: &str) {
    match shell {
        "zsh" => println!(
            "#compdef modemdeck\n_arguments '1:command:(status list-modems modem signal bands network operator apn speedtest doctor history diagnostics completions)'"
        ),
        "fish" => println!(
            "complete -c modemdeck -f -a 'status list-modems modem signal bands network operator apn speedtest doctor history diagnostics completions'"
        ),
        _ => println!(
            "complete -W 'status list-modems modem signal bands network operator apn speedtest doctor history diagnostics completions' modemdeck"
        ),
    }
}
