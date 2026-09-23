use serde::Deserialize;
use std::time::Duration;

pub const MANIFEST_URL: &str = "https://milmit.net/modemdeck/update.json";
pub const WEBSITE_URL: &str = "https://milmit.net";

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone)]
pub enum UpdateState { Current(UpdateManifest), Available(UpdateManifest) }

pub fn check(current: &str) -> Result<UpdateState, String> {
    let client=reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(4)).timeout(Duration::from_secs(8))
        .user_agent(concat!("ModemDeck/",env!("CARGO_PKG_VERSION")," (MilMit; https://milmit.net)"))
        .build().map_err(|error|format!("Could not initialize the update client: {error}"))?;
    let response=client.get(MANIFEST_URL).send()
        .map_err(|error|format!("Could not read the MilMit update manifest: {error}"))?
        .error_for_status().map_err(|error|format!("MilMit update server returned an error: {error}"))?;
    let manifest:UpdateManifest=response.json().map_err(|error|format!("MilMit update manifest is invalid: {error}"))?;
    if newer_than(&manifest.version,current){Ok(UpdateState::Available(manifest))}else{Ok(UpdateState::Current(manifest))}
}

fn version_numbers(value:&str)->Vec<u64>{
    value.trim_start_matches('v').split(|c:char|!c.is_ascii_digit()).filter(|s|!s.is_empty()).take(4).map(|s|s.parse::<u64>().unwrap_or(0)).collect()
}
fn newer_than(remote:&str,local:&str)->bool{
    let mut a=version_numbers(remote);let mut b=version_numbers(local);let len=a.len().max(b.len());a.resize(len,0);b.resize(len,0);a>b
}
pub fn open_url(url:&str)->Result<(),String>{
    gio::AppInfo::launch_default_for_uri(url,None::<&gio::AppLaunchContext>).map_err(|e|format!("Could not open link: {e}"))
}
