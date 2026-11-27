pub mod dw5821e;
pub mod fibocom;
pub mod huawei;
pub mod quectel;
pub mod sierra;
pub mod simcom;
pub mod telit;

use crate::model::VendorInfo;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityLevel {
    Supported,
    ReadOnly,
    Experimental,
    Unsupported,
}

impl CapabilityLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Supported => "Supported",
            Self::ReadOnly => "Read-only",
            Self::Experimental => "Experimental",
            Self::Unsupported => "Unsupported",
        }
    }

    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Verified,
    Unverified,
}

impl VerificationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Verified => "Verified",
            Self::Unverified => "Unverified",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CapabilityEntry {
    pub level: CapabilityLevel,
    pub verification: VerificationStatus,
}

impl CapabilityEntry {
    pub const fn new(level: CapabilityLevel, verification: VerificationStatus) -> Self {
        Self {
            level,
            verification,
        }
    }
    pub const fn unsupported() -> Self {
        Self {
            level: CapabilityLevel::Unsupported,
            verification: VerificationStatus::Unverified,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DriverCapabilities {
    pub vendor_read: bool,
    pub serving_cell: bool,
    pub carrier_aggregation: bool,
    pub neighbour_cells: bool,
    pub lte_cell_lock_preview: bool,
    pub lte_cell_lock_execute: bool,
    pub nr5g_diagnostics: bool,
    pub mbim_supported: bool,
    pub qmi_supported: bool,
    pub at_supported: bool,
    pub mm_dbus_supported: bool,
}

#[derive(Debug, Clone)]
pub struct CapabilityMatrix {
    pub driver_label: String,
    pub protocols: Vec<(&'static str, CapabilityEntry)>,
    pub features: Vec<(&'static str, CapabilityEntry)>,
}

impl DriverKind {
    pub fn identify(manufacturer: &str, model: &str) -> Self {
        let haystack = format!("{} {}", manufacturer, model).to_ascii_lowercase();

        if haystack.contains("dw5821e") || haystack.contains("t77w968") {
            Self::Dw5821e
        } else if haystack.contains("quectel")
            || matches_any(
                &haystack,
                &[
                    "rm500", "rm502", "rm520", "rm530", "em05", "em06", "em12", "ep06", "ec20",
                    "ec21", "ec25", "eg12", "eg18",
                ],
            )
        {
            Self::Quectel
        } else if haystack.contains("fibocom")
            || matches_any(
                &haystack,
                &["l850", "l860", "fm350", "fm160", "fm150", "dw5931"],
            )
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
            Self::Generic => DriverCapabilities {
                mm_dbus_supported: true,
                ..DriverCapabilities::default()
            },
            Self::Dw5821e => DriverCapabilities {
                vendor_read: true,
                serving_cell: true,
                carrier_aggregation: true,
                neighbour_cells: false,
                lte_cell_lock_preview: true,
                lte_cell_lock_execute: false, // EFS NV write requires modem reboot and can disable CA; keep safe preview unless explicitly verified
                nr5g_diagnostics: false,
                mbim_supported: true,
                qmi_supported: true,
                at_supported: true,
                mm_dbus_supported: true,
            },
            Self::Quectel => DriverCapabilities {
                vendor_read: true,
                serving_cell: true,
                carrier_aggregation: true,
                neighbour_cells: true,
                lte_cell_lock_preview: true,
                lte_cell_lock_execute: false,
                nr5g_diagnostics: matches_any(&lower, &["rm5", "rg5"]),
                mbim_supported: true,
                qmi_supported: true,
                at_supported: true,
                mm_dbus_supported: true,
            },
            Self::Fibocom => DriverCapabilities {
                vendor_read: true,
                serving_cell: true,
                carrier_aggregation: true,
                neighbour_cells: lower.contains("l850") || lower.contains("l860"),
                lte_cell_lock_preview: false,
                lte_cell_lock_execute: false,
                nr5g_diagnostics: matches_any(&lower, &["fm350", "fm160", "fm190", "dw5931"]),
                mbim_supported: true,
                qmi_supported: false,
                at_supported: true,
                mm_dbus_supported: true,
            },
            Self::Sierra => DriverCapabilities {
                vendor_read: true,
                serving_cell: true,
                carrier_aggregation: true,
                neighbour_cells: true,
                lte_cell_lock_preview: false,
                lte_cell_lock_execute: false,
                nr5g_diagnostics: matches_any(&lower, &["em91", "em92"]),
                mbim_supported: true,
                qmi_supported: true,
                at_supported: true,
                mm_dbus_supported: true,
            },
            Self::Huawei => DriverCapabilities {
                vendor_read: true,
                serving_cell: false,
                carrier_aggregation: false,
                neighbour_cells: false,
                lte_cell_lock_preview: false,
                lte_cell_lock_execute: false,
                nr5g_diagnostics: false,
                mbim_supported: true,
                qmi_supported: false,
                at_supported: true,
                mm_dbus_supported: true,
            },
            Self::Telit => DriverCapabilities {
                vendor_read: true,
                serving_cell: true,
                carrier_aggregation: false,
                neighbour_cells: true,
                lte_cell_lock_preview: false,
                lte_cell_lock_execute: false,
                nr5g_diagnostics: false,
                mbim_supported: true,
                qmi_supported: true,
                at_supported: true,
                mm_dbus_supported: true,
            },
            Self::Simcom => DriverCapabilities {
                vendor_read: true,
                serving_cell: true,
                carrier_aggregation: false,
                neighbour_cells: false,
                lte_cell_lock_preview: false,
                lte_cell_lock_execute: false,
                nr5g_diagnostics: matches_any(&lower, &["sim82", "sim83"]),
                mbim_supported: true,
                qmi_supported: true,
                at_supported: true,
                mm_dbus_supported: true,
            },
        }
    }

    pub fn capability_matrix(self, model: &str) -> CapabilityMatrix {
        let caps = self.capabilities(model);
        let ver = if self == Self::Dw5821e {
            VerificationStatus::Verified
        } else {
            VerificationStatus::Unverified
        };
        let entry = |enabled: bool, level: CapabilityLevel, v: VerificationStatus| {
            if enabled {
                CapabilityEntry::new(level, v)
            } else {
                CapabilityEntry::unsupported()
            }
        };
        CapabilityMatrix {
            driver_label: self.label().to_string(),
            protocols: vec![
                (
                    "ModemManager D-Bus",
                    CapabilityEntry::new(CapabilityLevel::Supported, VerificationStatus::Verified),
                ),
                (
                    "MBIM",
                    entry(caps.mbim_supported, CapabilityLevel::Supported, ver),
                ),
                (
                    "QMI",
                    entry(caps.qmi_supported, CapabilityLevel::Supported, ver),
                ),
                (
                    "AT Serial / D-Bus",
                    entry(caps.at_supported, CapabilityLevel::ReadOnly, ver),
                ),
            ],
            features: vec![
                (
                    "Band Lock (SetCurrentBands)",
                    CapabilityEntry::new(CapabilityLevel::Supported, VerificationStatus::Verified),
                ),
                (
                    "Vendor Telemetry",
                    entry(caps.vendor_read, CapabilityLevel::ReadOnly, ver),
                ),
                (
                    "Serving Cell Info",
                    entry(caps.serving_cell, CapabilityLevel::ReadOnly, ver),
                ),
                (
                    "Carrier Aggregation",
                    entry(caps.carrier_aggregation, CapabilityLevel::ReadOnly, ver),
                ),
                (
                    "Neighbour Cells",
                    entry(
                        caps.neighbour_cells,
                        CapabilityLevel::Experimental,
                        VerificationStatus::Unverified,
                    ),
                ),
                (
                    "Cell Lock Preview",
                    entry(caps.lte_cell_lock_preview, CapabilityLevel::ReadOnly, ver),
                ),
                (
                    "Cell Lock Execution",
                    entry(
                        caps.lte_cell_lock_execute,
                        CapabilityLevel::Experimental,
                        VerificationStatus::Unverified,
                    ),
                ),
                (
                    "5G NR Diagnostics",
                    entry(
                        caps.nr5g_diagnostics,
                        CapabilityLevel::Experimental,
                        VerificationStatus::Unverified,
                    ),
                ),
            ],
        }
    }

    pub fn has_vendor_deep_read(self) -> bool {
        self != Self::Generic
    }

    pub fn read_commands(self, model: &str) -> Vec<&'static str> {
        let lower = model.to_ascii_lowercase();
        match self {
            Self::Generic => vec![],
            Self::Dw5821e => vec!["AT^DEBUG?", "AT^CA_INFO?"],
            Self::Quectel => vec!["AT+QENG=\"servingcell\"", "AT+QCAINFO", "AT+QNWINFO"],
            Self::Fibocom => {
                if matches_any(&lower, &["fm350", "fm160", "fm190", "dw5931"]) {
                    vec!["AT+GTCCINFO?", "AT+GTCAINFO?", "AT+GTACT?"]
                } else {
                    vec!["AT+XCELLINFO?", "AT+GTCAINFO?"]
                }
            }
            Self::Sierra => {
                if matches_any(&lower, &["em91", "em92"]) {
                    vec!["AT!GSTATUS?", "AT!NRINFO?", "AT!LTEINFO?"]
                } else {
                    vec!["AT!GSTATUS?", "AT!LTEINFO?"]
                }
            }
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
        if pci > 503 {
            return Err("LTE PCI must be between 0 and 503.".into());
        }
        if earfcn > 262_143 {
            return Err("EARFCN is outside the supported LTE range.".into());
        }
        match self {
            Self::Dw5821e => dw5821e::cell_lock_preview(earfcn, pci),
            Self::Quectel => quectel::cell_lock_preview(earfcn, pci),
            _ => Err(
                "This driver does not expose a verified LTE cell-lock recipe in ModemDeck.".into(),
            ),
        }
    }

    /// Documented CA combinations observed/supported for this modem family.
    /// Returns empty when no verified combo table exists so ModemDeck never fabricates unsupported combos.
    pub fn documented_ca_combos(self) -> &'static [&'static str] {
        match self {
            Self::Dw5821e => &[
                "B1+B3",
                "B3+B7",
                "B1+B7",
                "B1+B3+B7",
                "B3+B20",
                "B7+B20",
                "B1+B3+B20",
                "B38+B38",
                "B40+B40",
                "B41+B41",
                "B42+B42",
            ],
            Self::Quectel => &["B1+B3", "B3+B7", "B1+B3+B7", "B3+B20", "B40+B40"],
            Self::Fibocom => &["B1+B3", "B3+B7", "B1+B3+B7"],
            Self::Sierra => &["B1+B3", "B3+B7", "B1+B3+B7"],
            _ => &[],
        }
    }
}

fn matches_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}
