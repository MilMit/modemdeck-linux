//! SQLite storage backend (`rusqlite`) for fast, bounded telemetry & test history.

use crate::storage::history_db::HistoryRecord;
use rusqlite::{Connection, params};
use std::{fs, path::PathBuf};

pub fn sqlite_path() -> PathBuf {
    crate::core::settings::data_home().join("modemdeck/modemdeck.db")
}

pub fn open_db() -> Result<Connection, String> {
    let path = sqlite_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(path).map_err(|e| format!("SQLite open failed: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         CREATE TABLE IF NOT EXISTS signal_history (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             timestamp INTEGER NOT NULL,
             modem_uid TEXT NOT NULL,
             sim_hash TEXT NOT NULL,
             operator TEXT NOT NULL,
             band TEXT NOT NULL,
             signal_percent INTEGER NOT NULL,
             rsrp REAL,
             rsrq REAL,
             snr REAL
         );
         CREATE INDEX IF NOT EXISTS idx_signal_ts ON signal_history(timestamp DESC);",
    )
    .map_err(|e| format!("SQLite schema init failed: {e}"))?;
    Ok(conn)
}

pub fn insert_history_record(rec: &HistoryRecord) -> Result<(), String> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO signal_history (timestamp, modem_uid, sim_hash, operator, band, signal_percent, rsrp, rsrq, snr)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            rec.timestamp as i64,
            rec.modem_uid,
            rec.sim_hash,
            rec.operator,
            rec.band,
            rec.signal_percent as i64,
            rec.rsrp,
            rec.rsrq,
            rec.snr,
        ],
    )
    .map_err(|e| format!("SQLite insert failed: {e}"))?;
    Ok(())
}

pub fn query_recent_downsampled(modem_uid: &str, max_points: usize) -> Vec<HistoryRecord> {
    let Ok(conn) = open_db() else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT timestamp, modem_uid, sim_hash, operator, band, signal_percent, rsrp, rsrq, snr
         FROM signal_history WHERE modem_uid = ?1 ORDER BY timestamp DESC LIMIT ?2",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(params![modem_uid, max_points as i64], |row| {
        Ok(HistoryRecord {
            timestamp: row.get::<_, i64>(0).unwrap_or(0) as u64,
            modem_uid: row.get(1).unwrap_or_default(),
            sim_hash: row.get(2).unwrap_or_default(),
            operator: row.get(3).unwrap_or_default(),
            band: row.get(4).unwrap_or_default(),
            signal_percent: row.get::<_, i64>(5).unwrap_or(0) as u32,
            rsrp: row.get(6).ok(),
            rsrq: row.get(7).ok(),
            snr: row.get(8).ok(),
            ..HistoryRecord::default()
        })
    }) else {
        return Vec::new();
    };
    let mut out: Vec<HistoryRecord> = rows.flatten().collect();
    out.reverse();
    out
}
