use crate::model::VendorInfo;

pub fn parse(responses: &[(String, String)]) -> VendorInfo {
    let raw = responses.first().map(|(_, value)| value.clone()).unwrap_or_default();
    let upper = raw.to_ascii_uppercase();
    VendorInfo { driver_name: "Telit / Cinterion".into(), deep_read_available: true, serving_band: find_after_label_number(&upper, "BAND:"), earfcn: find_after_label_number(&upper, "EARFCN:"), pci: find_after_label_number(&upper, "PCI:"), raw_debug: raw, ..VendorInfo::default() }
}
fn find_after_label_number(text: &str, label: &str) -> Option<u32> { let pos = text.find(label)? + label.len(); let digits: String = text[pos..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect(); digits.parse::<u32>().ok() }
