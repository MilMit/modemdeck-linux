use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::{Mutex, OnceLock}, time::{Duration, Instant}};

use crate::settings;

#[derive(Debug, Clone, Default)]
pub struct TrafficSample {
    pub interface: String,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub today_rx: u64,
    pub today_tx: u64,
    pub month_rx: u64,
    pub month_tx: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Counter { rx: u64, tx: u64 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UsageDb {
    last_interface: String,
    last_rx: u64,
    last_tx: u64,
    daily: BTreeMap<String, Counter>,
    monthly: BTreeMap<String, Counter>,
}

struct Runtime {
    db: UsageDb,
    last_sample: Option<Instant>,
    last_save: Instant,
}

static STATE: OnceLock<Mutex<Runtime>> = OnceLock::new();

fn path() -> PathBuf { settings::config_home().join("modemdeck/data-usage.json") }
fn load_db() -> UsageDb { fs::read_to_string(path()).ok().and_then(|v|serde_json::from_str(&v).ok()).unwrap_or_default() }
fn state() -> &'static Mutex<Runtime> { STATE.get_or_init(||Mutex::new(Runtime{db:load_db(),last_sample:None,last_save:Instant::now()})) }
fn save(db:&UsageDb){let path=path();if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}if let Ok(data)=serde_json::to_vec_pretty(db){let tmp=path.with_extension("json.tmp");if fs::write(&tmp,data).is_ok(){let _=fs::rename(tmp,path);}}}

fn keys()->(String,String){
    let now=glib::DateTime::now_local();
    if let Ok(now)=now{
        let day=now.format("%Y-%m-%d").map(|s|s.to_string()).unwrap_or_else(|_|"unknown-day".into());
        let month=now.format("%Y-%m").map(|s|s.to_string()).unwrap_or_else(|_|"unknown-month".into());
        (day,month)
    }else{("unknown-day".into(),"unknown-month".into())}
}
fn read_counter(interface:&str,name:&str)->Option<u64>{fs::read_to_string(format!("/sys/class/net/{interface}/statistics/{name}")).ok()?.trim().parse().ok()}
pub fn infer_interface(candidates:&[String])->Option<String>{
    for candidate in candidates{if PathBuf::from(format!("/sys/class/net/{candidate}")).exists(){return Some(candidate.clone());}}
    let entries=fs::read_dir("/sys/class/net").ok()?;
    for entry in entries.flatten(){let name=entry.file_name().to_string_lossy().to_string();if name.starts_with("ww")||name.starts_with("rmnet")||name.starts_with("usb"){return Some(name);}}
    None
}
pub fn sample(interface:&str)->Option<TrafficSample>{
    let rx=read_counter(interface,"rx_bytes")?;let tx=read_counter(interface,"tx_bytes")?;let now=Instant::now();let(day,month)=keys();let mut state=state().lock().ok()?;
    let elapsed=state.last_sample.map(|i|now.duration_since(i).as_secs_f64()).unwrap_or(0.0);let same=state.db.last_interface==interface;
    let drx=if same&&rx>=state.db.last_rx{rx-state.db.last_rx}else{0};let dtx=if same&&tx>=state.db.last_tx{tx-state.db.last_tx}else{0};
    if drx>0||dtx>0{let d=state.db.daily.entry(day.clone()).or_default();d.rx=d.rx.saturating_add(drx);d.tx=d.tx.saturating_add(dtx);let m=state.db.monthly.entry(month.clone()).or_default();m.rx=m.rx.saturating_add(drx);m.tx=m.tx.saturating_add(dtx);}
    state.db.last_interface=interface.to_string();state.db.last_rx=rx;state.db.last_tx=tx;state.last_sample=Some(now);
    while state.db.daily.len()>120{if let Some(key)=state.db.daily.keys().next().cloned(){state.db.daily.remove(&key);}else{break;}}
    while state.db.monthly.len()>24{if let Some(key)=state.db.monthly.keys().next().cloned(){state.db.monthly.remove(&key);}else{break;}}
    if state.last_save.elapsed()>=Duration::from_secs(30){save(&state.db);state.last_save=Instant::now();}
    let daily=state.db.daily.get(&day).cloned().unwrap_or_default();let monthly=state.db.monthly.get(&month).cloned().unwrap_or_default();
    Some(TrafficSample{interface:interface.into(),rx_bps:if elapsed>0.0{drx as f64/elapsed}else{0.0},tx_bps:if elapsed>0.0{dtx as f64/elapsed}else{0.0},today_rx:daily.rx,today_tx:daily.tx,month_rx:monthly.rx,month_tx:monthly.tx})
}
pub fn reset_usage()->Result<(),String>{let mut state=state().lock().map_err(|_|"Usage database is busy.".to_string())?;state.db.daily.clear();state.db.monthly.clear();save(&state.db);Ok(())}
pub fn human_bytes(value:u64)->String{human_rate(value as f64,false)}
pub fn human_bps(value:f64)->String{human_rate(value,true)}
fn human_rate(value:f64,per_sec:bool)->String{let suffix=if per_sec{"/s"}else{""};if value>=1024.0*1024.0*1024.0{format!("{:.2} GiB{}",value/(1024.0*1024.0*1024.0),suffix)}else if value>=1024.0*1024.0{format!("{:.2} MiB{}",value/(1024.0*1024.0),suffix)}else if value>=1024.0{format!("{:.1} KiB{}",value/1024.0,suffix)}else{format!("{:.0} B{}",value,suffix)}}
