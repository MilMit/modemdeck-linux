use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LockMode {
    #[default]
    Automatic,
    TemporaryLock,
    PersistentLock,
    SmartAutomatic,
}

impl LockMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::TemporaryLock => "Temporary Lock",
            Self::PersistentLock => "Persistent Lock",
            Self::SmartAutomatic => "Smart Automatic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LockPolicy {
    Strict,
    #[default]
    Safe,
}

impl LockPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Strict => "Strict Lock",
            Self::Safe => "Safe Lock",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LockVerificationStatus {
    #[default]
    Active,
    Applying,
    WaitingForModem,
    WaitingForRegistration,
    LockedAndVerified,
    ConfiguredUnverified,
    RestorePending,
    RecoveryMode,
    Failed,
    Unsupported,
}

impl LockVerificationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Applying => "Applying",
            Self::WaitingForModem => "Waiting for modem",
            Self::WaitingForRegistration => "Waiting for registration",
            Self::LockedAndVerified => "Locked and verified",
            Self::ConfiguredUnverified => "Configured (serving band unverified)",
            Self::RestorePending => "Restore pending",
            Self::RecoveryMode => "Recovery mode",
            Self::Failed => "Failed",
            Self::Unsupported => "Unsupported",
        }
    }
}

pub fn verify_band_lock(requested: &[u32], modem: &ModemInfo) -> LockVerificationStatus {
    if requested.is_empty() {
        return LockVerificationStatus::Failed;
    }
    let is_auto = requested.len() == 1 && requested[0] == 256;
    let mut cur = modem.current_bands.clone();
    let mut req = requested.to_vec();
    cur.sort_unstable();
    req.sort_unstable();

    if !is_auto && cur != req {
        if modem.state < 6 {
            return LockVerificationStatus::WaitingForModem;
        }
        return LockVerificationStatus::RestorePending;
    }

    if modem.state == 7 {
        return LockVerificationStatus::WaitingForRegistration;
    }
    if modem.state < 6 {
        return LockVerificationStatus::WaitingForModem;
    }

    if is_auto {
        return LockVerificationStatus::Active;
    }

    if let Some(serving) = modem.detected_serving_lte_band()
        && let Some(mm_enum) = crate::model::mm_enum_from_lte_band(serving)
    {
        if requested.contains(&mm_enum) {
            return LockVerificationStatus::LockedAndVerified;
        } else {
            return LockVerificationStatus::ConfiguredUnverified;
        }
    }
    LockVerificationStatus::ConfiguredUnverified
}
