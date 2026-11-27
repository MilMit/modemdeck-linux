use crate::drivers::DriverKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellLockScope {
    Temporary,
    Persistent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellLockRequest {
    pub earfcn: Option<u32>,
    pub nrarfcn: Option<u32>,
    pub pci: Option<u32>,
    pub scope: CellLockScope,
}

#[derive(Debug, Clone)]
pub struct CellLockCapability {
    pub preview_supported: bool,
    pub execute_supported: bool,
    pub reason: String,
}

pub fn inspect_capability(manufacturer: &str, model: &str) -> CellLockCapability {
    let driver = DriverKind::identify(manufacturer, model);
    let caps = driver.capabilities(model);
    if caps.lte_cell_lock_execute {
        CellLockCapability {
            preview_supported: true,
            execute_supported: true,
            reason: "Verified cell lock execution is supported.".into(),
        }
    } else if caps.lte_cell_lock_preview {
        CellLockCapability {
            preview_supported: true,
            execute_supported: false,
            reason: format!(
                "{} cell lock requires NV/EFS writes and a modem reset; kept in safe preview-only mode to prevent bricking or CA loss.",
                driver.label()
            ),
        }
    } else {
        CellLockCapability {
            preview_supported: false,
            execute_supported: false,
            reason: format!(
                "No verified cell-lock recipe is available for {}.",
                driver.label()
            ),
        }
    }
}
