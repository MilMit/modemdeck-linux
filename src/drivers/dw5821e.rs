use crate::model::{CarrierComponent, VendorInfo};

pub fn parse(debug: &str, ca: &str) -> VendorInfo {
    let mut info = VendorInfo {
        driver_name: "Dell/Foxconn DW5821e (T77W968)".into(),
        deep_read_available: true,
        raw_debug: debug.trim().to_string(),
        raw_ca: ca.trim().to_string(),
        ..VendorInfo::default()
    };

    for raw_line in debug.lines() {
        let line = raw_line.trim();
        if let Some(value) = line.strip_prefix("BAND:") {
            if info.serving_band.is_none() {
                info.serving_band = value.trim().parse::<u32>().ok();
            }
        } else if let Some(value) = line.strip_prefix("EARFCN(DL/UL):") {
            if info.earfcn.is_none() {
                info.earfcn = value
                    .trim()
                    .split('/')
                    .next()
                    .and_then(|part| part.trim().parse::<u32>().ok());
            }
        } else if let Some(value) = line.strip_prefix("BW:") {
            if info.bandwidth.is_empty() {
                info.bandwidth = value.trim().to_string();
            }
        } else if line.starts_with("eNB ID(PCI):") {
            if let Some(open) = line.rfind('(') {
                if let Some(close) = line[open + 1..].find(')') {
                    info.pci = line[open + 1..open + 1 + close]
                        .trim()
                        .parse::<u32>()
                        .ok();
                }
            }
        }
    }

    let mut ca_lines = Vec::new();
    let mut has_pcc = false;
    let mut has_scc = false;
    for raw_line in ca.lines() {
        let line = raw_line.trim();
        if line.contains("PCC") && line.contains("Band") {
            has_pcc = true;
            ca_lines.push(line.to_string());
            if let Some(component) = parse_ca_component(line) {
                info.ca_components.push(component);
            }
        } else if line.contains("SCC") && line.contains("Band") {
            has_scc = true;
            ca_lines.push(line.to_string());
            if let Some(component) = parse_ca_component(line) {
                info.ca_components.push(component);
            }
        }
    }

    info.ca_active = if has_scc || info.ca_components.len() > 1 {
        Some(true)
    } else if has_pcc || !info.ca_components.is_empty() || ca.trim().is_empty() || ca.trim().eq_ignore_ascii_case("OK") {
        Some(false)
    } else {
        None
    };
    info.ca_summary = ca_lines.join("\n");
    info
}

pub fn parse_responses(responses: &[(String, String)]) -> VendorInfo {
    let debug = response_for(responses, "AT^DEBUG?").unwrap_or_default();
    let ca = response_for(responses, "AT^CA_INFO?").unwrap_or_default();
    parse(&debug, &ca)
}

pub fn cell_lock_preview(earfcn: u32, pci: u32) -> Result<String, String> {
    if earfcn > u16::MAX as u32 {
        return Err("DW5821e EFS cell lock stores EARFCN in 16 bits; this EARFCN cannot be represented safely.".into());
    }
    if pci > u16::MAX as u32 {
        return Err("PCI cannot be represented safely by the DW5821e lock format.".into());
    }

    let earfcn_bytes = (earfcn as u16).to_le_bytes();
    let pci_bytes = (pci as u16).to_le_bytes();
    Ok(format!(
        "Preview only — not executed\n\nLock EARFCN + PCI:\nat^efs=\"/nv/item_files/modem/lte/rrc/csp/pci_lock\",4,\"{:02X},{:02X},{:02X},{:02X}\"\n\nUnlock:\nat^efs=\"/nv/item_files/modem/lte/rrc/csp/pci_lock\",0\n\nA modem restart is normally required. This EFS method can disable carrier aggregation on some networks, so ModemDeck does not execute it automatically in this alpha.",
        earfcn_bytes[0], earfcn_bytes[1], pci_bytes[0], pci_bytes[1]
    ))
}

fn parse_ca_component(line: &str) -> Option<CarrierComponent> {
    let role = line
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(':')
        .to_string();

    let band = line
        .split("LTE_B")
        .nth(1)
        .and_then(|tail| tail.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|value| value.parse::<u32>().ok());

    let bandwidth_mhz = line
        .split("Band_width is")
        .nth(1)
        .and_then(|tail| tail.trim().split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok());

    if band.is_none() && bandwidth_mhz.is_none() {
        return None;
    }

    Some(CarrierComponent {
        role,
        band,
        bandwidth_mhz,
    })
}

fn response_for(responses: &[(String, String)], command: &str) -> Option<String> {
    responses
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(command))
        .map(|(_, value)| value.clone())
}
