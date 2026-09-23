use crate::{model::{band_label, ModemInfo}, settings::Settings};
use std::{fs, io, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

pub fn export_privacy_safe(modems: &[ModemInfo], settings: &Settings) -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let downloads = home.join("Downloads");
    fs::create_dir_all(&downloads)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let path = downloads.join(format!("modemdeck-diagnostics-{stamp}.txt"));

    let mut out = String::new();
    out.push_str("ModemDeck diagnostics (privacy-safe)\n");
    out.push_str("======================================\n");
    out.push_str("IMEI/equipment identifiers are intentionally omitted.\n\n");
    out.push_str(&format!("ModemDeck version: {}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("Signal refresh: {} seconds\n", settings.normalized_refresh_seconds()));
    out.push_str(&format!("Notifications: {}\n", settings.notifications));
    out.push_str(&format!("Background monitor preference: {}\n\n", settings.background_monitor));

    if modems.is_empty() {
        out.push_str("No modem detected.\n");
    }

    for (index, modem) in modems.iter().enumerate() {
        out.push_str(&format!("Modem {}\n", index + 1));
        out.push_str(&format!("  Name: {}\n", modem.display_name()));
        out.push_str(&format!("  Firmware: {}\n", modem.revision));
        out.push_str(&format!("  Plugin: {}\n", modem.plugin));
        out.push_str(&format!("  State: {}\n", modem.state_label()));
        out.push_str(&format!("  Operator: {}\n", modem.operator_label()));
        out.push_str(&format!("  Network: {}\n", modem.network_label()));
        out.push_str(&format!("  Signal: {}%\n", modem.signal_percent));
        let (_, signal) = modem.preferred_signal();
        out.push_str(&format!("  RSRP: {:?}\n", signal.rsrp));
        out.push_str(&format!("  RSRQ: {:?}\n", signal.rsrq));
        out.push_str(&format!("  SNR: {:?}\n", signal.snr));
        out.push_str(&format!("  Vendor driver: {}\n", modem.vendor.driver_name));
        out.push_str(&format!("  CA detected: {:?}\n", modem.vendor.ca_active));
        out.push_str(&format!("  Serving band: {:?}\n", modem.vendor.serving_band));
        out.push_str(&format!("  EARFCN: {:?}\n", modem.vendor.earfcn));
        out.push_str(&format!("  PCI: {:?}\n", modem.vendor.pci));
        let bands = modem.current_bands.iter().map(|value| band_label(*value)).collect::<Vec<_>>().join(", ");
        out.push_str(&format!("  Configured bands: {}\n", bands));
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
