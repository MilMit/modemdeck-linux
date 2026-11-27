use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub const SYSTEM_MBPI_XML: &str = "/usr/share/mobile-broadband-provider-info/serviceproviders.xml";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OperatorApn {
    pub name: String,
    pub apn: String,
    pub profile_type: String, // "mobile" | "fwa" | "td-lte"
    pub ip_type: String,      // "ipv4" | "ipv6" | "ipv4v6"
    pub auth: String,         // "none" | "pap" | "chap"
    pub username: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct UssdCodeInfo {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OperatorEntry {
    pub id: String,
    pub name: String,
    pub country: String,
    pub country_code: String,
    pub mcc: String,
    pub mnc: String,
    pub spn_patterns: Vec<String>,
    pub gid1_patterns: Vec<String>,
    pub is_mvno: bool,
    pub host_network: String,
    pub apns: Vec<OperatorApn>,
    pub website: String,
    pub support_url: String,
    pub account_url: String,
    pub package_url: String,
    pub ussd_codes: Vec<UssdCodeInfo>,
    pub known_bands: Vec<String>,
    pub technologies: Vec<String>,
    pub active: bool,
    pub source: String,
    pub last_updated: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OperatorDatabaseFile {
    pub schema_version: u32,
    pub source: String,
    pub last_updated: String,
    pub signature: String,
    pub operators: Vec<OperatorEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentificationConfidence {
    Exact,
    MvnoMatched,
    HostOnly,
    Ambiguous,
    ManualOverride,
    Unknown,
}

impl IdentificationConfidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Exact => "Exact",
            Self::MvnoMatched => "MVNO Matched",
            Self::HostOnly => "Host Network Only",
            Self::Ambiguous => "Ambiguous",
            Self::ManualOverride => "Manual Selection",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperatorMatch {
    pub sim_operator: Option<OperatorEntry>,
    pub registered_network: Option<OperatorEntry>,
    pub confidence: IdentificationConfidence,
    pub roaming: bool,
    pub candidates: Vec<OperatorEntry>,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct OperatorDatabase {
    pub operators: Vec<OperatorEntry>,
    pub countries: BTreeSet<String>,
    pub sources: Vec<String>,
}

static OPERATOR_DB: OnceLock<OperatorDatabase> = OnceLock::new();

impl OperatorDatabase {
    pub fn global() -> &'static Self {
        OPERATOR_DB.get_or_init(Self::load)
    }

    pub fn load() -> Self {
        let mut operators = Vec::new();
        let mut sources = Vec::new();
        let mut seen_keys: HashMap<String, usize> = HashMap::new();

        // 1. Curated Iran + Global supplement (high metadata fidelity: USSD, URLs, MVNOs)
        for (raw, default_source) in [
            (
                include_str!("../../resources/operators/iran_operators.json"),
                "ModemDeck Iran Registry",
            ),
            (
                include_str!("../../resources/operators/global_supplement.json"),
                "ModemDeck Global Supplement",
            ),
        ] {
            if let Ok(parsed) = serde_json::from_str::<OperatorDatabaseFile>(raw) {
                let src = if parsed.source.is_empty() {
                    default_source.to_string()
                } else {
                    parsed.source.clone()
                };
                sources.push(src.clone());
                for mut op in parsed.operators {
                    if op.source.is_empty() {
                        op.source = src.clone();
                    }
                    if op.last_updated.is_empty() {
                        op.last_updated = parsed.last_updated.clone();
                    }
                    let key = format!("{}:{}:{}", op.mcc, op.mnc, op.name.to_ascii_lowercase());
                    seen_keys.insert(key, operators.len());
                    operators.push(op);
                }
            }
        }

        // 2. System mobile-broadband-provider-info XML (CC0-1.0)
        if let Ok(xml) = fs::read_to_string(SYSTEM_MBPI_XML) {
            let parsed = parse_serviceproviders_xml(&xml);
            if !parsed.is_empty() {
                sources.push("mobile-broadband-provider-info (system XML)".into());
                for op in parsed {
                    let key = format!("{}:{}:{}", op.mcc, op.mnc, op.name.to_ascii_lowercase());
                    if let std::collections::hash_map::Entry::Vacant(e) = seen_keys.entry(key) {
                        e.insert(operators.len());
                        operators.push(op);
                    }
                }
            }
        }

        // 3. Optional downloaded/updated operator database in data_home
        let custom_db = db_active_path();
        if let Ok(text) = fs::read_to_string(custom_db)
            && let Ok(parsed) = serde_json::from_str::<OperatorDatabaseFile>(&text)
        {
            sources.push(format!("Updated DB ({})", parsed.source));
            for op in parsed.operators {
                let key = format!("{}:{}:{}", op.mcc, op.mnc, op.name.to_ascii_lowercase());
                if let Some(&idx) = seen_keys.get(&key) {
                    operators[idx] = op;
                } else {
                    seen_keys.insert(key, operators.len());
                    operators.push(op);
                }
            }
        }

        // 4. Optional user overrides
        let override_path =
            crate::core::settings::config_home().join("modemdeck/operator_overrides.json");
        if let Ok(text) = fs::read_to_string(override_path)
            && let Ok(parsed) = serde_json::from_str::<OperatorDatabaseFile>(&text)
        {
            sources.push("User Operator Overrides".into());
            for op in parsed.operators {
                operators.push(op);
            }
        }

        let countries: BTreeSet<String> = operators
            .iter()
            .filter(|o| !o.country.trim().is_empty())
            .map(|o| o.country.clone())
            .collect();

        Self {
            operators,
            countries,
            sources,
        }
    }

    pub fn operator_count(&self) -> usize {
        self.operators.len()
    }

    pub fn country_count(&self) -> usize {
        self.countries.len()
    }

    pub fn by_mcc_mnc(&self, mcc: &str, mnc: &str) -> Vec<OperatorEntry> {
        let mnc_trimmed = mnc.trim_start_matches('0');
        self.operators
            .iter()
            .filter(|o| {
                o.mcc == mcc && (o.mnc == mnc || o.mnc.trim_start_matches('0') == mnc_trimmed)
            })
            .cloned()
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn identify(
        &self,
        sim_plmn: &str,
        sim_spn: &str,
        sim_gid1: &str,
        reg_plmn: &str,
        reg_name: &str,
        roaming: bool,
        manual_override_id: Option<&str>,
    ) -> OperatorMatch {
        if let Some(id) = manual_override_id
            && let Some(op) = self.operators.iter().find(|o| o.id == id).cloned()
        {
            return OperatorMatch {
                sim_operator: Some(op.clone()),
                registered_network: self.lookup_single(reg_plmn, reg_name),
                confidence: IdentificationConfidence::ManualOverride,
                roaming,
                candidates: vec![op],
                notes: "User-selected manual operator profile.".into(),
            };
        }

        let registered_network = self.lookup_single(reg_plmn, reg_name);
        let (mcc, mnc) = split_plmn(sim_plmn)
            .or_else(|| split_plmn(reg_plmn))
            .unwrap_or(("", ""));

        if mcc.is_empty() {
            return OperatorMatch {
                sim_operator: None,
                registered_network,
                confidence: IdentificationConfidence::Unknown,
                roaming,
                candidates: Vec::new(),
                notes: "MCC/MNC not reported by SIM or network.".into(),
            };
        }

        let candidates = self.by_mcc_mnc(mcc, mnc);
        if candidates.is_empty() {
            return OperatorMatch {
                sim_operator: None,
                registered_network,
                confidence: IdentificationConfidence::Unknown,
                roaming,
                candidates,
                notes: format!("No operator entry found for PLMN {mcc}-{mnc}."),
            };
        }

        let spn_lower = sim_spn.trim().to_ascii_lowercase();
        let gid1_lower = sim_gid1.trim().to_ascii_lowercase();

        // Check MVNO / SPN / GID1 match first so MVNOs are never blindly collapsed to host network
        if (!spn_lower.is_empty() || !gid1_lower.is_empty())
            && let Some(mvno) = candidates.iter().find(|c| {
                (!gid1_lower.is_empty()
                    && c.gid1_patterns
                        .iter()
                        .any(|g| gid1_lower.contains(&g.to_ascii_lowercase())))
                    || (!spn_lower.is_empty()
                        && c.spn_patterns
                            .iter()
                            .any(|p| spn_lower.contains(&p.to_ascii_lowercase())))
            })
        {
            return OperatorMatch {
                sim_operator: Some(mvno.clone()),
                registered_network,
                confidence: if mvno.is_mvno {
                    IdentificationConfidence::MvnoMatched
                } else {
                    IdentificationConfidence::Exact
                },
                roaming,
                candidates,
                notes: String::new(),
            };
        }

        if candidates.len() == 1 {
            let only = candidates[0].clone();
            let conf = if only.is_mvno && spn_lower.is_empty() {
                IdentificationConfidence::HostOnly
            } else {
                IdentificationConfidence::Exact
            };
            return OperatorMatch {
                sim_operator: Some(only),
                registered_network,
                confidence: conf,
                roaming,
                candidates,
                notes: String::new(),
            };
        }

        // Multiple operators share this MCC/MNC (e.g., host MNO + MVNOs) and SPN/GID1 did not disambiguate
        let primary = candidates.iter().find(|c| !c.is_mvno).cloned();
        OperatorMatch {
            sim_operator: primary,
            registered_network,
            confidence: IdentificationConfidence::Ambiguous,
            roaming,
            candidates,
            notes: "Multiple operators or MVNOs share this MCC/MNC; verify SPN or select manually."
                .into(),
        }
    }

    fn lookup_single(&self, plmn: &str, name_hint: &str) -> Option<OperatorEntry> {
        let (mcc, mnc) = split_plmn(plmn)?;
        let list = self.by_mcc_mnc(mcc, mnc);
        if list.is_empty() {
            return None;
        }
        let hint = name_hint.trim().to_ascii_lowercase();
        if !hint.is_empty()
            && let Some(found) = list
                .iter()
                .find(|o| o.name.to_ascii_lowercase().contains(&hint))
        {
            return Some(found.clone());
        }
        list.into_iter()
            .find(|o| !o.is_mvno)
            .or_else(|| self.by_mcc_mnc(mcc, mnc).into_iter().next())
    }
}

pub fn split_plmn(plmn: &str) -> Option<(&str, &str)> {
    let clean = plmn.trim();
    if let Some((mcc, mnc)) = clean.split_once('-')
        && mcc.len() == 3
        && !mnc.is_empty()
    {
        return Some((mcc, mnc));
    }
    if clean.len() >= 5 && clean.chars().all(|c| c.is_ascii_digit()) {
        return Some((&clean[..3], &clean[3..]));
    }
    None
}

/// Lightweight streaming XML parser for `mobile-broadband-provider-info/serviceproviders.xml`.
pub fn parse_serviceproviders_xml(xml: &str) -> Vec<OperatorEntry> {
    let mut results = Vec::new();
    let mut current_country = String::new();
    let mut current_name = String::new();
    let mut current_mccs: Vec<(String, String)> = Vec::new();
    let mut current_apns: Vec<OperatorApn> = Vec::new();
    let mut active_apn: Option<OperatorApn> = None;
    let mut in_provider = false;
    let mut in_name = false;

    for raw_line in xml.lines() {
        let line = raw_line.trim();
        if line.starts_with("<country ") {
            current_country = extract_attr(line, "code")
                .unwrap_or_default()
                .to_ascii_uppercase();
        } else if line.starts_with("</country>") {
            current_country.clear();
        } else if line.starts_with("<provider") {
            in_provider = true;
            current_name.clear();
            current_mccs.clear();
            current_apns.clear();
        } else if line.starts_with("</provider>") && in_provider {
            in_provider = false;
            for (mcc, mnc) in &current_mccs {
                results.push(OperatorEntry {
                    id: format!("mbpi-{}-{}-{}", current_country.to_lowercase(), mcc, mnc),
                    name: if current_name.is_empty() {
                        format!("{mcc}-{mnc}")
                    } else {
                        current_name.clone()
                    },
                    country: country_name_from_code(&current_country),
                    country_code: current_country.clone(),
                    mcc: mcc.clone(),
                    mnc: mnc.clone(),
                    spn_patterns: vec![current_name.to_ascii_lowercase()],
                    gid1_patterns: Vec::new(),
                    is_mvno: false,
                    host_network: String::new(),
                    apns: current_apns.clone(),
                    website: String::new(),
                    support_url: String::new(),
                    account_url: String::new(),
                    package_url: String::new(),
                    ussd_codes: Vec::new(),
                    known_bands: Vec::new(),
                    technologies: vec!["GSM/UMTS/LTE".into()],
                    active: true,
                    source: "mobile-broadband-provider-info (CC0-1.0)".into(),
                    last_updated: "system".into(),
                });
            }
        } else if in_provider {
            if line.starts_with("<name") && current_name.is_empty() {
                if let Some(text) = extract_tag_text(line, "name") {
                    current_name = text;
                } else {
                    in_name = true;
                }
            } else if in_name {
                if let Some(end) = line.find("</name>") {
                    current_name.push_str(&line[..end]);
                    in_name = false;
                }
            } else if line.starts_with("<network-id ") {
                if let (Some(mcc), Some(mnc)) =
                    (extract_attr(line, "mcc"), extract_attr(line, "mnc"))
                {
                    current_mccs.push((mcc, mnc));
                }
            } else if line.starts_with("<apn ") {
                if let Some(val) = extract_attr(line, "value") {
                    active_apn = Some(OperatorApn {
                        name: val.clone(),
                        apn: val,
                        profile_type: "mobile".into(),
                        ip_type: "ipv4v6".into(),
                        auth: "none".into(),
                        username: String::new(),
                        recommended: current_apns.is_empty(),
                    });
                    if line.ends_with("/>")
                        && let Some(apn) = active_apn.take()
                    {
                        current_apns.push(apn);
                    }
                }
            } else if line.starts_with("</apn>") {
                if let Some(apn) = active_apn.take() {
                    current_apns.push(apn);
                }
            } else if let Some(ref mut apn) = active_apn {
                if let Some(u) = extract_tag_text(line, "username") {
                    apn.username = u;
                } else if let Some(n) = extract_tag_text(line, "name") {
                    apn.name = n;
                }
            }
        }
    }
    results
}

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn extract_tag_text(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    if !line.starts_with(&open) {
        return None;
    }
    let gt = line.find('>')? + 1;
    let end = line[gt..].find(&close)? + gt;
    Some(line[gt..end].trim().to_string())
}

fn country_name_from_code(code: &str) -> String {
    match code {
        "IR" => "Iran",
        "CA" => "Canada",
        "US" => "United States",
        "NO" => "Norway",
        "GB" => "United Kingdom",
        "DE" => "Germany",
        "FR" => "France",
        "ES" => "Spain",
        "IT" => "Italy",
        "SE" => "Sweden",
        "FI" => "Finland",
        "NL" => "Netherlands",
        "CH" => "Switzerland",
        "AT" => "Austria",
        "TR" => "Turkey",
        "AE" => "United Arab Emirates",
        "SA" => "Saudi Arabia",
        "QA" => "Qatar",
        "OM" => "Oman",
        "IQ" => "Iraq",
        "AM" => "Armenia",
        "GE" => "Georgia",
        "AZ" => "Azerbaijan",
        "TJ" => "Tajikistan",
        "UZ" => "Uzbekistan",
        "KZ" => "Kazakhstan",
        "RU" => "Russia",
        "IN" => "India",
        "PK" => "Pakistan",
        "CN" => "China",
        "JP" => "Japan",
        "KR" => "South Korea",
        "AU" => "Australia",
        "NZ" => "New Zealand",
        other => other,
    }
    .to_string()
}

pub fn db_dir() -> PathBuf {
    crate::core::settings::data_home().join("modemdeck/operators")
}

pub fn db_active_path() -> PathBuf {
    db_dir().join("operators.json")
}

pub fn db_backup_path() -> PathBuf {
    db_dir().join("operators.json.bak")
}

/// Computes a deterministic 64-bit FNV-1a integrity checksum over canonical payload.
pub fn compute_integrity_tag(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Validates and atomically installs an updated operator database with backup & rollback.
pub fn install_operator_db_update(
    json_bytes: &[u8],
    expected_integrity: Option<&str>,
) -> Result<usize, String> {
    if let Some(expected) = expected_integrity {
        let actual = compute_integrity_tag(json_bytes);
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err(format!(
                "Integrity verification failed (expected {expected}, got {actual})."
            ));
        }
    }
    let parsed: OperatorDatabaseFile =
        serde_json::from_slice(json_bytes).map_err(|e| format!("Invalid operator DB JSON: {e}"))?;
    if parsed.schema_version == 0 || parsed.operators.is_empty() {
        return Err("Operator DB rejected: missing schema_version or empty operator list.".into());
    }

    let dir = db_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create operator DB directory: {e}"))?;
    let active = db_active_path();
    let backup = db_backup_path();
    let tmp = dir.join("operators.json.tmp");

    if active.exists() {
        fs::copy(&active, &backup).map_err(|e| format!("Backup failed: {e}"))?;
    }
    fs::write(&tmp, json_bytes).map_err(|e| format!("Write temp DB failed: {e}"))?;
    if let Err(e) = fs::rename(&tmp, &active) {
        if backup.exists() {
            let _ = fs::copy(&backup, &active);
        }
        return Err(format!("Atomic DB replacement failed (rolled back): {e}"));
    }
    Ok(parsed.operators.len())
}

pub fn rollback_operator_db() -> Result<(), String> {
    let active = db_active_path();
    let backup = db_backup_path();
    if !backup.exists() {
        return Err("No operator database backup available to roll back.".into());
    }
    fs::copy(&backup, &active).map_err(|e| format!("Rollback failed: {e}"))?;
    Ok(())
}

pub fn export_custom_operators(path: &Path, entries: &[OperatorEntry]) -> Result<(), String> {
    let bundle = OperatorDatabaseFile {
        schema_version: 1,
        source: "ModemDeck Custom Export".into(),
        last_updated: "export".into(),
        signature: String::new(),
        operators: entries.to_vec(),
    };
    let json = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}
