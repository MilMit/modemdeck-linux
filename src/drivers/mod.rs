pub mod dw5821e;
pub mod fibocom;
pub mod huawei;
pub mod quectel;
pub mod sierra;
pub mod simcom;
pub mod telit;

use crate::model::VendorInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKind {
    Generic,
    Dw5821e,
    Quectel,
    Fibocom,
    Sierra,
    Huawei,
    Telit,
    Simcom,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DriverCapabilities {
    pub vendor_read: bool,
    pub serving_cell: bool,
    pub carrier_aggregation: bool,
    pub neighbour_cells: bool,
    pub lte_cell_lock_preview: bool,
    pub nr5g_diagnostics: bool,
}

impl DriverKind {
    pub fn identify(manufacturer: &str, model: &str) -> Self {
        let haystack = format!("{} {}", manufacturer, model).to_ascii_lowercase();

        if haystack.contains("dw5821e") || haystack.contains("t77w968") {
            Self::Dw5821e
        } else if haystack.contains("quectel")
            || matches_any(&haystack, &["rm500", "rm502", "rm520", "rm530", "em05", "em06", "em12", "ep06", "ec20", "ec21", "ec25", "eg12", "eg18"])
        {
            Self::Quectel
        } else if haystack.contains("fibocom")
            || matches_any(&haystack, &["l850", "l860", "fm350", "fm160", "fm150", "dw5931"])
        {
            Self::Fibocom
        } else if haystack.contains("sierra")
            || haystack.contains("airprime")
            || matches_any(&haystack, &["em74", "mc74", "em75", "em91", "em92"])
        {
            Self::Sierra
        } else if haystack.contains("huawei") || haystack.contains("mobile connect") {
            Self::Huawei
        } else if haystack.contains("telit") || haystack.contains("cinterion") {
            Self::Telit
        } else if haystack.contains("simcom") || haystack.contains("simcom_wwan") {
            Self::Simcom
        } else {
            Self::Generic
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "dw5821e" => Some(Self::Dw5821e),
            "quectel" => Some(Self::Quectel),
            "fibocom" => Some(Self::Fibocom),
            "sierra" => Some(Self::Sierra),
            "huawei" => Some(Self::Huawei),
            "telit" => Some(Self::Telit),
            "simcom" => Some(Self::Simcom),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Dw5821e => "dw5821e",
            Self::Quectel => "quectel",
            Self::Fibocom => "fibocom",
            Self::Sierra => "sierra",
            Self::Huawei => "huawei",
            Self::Telit => "telit",
            Self::Simcom => "simcom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "Generic ModemManager",
            Self::Dw5821e => "Dell/Foxconn DW5821e (T77W968)",
            Self::Quectel => "Quectel",
            Self::Fibocom => "Fibocom",
            Self::Sierra => "Sierra Wireless / AirPrime",
            Self::Huawei => "Huawei",
            Self::Telit => "Telit / Cinterion",
            Self::Simcom => "SIMCom",
        }
    }

    pub fn capabilities(self, model: &str) -> DriverCapabilities {
        let lower = model.to_ascii_lowercase();
        match self {
            Self::Generic => DriverCapabilities::default(),
            Self::Dw5821e => DriverCapabilities { vendor_read: true, serving_cell: true, carrier_aggregation: true, neighbour_cells: false, lte_cell_lock_preview: true, nr5g_diagnostics: false },
            Self::Quectel => DriverCapabilities { vendor_read: true, serving_cell: true, carrier_aggregation: true, neighbour_cells: true, lte_cell_lock_preview: true, nr5g_diagnostics: matches_any(&lower, &["rm5", "rg5"]) },
            Self::Fibocom => DriverCapabilities { vendor_read: true, serving_cell: true, carrier_aggregation: true, neighbour_cells: lower.contains("l850") || lower.contains("l860"), lte_cell_lock_preview: false, nr5g_diagnostics: matches_any(&lower, &["fm350", "fm160", "fm190", "dw5931"]) },
            Self::Sierra => DriverCapabilities { vendor_read: true, serving_cell: true, carrier_aggregation: true, neighbour_cells: true, lte_cell_lock_preview: false, nr5g_diagnostics: matches_any(&lower, &["em91", "em92"]) },
            Self::Huawei => DriverCapabilities { vendor_read: true, serving_cell: false, carrier_aggregation: false, neighbour_cells: false, lte_cell_lock_preview: false, nr5g_diagnostics: false },
            Self::Telit => DriverCapabilities { vendor_read: true, serving_cell: true, carrier_aggregation: false, neighbour_cells: true, lte_cell_lock_preview: false, nr5g_diagnostics: false },
            Self::Simcom => DriverCapabilities { vendor_read: true, serving_cell: true, carrier_aggregation: false, neighbour_cells: false, lte_cell_lock_preview: false, nr5g_diagnostics: matches_any(&lower, &["sim82", "sim83"]) },
        }
    }

    pub fn has_vendor_deep_read(self) -> bool { self != Self::Generic }

    pub fn read_commands(self, model: &str) -> Vec<&'static str> {
        let lower = model.to_ascii_lowercase();
        match self {
            Self::Generic => vec![],
            Self::Dw5821e => vec!["AT^DEBUG?", "AT^CA_INFO?"],
            Self::Quectel => vec!["AT+QENG=\"servingcell\"", "AT+QCAINFO", "AT+QNWINFO"],
            Self::Fibocom => if matches_any(&lower, &["fm350", "fm160", "fm190", "dw5931"]) { vec!["AT+GTCCINFO?", "AT+GTCAINFO?", "AT+GTACT?"] } else { vec!["AT+XCELLINFO?", "AT+GTCAINFO?"] },
            Self::Sierra => if matches_any(&lower, &["em91", "em92"]) { vec!["AT!GSTATUS?", "AT!NRINFO?", "AT!LTEINFO?"] } else { vec!["AT!GSTATUS?", "AT!LTEINFO?"] },
            Self::Huawei => vec!["AT^SYSINFOEX", "AT^HCSQ?"],
            Self::Telit => vec!["AT#MONI"],
            Self::Simcom => vec!["AT+CPSI?", "AT+CESQ"],
        }
    }

    pub fn parse(self, model: &str, responses: &[(String, String)]) -> VendorInfo {
        match self {
            Self::Dw5821e => dw5821e::parse_responses(responses),
            Self::Quectel => quectel::parse(responses),
            Self::Fibocom => fibocom::parse(model, responses),
            Self::Sierra => sierra::parse(responses),
            Self::Huawei => huawei::parse(responses),
            Self::Telit => telit::parse(responses),
            Self::Simcom => simcom::parse(responses),
            Self::Generic => VendorInfo::default(),
        }
    }

    pub fn cell_lock_preview(self, earfcn: u32, pci: u32) -> Result<String, String> {
        if pci > 503 { return Err("LTE PCI must be between 0 and 503.".into()); }
        if earfcn > 262_143 { return Err("EARFCN is outside the supported LTE range.".into()); }
        match self {
            Self::Dw5821e => dw5821e::cell_lock_preview(earfcn, pci),
            Self::Quectel => quectel::cell_lock_preview(earfcn, pci),
            _ => Err("This driver does not expose a verified LTE cell-lock recipe in ModemDeck.".into()),
        }
    }
}

fn matches_any(haystack: &str, needles: &[&str]) -> bool { needles.iter().any(|needle| haystack.contains(needle)) }
