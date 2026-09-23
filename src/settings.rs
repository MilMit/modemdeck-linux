use serde::{Deserialize, Serialize};
use std::{env, fs, io, path::PathBuf, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub background_monitor: bool,
    pub notifications: bool,
    pub signal_refresh_seconds: u32,
    pub traffic_refresh_seconds: u32,
    pub language: String,
    pub restore_profile_on_startup: bool,
    pub check_updates_on_startup: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { background_monitor:false,notifications:true,signal_refresh_seconds:10,traffic_refresh_seconds:2,language:"system".into(),restore_profile_on_startup:false,check_updates_on_startup:false }
    }
}

impl Settings {
    pub fn load() -> Self {
        fs::read_to_string(settings_path()).ok().and_then(|text| serde_json::from_str::<Self>(&text).ok()).unwrap_or_default()
    }
    pub fn save(&self) -> io::Result<()> {
        let path=settings_path(); if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
        let text=serde_json::to_string_pretty(self).map_err(|e|io::Error::new(io::ErrorKind::Other,e.to_string()))?;
        fs::write(path,text)
    }
    pub fn normalized_refresh_seconds(&self)->u32{self.signal_refresh_seconds.clamp(5,60)}
    pub fn normalized_traffic_refresh_seconds(&self)->u32{self.traffic_refresh_seconds.clamp(1,30)}
}
pub fn settings_path()->PathBuf{config_home().join("modemdeck/settings.json")}
pub fn config_home()->PathBuf{
    env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(||env::var_os("HOME").map(|home|PathBuf::from(home).join(".config")))
        .unwrap_or_else(||PathBuf::from(".config"))
}
pub fn set_background_monitor_enabled(enabled:bool)->Result<String,String>{
    let action=if enabled{"enable"}else{"disable"};
    let output=Command::new("systemctl").args(["--user",action,"--now","net.milmit.ModemDeck-monitor.service"]).output()
        .map_err(|e|format!("Could not run systemctl --user: {e}"))?;
    if output.status.success(){Ok(if enabled{"Background monitor enabled.".into()}else{"Background monitor disabled.".into()})}
    else{let e=String::from_utf8_lossy(&output.stderr).trim().to_string();Err(if e.is_empty(){"systemctl --user returned an error.".into()}else{e})}
}
pub fn restart_background_monitor()->Result<(),String>{
    let output=Command::new("systemctl").args(["--user","restart","net.milmit.ModemDeck-monitor.service"]).output()
        .map_err(|e|format!("Could not restart background monitor: {e}"))?;
    if output.status.success(){Ok(())}else{let e=String::from_utf8_lossy(&output.stderr).trim().to_string();Err(if e.is_empty(){"Background monitor restart failed.".into()}else{e})}
}
