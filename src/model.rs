use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct RadioSignal {
    pub rssi: Option<f64>,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub snr: Option<f64>,
}

impl RadioSignal {
    pub fn has_any(&self) -> bool {
        self.rssi.is_some() || self.rsrp.is_some() || self.rsrq.is_some() || self.snr.is_some()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServingCellInfo {
    pub cell_type: u32,
    pub serving_cell_type: Option<u32>,
    pub operator_id: String,
    pub tac: String,
    pub ci: String,
    pub physical_ci: String,
    pub earfcn: Option<u32>,
    pub nrarfcn: Option<u32>,
    pub band: Option<u32>,
    pub bandwidth: Option<u32>,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
}

impl ServingCellInfo {
    pub fn technology_label(&self) -> &'static str { match self.cell_type { 1 => "CDMA", 2 => "GSM", 3 => "UMTS", 4 => "TD-SCDMA", 5 => "LTE", 6 => "5G NR", _ => "Unknown" } }
    pub fn role_label(&self) -> &'static str { match self.serving_cell_type { Some(1) => "Primary cell", Some(2) => "Secondary cell", Some(3) => "Primary SCG cell", Some(4) => "Secondary SCG cell", _ => "Serving cell" } }
    pub fn channel_label(&self) -> String { if let Some(earfcn) = self.earfcn { format!("EARFCN {earfcn}") } else if let Some(nrarfcn) = self.nrarfcn { format!("NR-ARFCN {nrarfcn}") } else { "—".into() } }
    pub fn band_label(&self) -> String { match (self.cell_type, self.band) { (5, Some(band)) => { let duplex = if is_lte_tdd_band(band) { "TDD" } else { "FDD" }; format!("LTE B{band} • {duplex}") }, (6, Some(band)) => format!("5G n{band}"), _ => "—".into() } }
    pub fn pci_label(&self) -> String { if self.physical_ci.trim().is_empty() { return "—".into(); } let raw = self.physical_ci.trim(); u32::from_str_radix(raw.trim_start_matches("0x"), 16).map(|decimal| format!("{decimal} (0x{raw})")).unwrap_or_else(|_| raw.to_string()) }
}

#[derive(Debug, Clone, Default)]
pub struct CarrierComponent { pub role: String, pub band: Option<u32>, pub bandwidth_mhz: Option<f64> }
#[derive(Debug, Clone, Default)]
pub struct VendorInfo { pub driver_name: String, pub read_source: String, pub deep_read_available: bool, pub serving_band: Option<u32>, pub earfcn: Option<u32>, pub pci: Option<u32>, pub bandwidth: String, pub ca_active: Option<bool>, pub ca_components: Vec<CarrierComponent>, pub ca_summary: String, pub raw_debug: String, pub raw_ca: String, pub error: String }
#[derive(Debug, Clone, Default)]
pub struct BandChangeState { pub previous_bands: Vec<u32>, pub requested_bands: Vec<u32>, pub pending_confirmation: bool, pub generation: u64, pub message: String }
#[derive(Debug, Clone, Default)]
pub struct ModemInfo {
    pub object_path: String, pub manufacturer: String, pub model: String, pub revision: String, pub equipment_id: String, pub device_uid: String, pub plugin: String, pub primary_port: String, pub at_ports: Vec<String>, pub net_ports: Vec<String>, pub sim_path: String, pub unlock_required: u32, pub primary_sim_slot: u32, pub operator_name: String, pub operator_code: String, pub state: i32, pub current_capabilities: u32, pub access_technologies: u32, pub signal_percent: u32, pub signal_recent: bool, pub supported_bands: Vec<u32>, pub current_bands: Vec<u32>, pub lte_signal: RadioSignal, pub nr5g_signal: RadioSignal, pub serving_cells: Vec<ServingCellInfo>, pub cell_info_error: String, pub cell_info_source: String, pub vendor: VendorInfo, pub band_change: BandChangeState,
}

impl ModemInfo {
    pub fn display_name(&self) -> String { match (self.manufacturer.trim(), self.model.trim()) { ("", "") => "Unknown modem".into(), ("", model) => model.into(), (manufacturer, "") => manufacturer.into(), (manufacturer, model) => format!("{manufacturer} {model}") } }
    pub fn sim_lock_label(&self) -> &'static str { match self.unlock_required { 0 => "Unknown", 1 => "Unlocked", 2 => "SIM PIN required", 3 => "SIM PIN2 required", 4 => "SIM PUK required", 5 => "SIM PUK2 required", 6 => "Network PIN required", 7 => "Network subset PIN required", 8 => "Service provider PIN required", 9 => "Corporate PIN required", _ => "SIM/network lock required" } }
    pub fn state_label(&self) -> &'static str { match self.state { -1 => "Failed", 0 => "Unknown", 1 => "Initializing", 2 => "Locked", 3 => "Disabled", 4 => "Disabling", 5 => "Enabling", 6 => "Enabled", 7 => "Searching", 8 => "Registered", 9 => "Disconnecting", 10 => "Connecting", 11 => "Connected", _ => "Unknown" } }
    pub fn network_label(&self) -> String {
        const NR5G: u32 = 1 << 15; const LTE: u32 = 1 << 14; const HSPA_PLUS: u32 = 1 << 9; const HSPA: u32 = 1 << 8; const UMTS: u32 = 1 << 5; const EDGE: u32 = 1 << 4; const GPRS: u32 = 1 << 3; const GSM: u32 = 1 << 1;
        if self.access_technologies & NR5G != 0 { "5G".into() } else if self.access_technologies & LTE != 0 { if self.carrier_aggregation_active() { "4G+".into() } else { "4G".into() } } else if self.access_technologies & (HSPA_PLUS | HSPA) != 0 { "3G+".into() } else if self.access_technologies & UMTS != 0 { "3G".into() } else if self.access_technologies & EDGE != 0 { "2G E".into() } else if self.access_technologies & (GPRS | GSM) != 0 { "2G".into() } else { "—".into() }
    }
    pub fn carrier_aggregation_active(&self) -> bool { self.vendor.ca_active == Some(true) || self.serving_cells.iter().any(|cell| matches!(cell.serving_cell_type, Some(2) | Some(4))) }
    pub fn carrier_aggregation_count(&self) -> usize { if !self.vendor.ca_components.is_empty() { self.vendor.ca_components.len() } else if self.carrier_aggregation_active() { 2 } else { 1 } }
    pub fn ca_layout_label(&self) -> String { if self.vendor.ca_components.is_empty() { return if self.carrier_aggregation_active() { "CA active".into() } else { "—".into() }; } self.vendor.ca_components.iter().filter_map(|component| component.band.map(|band| format!("B{band}"))).collect::<Vec<_>>().join(" + ") }
    pub fn ca_component_bandwidth_mhz(&self) -> Option<f64> { if self.vendor.ca_components.is_empty() { return None; } let values = self.vendor.ca_components.iter().map(|component| component.bandwidth_mhz).collect::<Option<Vec<_>>>()?; Some(values.into_iter().sum()) }
    pub fn capabilities_label(&self) -> String {
        const POTS: u32 = 1 << 0; const CDMA_EVDO: u32 = 1 << 1; const GSM_UMTS: u32 = 1 << 2; const LTE: u32 = 1 << 3; const IRIDIUM: u32 = 1 << 5; const NR5G: u32 = 1 << 6; const TDS: u32 = 1 << 7;
        let mut parts = Vec::new(); if self.current_capabilities & GSM_UMTS != 0 { parts.push("2G/3G"); } if self.current_capabilities & LTE != 0 { parts.push("4G LTE"); } if self.current_capabilities & NR5G != 0 { parts.push("5G NR"); } if self.current_capabilities & CDMA_EVDO != 0 { parts.push("CDMA/EVDO"); } if self.current_capabilities & TDS != 0 { parts.push("TD-SCDMA"); } if self.current_capabilities & IRIDIUM != 0 { parts.push("Iridium"); } if self.current_capabilities & POTS != 0 { parts.push("POTS"); } if parts.is_empty() { "Not reported".into() } else { parts.join(" • ") }
    }
    pub fn operator_label(&self) -> String { if !self.operator_name.trim().is_empty() { self.operator_name.clone() } else if !self.operator_code.trim().is_empty() { self.operator_code.clone() } else { "No operator".into() } }
    pub fn preferred_signal(&self) -> (&'static str, &RadioSignal) { if self.nr5g_signal.has_any() { ("5G NR", &self.nr5g_signal) } else { ("LTE", &self.lte_signal) } }
}

pub fn lte_band_from_earfcn(earfcn: u32) -> Option<u32> {
    let ranges: &[(u32, u32, u32)] = &[(0,599,1),(600,1199,2),(1200,1949,3),(1950,2399,4),(2400,2649,5),(2650,2749,6),(2750,3449,7),(3450,3799,8),(3800,4149,9),(4150,4749,10),(4750,4949,11),(5010,5179,12),(5180,5279,13),(5280,5379,14),(5730,5849,17),(5850,5999,18),(6000,6149,19),(6150,6449,20),(6450,6599,21),(6600,7399,22),(7500,7699,23),(7700,8039,24),(8040,8689,25),(8690,9039,26),(9040,9209,27),(9210,9659,28),(9660,9769,29),(9770,9869,30),(9870,9919,31),(9920,10359,32),(36000,36199,33),(36200,36349,34),(36350,36949,35),(36950,37549,36),(37550,37749,37),(37750,38249,38),(38250,38649,39),(38650,39649,40),(39650,41589,41),(41590,43589,42),(43590,45589,43),(45590,46589,44),(46590,46789,45),(46790,54539,46),(54540,55239,47),(55240,56739,48),(65536,66435,65),(66436,67335,66),(67336,67535,67),(67536,67835,68),(67836,68335,69),(68336,68585,70),(68586,68935,71),(68936,68985,72),(68986,69035,73),(69036,69465,74),(69466,70315,75),(70316,70365,76),(70366,70545,85),(70546,70595,87),(70596,70645,88)];
    ranges.iter().find(|(start,end,_)| earfcn >= *start && earfcn <= *end).map(|(_,_,band)| *band)
}
pub fn is_lte_tdd_band(band: u32) -> bool { matches!(band, 33..=53) }
pub fn band_label(value: u32) -> String { match value { 0=>"Unknown".into(),1=>"GSM 900".into(),2=>"GSM 1800".into(),3=>"GSM 1900".into(),4=>"GSM 850".into(),5=>"3G B1".into(),6=>"3G B3".into(),7=>"3G B4".into(),8=>"3G B6".into(),9=>"3G B5".into(),10=>"3G B8".into(),11=>"3G B9".into(),12=>"3G B2".into(),13=>"3G B7".into(),31..=44|47..=101=>format!("LTE B{}", value-30),115=>"LTE B85".into(),210..=214|219..=222|225..=226|232=>format!("3G B{}",value-200),256=>"Auto / any band".into(),301..=395=>format!("5G n{}",value-300),557..=561=>format!("5G n{}",value-300),1031..=>format!("LTE B{}",value-1030),_=>format!("Band enum #{value}") } }
pub fn lte_band_number_from_mm_enum(value: u32) -> Option<u32> { match value { 31..=44|47..=101=>Some(value-30),115=>Some(85),1031..=1199=>Some(value-1030),_=>None } }
pub fn is_lte_mm_band(value: u32) -> bool { lte_band_number_from_mm_enum(value).is_some() }
pub fn is_lte_tdd_mm_band(value: u32) -> bool { lte_band_number_from_mm_enum(value).map(is_lte_tdd_band).unwrap_or(false) }
