use crate::drivers::DriverKind;
use crate::model::{CarrierComponent, ModemInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaAnalysis {
    pub active: bool,
    pub mode_label: String, // e.g. "1CC (No CA)", "2CA", "3CA", "4CA", "5CA", "LTE + NR EN-DC"
    pub pcc: Option<CarrierComponent>,
    pub sccs: Vec<CarrierComponent>,
    pub total_verified_bandwidth_mhz: Option<f64>,
    pub documented_combos: Vec<String>,
    pub note: String,
}

pub fn analyze_ca(modem: &ModemInfo) -> CaAnalysis {
    let driver = DriverKind::identify(&modem.manufacturer, &modem.model);
    let documented_combos: Vec<String> = driver
        .documented_ca_combos()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut components = modem.vendor.ca_components.clone();
    if components.is_empty() && !modem.serving_cells.is_empty() {
        for cell in &modem.serving_cells {
            let role = match cell.serving_cell_type {
                Some(1) => "PCC",
                Some(2) => "SCC1",
                Some(3) => "PSCell",
                Some(4) => "SCG SCC",
                _ => "Serving",
            };
            components.push(CarrierComponent {
                role: role.into(),
                band: cell.band,
                earfcn: cell.earfcn.or(cell.nrarfcn),
                pci: cell.physical_ci.trim().parse::<u32>().ok(),
                bandwidth_mhz: cell.bandwidth.map(|b| b as f64),
                mimo: None,
                modulation: None,
                active: true,
            });
        }
    }

    let pcc = components
        .iter()
        .find(|c| c.role.eq_ignore_ascii_case("PCC") || c.role.eq_ignore_ascii_case("Serving"))
        .cloned()
        .or_else(|| components.first().cloned());

    let sccs: Vec<CarrierComponent> = components
        .iter()
        .filter(|c| {
            let r = c.role.to_ascii_uppercase();
            r.starts_with("SCC") || r.contains("PSCELL") || r.contains("SCG")
        })
        .cloned()
        .collect();

    let is_endc = modem.is_5g_active() && modem.lte_signal.has_any();
    let active = modem.carrier_aggregation_active() || !sccs.is_empty() || is_endc;
    let total_cc = if active { (1 + sccs.len()).max(2) } else { 1 };

    let mode_label = if is_endc {
        "LTE + NR EN-DC".into()
    } else if active {
        format!("{total_cc}CA")
    } else {
        "Single Carrier (CA Inactive)".into()
    };

    let total_verified_bandwidth_mhz = if !components.is_empty() {
        let sum: f64 = components.iter().filter_map(|c| c.bandwidth_mhz).sum();
        if sum > 0.0 { Some(sum) } else { None }
    } else {
        None
    };

    let note = if active {
        "Carrier Aggregation is verified active on the radio link.".into()
    } else {
        "Selecting multiple allowed bands does not guarantee Carrier Aggregation; networks often activate SCCs only under active data load.".into()
    };

    CaAnalysis {
        active,
        mode_label,
        pcc,
        sccs,
        total_verified_bandwidth_mhz,
        documented_combos,
        note,
    }
}
