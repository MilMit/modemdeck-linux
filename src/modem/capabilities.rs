use crate::drivers::{DriverCapabilities, DriverKind};
use crate::model::{BandCategory, BandInfo, ModemInfo, classify_mm_band};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct RuntimeModemCapabilities {
    pub has_signal_iface: bool,
    pub has_messaging_iface: bool,
    pub has_voice_iface: bool,
    pub has_ussd_iface: bool,
    pub has_3gpp_iface: bool,
    pub driver_caps: DriverCapabilities,
    pub bands_by_category: BTreeMap<BandCategory, Vec<BandInfo>>,
    pub can_set_bands: bool,
}

impl RuntimeModemCapabilities {
    pub fn detect(
        modem: &ModemInfo,
        has_signal_iface: bool,
        has_messaging_iface: bool,
        has_voice_iface: bool,
        has_ussd_iface: bool,
        has_3gpp_iface: bool,
    ) -> Self {
        let driver = DriverKind::identify(&modem.manufacturer, &modem.model);
        let driver_caps = driver.capabilities(&modem.model);
        let mut bands_by_category: BTreeMap<BandCategory, Vec<BandInfo>> = BTreeMap::new();
        for &band in &modem.supported_bands {
            let info = classify_mm_band(band);
            if info.category != BandCategory::Auto {
                bands_by_category
                    .entry(info.category)
                    .or_default()
                    .push(info);
            }
        }
        let can_set_bands = modem.supported_bands.iter().any(|b| *b != 256 && *b != 0);

        Self {
            has_signal_iface,
            has_messaging_iface,
            has_voice_iface,
            has_ussd_iface,
            has_3gpp_iface,
            driver_caps,
            bands_by_category,
            can_set_bands,
        }
    }
}
