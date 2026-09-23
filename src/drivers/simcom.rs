use crate::model::VendorInfo;

pub fn parse(responses: &[(String, String)]) -> VendorInfo {
    let raw = responses.first().map(|(_, value)| value.clone()).unwrap_or_default();
    let mut info = VendorInfo { driver_name: "SIMCom".into(), deep_read_available: true, raw_debug: raw.clone(), ..VendorInfo::default() };
    let upper = raw.to_ascii_uppercase();
    if let Some(pos) = upper.find("BAND") {
        let digits: String = upper[pos + 4..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
        info.serving_band = digits.parse::<u32>().ok();
    }
    info
}
