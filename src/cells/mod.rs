//! Cell & Tower Analyzer (Serving/Neighbor Cells, eNodeB/gNodeB ID, Frequency, History).
//! Never sends cell/location data to external services without explicit user consent.

use crate::model::{ModemInfo, ServingCellInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerLocationEstimate {
    pub latitude: f64,
    pub longitude: f64,
    pub verified: bool, // false = estimated from cell DB, true = verified
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellSummary {
    pub technology: String,
    pub cell_id: String,
    pub enb_or_gnb_id: Option<u64>,
    pub pci: String,
    pub tac: String,
    pub earfcn: Option<u32>,
    pub nrarfcn: Option<u32>,
    pub band_label: String,
    pub bandwidth_mhz: Option<u32>,
    pub role: String,
}

impl CellSummary {
    pub fn from_serving(cell: &ServingCellInfo) -> Self {
        let cid_num = u64::from_str_radix(cell.ci.trim().trim_start_matches("0x"), 16)
            .ok()
            .or_else(|| cell.ci.trim().parse::<u64>().ok());
        // Standard E-UTRAN Cell ID is 28 bits: upper 20 bits = eNodeB ID
        let enb_or_gnb_id = if cell.cell_type == 5 {
            cid_num.map(|id| id >> 8)
        } else if cell.cell_type == 6 {
            cid_num.map(|id| id >> 14)
        } else {
            None
        };
        Self {
            technology: cell.technology_label().into(),
            cell_id: cell.ci.clone(),
            enb_or_gnb_id,
            pci: cell.pci_label(),
            tac: cell.tac.clone(),
            earfcn: cell.earfcn,
            nrarfcn: cell.nrarfcn,
            band_label: cell.band_label(),
            bandwidth_mhz: cell.bandwidth,
            role: cell.role_label().into(),
        }
    }
}

pub fn summarize_cells(modem: &ModemInfo) -> Vec<CellSummary> {
    modem
        .serving_cells
        .iter()
        .map(CellSummary::from_serving)
        .collect()
}
