use crate::model::VendorInfo;

pub fn parse(model: &str, responses: &[(String, String)]) -> VendorInfo {
    let primary = responses.first().map(|(_, value)| value.clone()).unwrap_or_default();
    let ca = responses
        .iter()
        .find(|(name, _)| name.to_ascii_uppercase().contains("CAINFO"))
        .map(|(_, value)| value.clone())
        .unwrap_or_default();

    let mut info = VendorInfo {
        driver_name: "Fibocom".into(),
        deep_read_available: true,
        raw_debug: primary.trim().to_string(),
        raw_ca: ca.trim().to_string(),
        ..VendorInfo::default()
    };

    let lower_model = model.to_ascii_lowercase();
    if lower_model.contains("fm350") || lower_model.contains("fm160") || lower_model.contains("dw5931") {
        for line in primary.lines().map(str::trim) {
            if let Some(code) = carrier_band_code(line) {
                if line.to_ascii_uppercase().contains("PCC") && info.serving_band.is_none() {
                    info.serving_band = fibocom_band_code(code);
                }
            }
        }
    }

    let ca_upper = ca.to_ascii_uppercase();
    info.ca_active = if ca_upper.contains("SCC") {
        Some(true)
    } else if !ca.trim().is_empty() {
        Some(false)
    } else {
        None
    };
    info.ca_summary = ca.lines().map(str::trim).filter(|line| !line.is_empty()).take(12).collect::<Vec<_>>().join("\n");
    info
}

fn carrier_band_code(line: &str) -> Option<u32> {
    let upper = line.to_ascii_uppercase();
    let marker = if let Some(pos) = upper.find("PCC:") {
        pos + 4
    } else if let Some(pos) = upper.find("SCC") {
        upper[pos..].find(':').map(|relative| pos + relative + 1)?
    } else {
        return None;
    };
    let digits: String = line[marker..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}

fn fibocom_band_code(code: u32) -> Option<u32> {
    match code {
        101 => Some(1), 103 => Some(3), 107 => Some(7), 120 => Some(20), 128 => Some(28),
        138 => Some(38), 140 => Some(40), 141 => Some(41), 142 => Some(42), 143 => Some(43),
        _ => None,
    }
}
