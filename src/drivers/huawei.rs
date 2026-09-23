use crate::model::VendorInfo;

pub fn parse(responses: &[(String, String)]) -> VendorInfo {
    let raw = responses.iter().map(|(command, value)| format!("{command}\n{value}")).collect::<Vec<_>>().join("\n\n");
    VendorInfo {
        driver_name: "Huawei".into(),
        deep_read_available: true,
        raw_debug: raw,
        ..VendorInfo::default()
    }
}
