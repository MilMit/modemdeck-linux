use crate::core::settings;
use crate::locking::band_lock::{LockMode, LockPolicy, LockVerificationStatus};
use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentLockEntry {
    pub modem_uid: String,
    pub sim_hash: String,
    pub operator_code: String,
    pub mode: LockMode,
    pub policy: LockPolicy,
    pub bands: Vec<u32>,
    pub expires_unix: Option<u64>,
    pub updated_unix: u64,
    pub last_status: LockVerificationStatus,
    pub last_reason: String,
    #[serde(default)]
    pub recent_restore_attempts: Vec<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistentLockStore {
    pub entries: Vec<PersistentLockEntry>,
}

pub fn path() -> PathBuf {
    settings::config_home().join("modemdeck/persistent-locks.json")
}

pub fn load() -> PersistentLockStore {
    fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(store: &PersistentLockStore) -> io::Result<()> {
    let p = path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(store).map_err(|e| io::Error::other(e.to_string()))?;
    fs::write(&tmp, data)?;
    fs::rename(tmp, p)
}

pub fn find_for_modem<'a>(
    store: &'a PersistentLockStore,
    modem: &ModemInfo,
) -> Option<&'a PersistentLockEntry> {
    let uid = modem.stable_id();
    let sim = &modem.sim_identity.stable_hash;
    let op = &modem.operator_code;
    store
        .entries
        .iter()
        .find(|e| {
            e.modem_uid == uid
                && e.sim_hash == *sim
                && (e.operator_code.is_empty() || e.operator_code == *op)
        })
        .or_else(|| {
            store
                .entries
                .iter()
                .find(|e| e.modem_uid == uid && e.sim_hash == *sim)
        })
        .or_else(|| store.entries.iter().find(|e| e.modem_uid == uid))
}

pub fn upsert_for_modem(modem: &ModemInfo, mut entry: PersistentLockEntry) -> io::Result<()> {
    let mut store = load();
    entry.modem_uid = modem.stable_id();
    entry.sim_hash = modem.sim_identity.stable_hash.clone();
    entry.operator_code = modem.operator_code.clone();
    entry.updated_unix = now_unix();

    if let Some(existing) = store
        .entries
        .iter_mut()
        .find(|e| e.modem_uid == entry.modem_uid && e.sim_hash == entry.sim_hash)
    {
        *existing = entry;
    } else {
        store.entries.push(entry);
    }
    save(&store)
}

pub fn clear_for_modem(modem: &ModemInfo) -> io::Result<()> {
    let mut store = load();
    let uid = modem.stable_id();
    store.entries.retain(|e| e.modem_uid != uid);
    save(&store)
}

/// Anti-reconnect-loop guard: allow at most 3 restore attempts within 5 minutes.
pub fn can_attempt_restore(entry: &mut PersistentLockEntry) -> bool {
    let now = now_unix();
    if let Some(exp) = entry.expires_unix
        && now >= exp
    {
        return false;
    }
    entry
        .recent_restore_attempts
        .retain(|t| now.saturating_sub(*t) < 300);
    if entry.recent_restore_attempts.len() >= 3 {
        return false;
    }
    entry.recent_restore_attempts.push(now);
    true
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
