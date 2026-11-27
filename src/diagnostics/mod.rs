use crate::{
    analyzer::signal_score::{RadioTech, evaluate_signal},
    core::settings::Settings,
    drivers::DriverKind,
    locking::persistent,
    model::{ModemInfo, band_label},
};
use std::{
    fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn export_privacy_safe(modems: &[ModemInfo], settings: &Settings) -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let downloads = home.join("Downloads");
    fs::create_dir_all(&downloads)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let path = downloads.join(format!("modemdeck-diagnostics-{stamp}.txt"));

    let lock_store = persistent::load();
    let mut out = String::new();
    out.push_str("ModemDeck diagnostics (privacy-safe)\n");
    out.push_str("======================================\n");
    out.push_str(
        "IMEI, IMSI, ICCID, and authentication credentials are intentionally omitted.\n\n",
    );
    out.push_str(&format!(
        "ModemDeck version: {}\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(&format!(
        "Signal refresh: {} seconds\n",
        settings.normalized_refresh_seconds()
    ));
    out.push_str(&format!("Notifications: {}\n", settings.notifications));
    out.push_str(&format!(
        "Background monitor: {}\n",
        settings.background_monitor
    ));
    out.push_str(&format!("Theme: {}\n\n", settings.theme));

    if modems.is_empty() {
        out.push_str("No modem detected.\n");
    }

    for (index, modem) in modems.iter().enumerate() {
        let driver = DriverKind::identify(&modem.manufacturer, &modem.model);
        let (_, snap) = modem.preferred_snapshot();
        let tech = if modem.is_5g_active() {
            RadioTech::Nr5g
        } else {
            RadioTech::Lte
        };
        let score = evaluate_signal(tech, snap, &[]);

        out.push_str(&format!("Modem {}\n", index + 1));
        out.push_str(&format!("  Name: {}\n", modem.display_name()));
        out.push_str(&format!("  Firmware: {}\n", modem.revision));
        out.push_str(&format!("  Plugin: {}\n", modem.plugin));
        out.push_str(&format!("  State: {}\n", modem.state_label()));
        out.push_str(&format!("  Operator: {}\n", modem.operator_label()));
        out.push_str(&format!("  SIM Hash: {}\n", modem.sim_identity.stable_hash));
        out.push_str(&format!("  Network: {}\n", modem.network_label()));
        out.push_str(&format!(
            "  Signal: {}% ({})\n",
            modem.signal_percent,
            snap.freshness.label()
        ));
        out.push_str(&format!(
            "  Radio Quality: {} ({}/100, Confidence: {})\n",
            score.grade.label(),
            score
                .score
                .map(|s| s.to_string())
                .unwrap_or_else(|| "—".into()),
            score.confidence.label()
        ));
        out.push_str(&format!("  RSRP: {:?}\n", snap.signal.rsrp));
        out.push_str(&format!("  RSRQ: {:?}\n", snap.signal.rsrq));
        out.push_str(&format!("  SNR: {:?}\n", snap.signal.snr));
        out.push_str(&format!("  RSSI: {:?}\n", snap.signal.rssi));
        out.push_str(&format!("  Vendor driver: {}\n", driver.label()));
        out.push_str(&format!("  CA detected: {:?}\n", modem.vendor.ca_active));
        out.push_str(&format!(
            "  Serving band: {:?}\n",
            modem.detected_serving_lte_band()
        ));
        out.push_str(&format!("  EARFCN: {:?}\n", modem.vendor.earfcn));
        out.push_str(&format!("  PCI: {:?}\n", modem.vendor.pci));
        let bands = modem
            .current_bands
            .iter()
            .map(|value| band_label(*value))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("  Configured bands: {}\n", bands));
        if let Some(lock) = persistent::find_for_modem(&lock_store, modem) {
            out.push_str(&format!(
                "  Lock Mode: {} ({}) • Status: {}\n",
                lock.mode.label(),
                lock.policy.label(),
                lock.last_status.label()
            ));
        }
        if !modem.cell_info_error.is_empty() {
            out.push_str(&format!("  Cell-info error: {}\n", modem.cell_info_error));
        }
        if !modem.vendor.error.is_empty() {
            out.push_str(&format!("  Vendor-read note: {}\n", modem.vendor.error));
        }
        out.push('\n');
    }

    fs::write(&path, out)?;
    Ok(path)
}
