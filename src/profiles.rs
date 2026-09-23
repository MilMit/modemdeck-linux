use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

use crate::settings;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BandProfile {
    pub name: String,
    pub bands: Vec<u32>,
    #[serde(default)]
    pub built_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileStore {
    pub profiles: Vec<BandProfile>,
    pub active_profile: String,
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self {
            profiles: default_profiles(),
            active_profile: "Auto".into(),
        }
    }
}

pub fn default_profiles() -> Vec<BandProfile> {
    vec![
        BandProfile { name: "Auto".into(), bands: vec![256], built_in: true },
        BandProfile { name: "B42 TD-LTE".into(), bands: vec![72], built_in: true },
        BandProfile { name: "B1+B3+B7".into(), bands: vec![31, 33, 37], built_in: true },
    ]
}

pub fn path() -> PathBuf { settings::config_home().join("modemdeck/profiles.json") }

pub fn load() -> ProfileStore {
    let mut store = fs::read_to_string(path()).ok().and_then(|text| serde_json::from_str::<ProfileStore>(&text).ok()).unwrap_or_default();
    for built_in in default_profiles() {
        if !store.profiles.iter().any(|p| p.name == built_in.name) { store.profiles.push(built_in); }
    }
    store
}

pub fn save(store: &ProfileStore) -> io::Result<()> {
    let path = path();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let data = serde_json::to_vec_pretty(store).map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
    fs::write(path, data)
}

pub fn set_active(name: &str) -> io::Result<()> {
    let mut store = load(); store.active_profile = name.to_string(); save(&store)
}

pub fn upsert(name: &str, bands: Vec<u32>) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() { return Err("Profile name cannot be empty.".into()); }
    if bands.is_empty() { return Err("A profile must contain at least one band.".into()); }
    let mut store = load();
    if let Some(existing) = store.profiles.iter_mut().find(|p| p.name.eq_ignore_ascii_case(name)) {
        if existing.built_in { return Err("Built-in profiles cannot be overwritten. Choose another name.".into()); }
        existing.name = name.to_string(); existing.bands = bands;
    } else { store.profiles.push(BandProfile { name: name.to_string(), bands, built_in: false }); }
    save(&store).map_err(|e| e.to_string())
}

pub fn remove(name: &str) -> Result<(), String> {
    let mut store = load();
    let Some(profile) = store.profiles.iter().find(|p| p.name == name) else { return Err("Profile not found.".into()); };
    if profile.built_in { return Err("Built-in profiles cannot be deleted.".into()); }
    store.profiles.retain(|p| p.name != name);
    if store.active_profile == name { store.active_profile = "Auto".into(); }
    save(&store).map_err(|e| e.to_string())
}

pub fn find(name: &str) -> Option<BandProfile> { load().profiles.into_iter().find(|p| p.name == name) }

pub fn matching_name(bands: &[u32]) -> Option<String> {
    let mut target = bands.to_vec(); target.sort_unstable();
    load().profiles.into_iter().find_map(|p| {
        let mut pb = p.bands.clone(); pb.sort_unstable();
        (pb == target).then_some(p.name)
    })
}
