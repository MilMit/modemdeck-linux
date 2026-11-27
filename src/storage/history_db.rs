use crate::core::settings;
use crate::model::ModemInfo;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_HISTORY_SAMPLES: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeRange {
    LastMinute,
    Last5Minutes,
    Last15Minutes,
    LastHour,
    Last24Hours,
    Last7Days,
    Last30Days,
    Custom(u64, u64),
}

impl TimeRange {
    pub fn label(self) -> &'static str {
        match self {
            Self::LastMinute => "Last minute",
            Self::Last5Minutes => "Last 5 minutes",
            Self::Last15Minutes => "Last 15 minutes",
            Self::LastHour => "Last hour",
            Self::Last24Hours => "Last 24 hours",
            Self::Last7Days => "Last 7 days",
            Self::Last30Days => "Last 30 days",
            Self::Custom(_, _) => "Custom range",
        }
    }

    pub fn bounds(self, now_unix: u64) -> (u64, u64) {
        match self {
            Self::LastMinute => (now_unix.saturating_sub(60), now_unix),
            Self::Last5Minutes => (now_unix.saturating_sub(300), now_unix),
            Self::Last15Minutes => (now_unix.saturating_sub(900), now_unix),
            Self::LastHour => (now_unix.saturating_sub(3600), now_unix),
            Self::Last24Hours => (now_unix.saturating_sub(86_400), now_unix),
            Self::Last7Days => (now_unix.saturating_sub(7 * 86_400), now_unix),
            Self::Last30Days => (now_unix.saturating_sub(30 * 86_400), now_unix),
            Self::Custom(s, e) => (s, e),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryRecord {
    pub timestamp: u64,
    pub modem_uid: String,
    pub sim_hash: String,
    pub operator: String,
    pub band: String,
    pub signal_percent: u32,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub rssi: Option<f64>,
    pub snr: Option<f64>,
    pub download_mbps: Option<f64>,
    pub upload_mbps: Option<f64>,
    pub ping_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss_percent: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub modem_uid: Option<String>,
    pub sim_hash: Option<String>,
    pub operator: Option<String>,
    pub band: Option<String>,
}

pub fn history_path() -> PathBuf {
    settings::data_home().join("modemdeck/history.jsonl")
}

pub fn record_modems(modems: &[ModemInfo]) {
    let now = now_unix();
    for modem in modems {
        let (_, sig) = modem.preferred_signal();
        let band = modem
            .detected_serving_lte_band()
            .map(|b| format!("B{b}"))
            .or_else(|| modem.detected_serving_nr_band().map(|b| format!("n{b}")))
            .unwrap_or_default();
        let rec = HistoryRecord {
            timestamp: now,
            modem_uid: modem.stable_id(),
            sim_hash: modem.sim_identity.stable_hash.clone(),
            operator: modem.operator_label(),
            band,
            signal_percent: modem.signal_percent,
            rsrp: sig.rsrp,
            rsrq: sig.rsrq,
            rssi: sig.rssi,
            snr: sig.snr,
            ..HistoryRecord::default()
        };
        let _ = append_record(&rec);
    }
}

pub fn append_record(record: &HistoryRecord) -> io::Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    let line = serde_json::to_string(record).map_err(|e| io::Error::other(e.to_string()))?;
    writeln!(file, "{line}")?;
    Ok(())
}

pub fn query(range: TimeRange, filter: &HistoryFilter) -> Vec<HistoryRecord> {
    let path = history_path();
    let Ok(file) = fs::File::open(&path) else {
        return Vec::new();
    };
    let now = now_unix();
    let (start, end) = range.bounds(now);
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let Ok(rec) = serde_json::from_str::<HistoryRecord>(&line) else {
            continue;
        };
        if rec.timestamp < start || rec.timestamp > end {
            continue;
        }
        if let Some(ref m) = filter.modem_uid
            && !m.is_empty()
            && &rec.modem_uid != m
        {
            continue;
        }
        if let Some(ref s) = filter.sim_hash
            && !s.is_empty()
            && &rec.sim_hash != s
        {
            continue;
        }
        if let Some(ref o) = filter.operator
            && !o.is_empty()
            && !rec.operator.eq_ignore_ascii_case(o)
        {
            continue;
        }
        if let Some(ref b) = filter.band
            && !b.is_empty()
            && !rec.band.eq_ignore_ascii_case(b)
        {
            continue;
        }
        out.push(rec);
    }
    out
}

pub fn prune_retention(retention_days: u32) -> io::Result<usize> {
    let path = history_path();
    let Ok(file) = fs::File::open(&path) else {
        return Ok(0);
    };
    let cutoff = now_unix().saturating_sub((retention_days as u64).saturating_mul(86_400));
    let reader = BufReader::new(file);
    let mut kept: Vec<String> = Vec::new();
    let mut removed = 0usize;
    for line in reader.lines().map_while(Result::ok) {
        if let Ok(rec) = serde_json::from_str::<HistoryRecord>(&line) {
            if rec.timestamp >= cutoff {
                kept.push(line);
            } else {
                removed += 1;
            }
        }
    }
    if kept.len() > MAX_HISTORY_SAMPLES {
        let extra = kept.len() - MAX_HISTORY_SAMPLES;
        kept.drain(0..extra);
        removed += extra;
    }
    if removed > 0 {
        let tmp = path.with_extension("jsonl.tmp");
        fs::write(
            &tmp,
            kept.join("\n") + if kept.is_empty() { "" } else { "\n" },
        )?;
        fs::rename(tmp, path)?;
    }
    Ok(removed)
}

pub fn clear() -> io::Result<()> {
    match fs::remove_file(history_path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn export_csv(records: &[HistoryRecord], dest: &std::path::Path) -> io::Result<()> {
    let mut out = String::from(
        "timestamp,modem_uid,sim_hash,operator,band,signal_percent,rsrp,rsrq,rssi,snr,download_mbps,upload_mbps,ping_ms,jitter_ms,packet_loss_percent\n",
    );
    for r in records {
        let fmt_opt = |v: Option<f64>| v.map(|x| format!("{x:.2}")).unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.timestamp,
            escape_csv(&r.modem_uid),
            escape_csv(&r.sim_hash),
            escape_csv(&r.operator),
            escape_csv(&r.band),
            r.signal_percent,
            fmt_opt(r.rsrp),
            fmt_opt(r.rsrq),
            fmt_opt(r.rssi),
            fmt_opt(r.snr),
            fmt_opt(r.download_mbps),
            fmt_opt(r.upload_mbps),
            fmt_opt(r.ping_ms),
            fmt_opt(r.jitter_ms),
            fmt_opt(r.packet_loss_percent),
        ));
    }
    fs::write(dest, out)
}

pub fn export_json(records: &[HistoryRecord], dest: &std::path::Path) -> io::Result<()> {
    let data = serde_json::to_vec_pretty(records).map_err(|e| io::Error::other(e.to_string()))?;
    fs::write(dest, data)
}

fn escape_csv(val: &str) -> String {
    if val.contains(',') || val.contains('"') {
        format!("\"{}\"", val.replace('"', "\"\""))
    } else {
        val.to_string()
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
