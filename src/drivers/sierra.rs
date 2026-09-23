use crate::model::VendorInfo;

pub fn parse(responses: &[(String, String)]) -> VendorInfo {
    let status = response_for(responses, "AT!GSTATUS?").unwrap_or_default();
    let lte = response_for(responses, "AT!LTEINFO?").unwrap_or_default();
    let nr = response_for(responses, "AT!NRINFO?").unwrap_or_default();
    let mut info = VendorInfo { driver_name: "Sierra Wireless / AirPrime".into(), deep_read_available: true, raw_debug: status.trim().to_string(), raw_ca: if nr.trim().is_empty() { lte.trim().to_string() } else { format!("{}\n{}", lte.trim(), nr.trim()) }, ..VendorInfo::default() };
    for line in status.lines().map(str::trim) {
        let lower = line.to_ascii_lowercase();
        if lower.contains("lte band:") { if let Some(after) = split_value(line, "LTE band:") { info.serving_band = first_number(after); } }
        if lower.contains("lte rx chan:") { if let Some(after) = split_value(line, "LTE Rx chan:") { info.earfcn = first_number(after); } }
        if lower.contains("pci:") && info.pci.is_none() { if let Some(after) = split_value(line, "PCI:") { info.pci = first_number(after); } }
        if lower.contains("lte bw:") && info.bandwidth.is_empty() { if let Some(after) = split_value(line, "LTE bw:") { info.bandwidth = after.trim().to_string(); } }
    }
    let combined = format!("{status}\n{lte}\n{nr}").to_ascii_lowercase();
    info.ca_active = if combined.contains("scell") && !combined.contains("scell band: --") { Some(true) } else if combined.contains("lte ca state") || combined.contains("scell") { Some(false) } else { None };
    info.ca_summary = status.lines().map(str::trim).filter(|line| { let lower = line.to_ascii_lowercase(); lower.contains("ca state") || lower.contains("scell") }).collect::<Vec<_>>().join("\n");
    info
}
fn response_for(responses: &[(String, String)], command: &str) -> Option<String> { responses.iter().find(|(name, _)| name.eq_ignore_ascii_case(command)).map(|(_, value)| value.clone()) }
fn split_value<'a>(line: &'a str, needle: &str) -> Option<&'a str> { let pos = line.to_ascii_lowercase().find(&needle.to_ascii_lowercase())?; Some(&line[pos + needle.len()..]) }
fn first_number(value: &str) -> Option<u32> { let digits: String = value.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect(); digits.parse::<u32>().ok() }
