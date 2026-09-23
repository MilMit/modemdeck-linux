use crate::model::VendorInfo;

pub fn parse(responses: &[(String, String)]) -> VendorInfo {
    let qeng = response_for(responses, "AT+QENG=\"servingcell\"").unwrap_or_default();
    let qca = response_for(responses, "AT+QCAINFO").unwrap_or_default();
    let qnw = response_for(responses, "AT+QNWINFO").unwrap_or_default();

    let mut info = VendorInfo {
        driver_name: "Quectel".into(),
        deep_read_available: true,
        raw_debug: qeng.trim().to_string(),
        raw_ca: qca.trim().to_string(),
        ..VendorInfo::default()
    };

    for line in qeng.lines().map(str::trim) {
        if !line.contains("+QENG:") || !line.contains("\"LTE\"") { continue; }
        let payload = line.split_once(':').map(|(_, v)| v).unwrap_or(line);
        let fields = csv_fields(payload);
        if fields.len() >= 10 && fields.get(2).map(String::as_str) == Some("LTE") {
            info.pci = parse_u32(fields.get(7));
            info.earfcn = parse_u32(fields.get(8));
            info.serving_band = parse_u32(fields.get(9));
            if fields.len() > 11 {
                let ul = fields.get(10).cloned().unwrap_or_default();
                let dl = fields.get(11).cloned().unwrap_or_default();
                if !ul.is_empty() || !dl.is_empty() { info.bandwidth = format!("UL {ul} / DL {dl}"); }
            }
            break;
        }
    }

    if info.serving_band.is_none() {
        if let Some(pos) = qnw.to_ascii_uppercase().find("LTE BAND ") {
            let rest = &qnw[pos + "LTE BAND ".len()..];
            info.serving_band = rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok();
        }
    }

    let ca_lines: Vec<String> = qca.lines().map(str::trim).filter(|line| line.contains("+QCAINFO")).map(ToOwned::to_owned).collect();
    let has_scc = ca_lines.iter().any(|line| line.to_ascii_uppercase().contains("SCC"));
    info.ca_active = if has_scc { Some(true) } else if !qca.trim().is_empty() { Some(false) } else { None };
    info.ca_summary = ca_lines.join("\n");
    info
}

pub fn cell_lock_preview(earfcn: u32, pci: u32) -> Result<String, String> {
    Ok(format!("Preview only — not executed\n\nLTE-only mode required by many Quectel firmwares:\nAT+QCFG=\"NWSCANMODE\",3\n\nLock EARFCN + PCI:\nAT+QNWLOCK=\"common/lte\",2,{earfcn},{pci}\n\nUnlock:\nAT+QNWLOCK=\"common/lte\",0\n\nExact persistence/reboot behavior varies by Quectel family and firmware. ModemDeck will not execute this until the detected model has a verified write profile."))
}

fn response_for(responses: &[(String, String)], command: &str) -> Option<String> {
    responses.iter().find(|(name, _)| name.eq_ignore_ascii_case(command)).map(|(_, value)| value.clone())
}
fn csv_fields(value: &str) -> Vec<String> { value.split(',').map(|item| item.trim().trim_matches('"').to_string()).collect() }
fn parse_u32(value: Option<&String>) -> Option<u32> { value.and_then(|v| v.trim().parse::<u32>().ok()) }
