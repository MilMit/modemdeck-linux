use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

use crate::settings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandTransaction {
    pub device_uid: String,
    pub object_path: String,
    pub previous_bands: Vec<u32>,
    pub requested_bands: Vec<u32>,
    pub created_unix: u64,
    #[serde(default)]
    pub guard_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsBackup {
    pub kind: String,
    pub device_uid: String,
    pub object_path: String,
    pub bands: Vec<u32>,
    pub created_unix: u64,
    pub note: String,
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|v| v.as_secs()).unwrap_or(0)
}

pub fn pending_path() -> PathBuf { settings::config_home().join("modemdeck/pending-band-change.json") }
pub fn backups_dir() -> PathBuf { settings::config_home().join("modemdeck/backups") }

pub fn write_pending(transaction: &BandTransaction) -> io::Result<()> {
    let path = pending_path();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let bytes = serde_json::to_vec_pretty(transaction).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, bytes)?;
    fs::rename(temp, path)
}
pub fn read_pending() -> Option<BandTransaction> { fs::read_to_string(pending_path()).ok().and_then(|v| serde_json::from_str(&v).ok()) }
pub fn clear_pending() { let _ = fs::remove_file(pending_path()); }

pub fn spawn_rollback_guard(seconds: u64, token: &str) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .arg("--rollback-guard").arg(seconds.to_string()).arg(token)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn prune_backups(keep: usize) {
    let Ok(entries) = fs::read_dir(backups_dir()) else { return; };
    let mut files: Vec<_> = entries.flatten().filter_map(|entry| {
        let meta = entry.metadata().ok()?;
        if !meta.is_file() { return None; }
        Some((meta.modified().ok()?, entry.path()))
    }).collect();
    files.sort_by_key(|item| item.0);
    let remove_count = files.len().saturating_sub(keep);
    for (_, path) in files.into_iter().take(remove_count) { let _ = fs::remove_file(path); }
}

pub fn backup_band_state(device_uid: &str, object_path: &str, bands: &[u32], note: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(backups_dir())?;
    let stamp = now_unix();
    let backup = SettingsBackup { kind:"band".into(),device_uid:device_uid.into(),object_path:object_path.into(),bands:bands.to_vec(),created_unix:stamp,note:note.into() };
    let path = backups_dir().join(format!("band-{stamp}.json"));
    let bytes = serde_json::to_vec_pretty(&backup).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    fs::write(&path, bytes)?;
    prune_backups(50);
    Ok(path)
}

pub fn backup_cell_intent(device_uid: &str, object_path: &str, bands: &[u32], note: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(backups_dir())?;
    let stamp = now_unix();
    let backup = SettingsBackup { kind:"cell-lock-preview".into(),device_uid:device_uid.into(),object_path:object_path.into(),bands:bands.to_vec(),created_unix:stamp,note:note.into() };
    let path = backups_dir().join(format!("cell-preview-{stamp}.json"));
    let bytes = serde_json::to_vec_pretty(&backup).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    fs::write(&path, bytes)?;
    prune_backups(50);
    Ok(path)
}
