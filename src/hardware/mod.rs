use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, sync::OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CompatStatus {
    HardwareVerified,
    SoftwareTested,
    GenericCompatible,
    Documented,
    Experimental,
    #[default]
    Untested,
    Unsupported,
    PermissionRequired,
}

impl CompatStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::HardwareVerified => "Hardware Verified",
            Self::SoftwareTested => "Software Tested",
            Self::GenericCompatible => "Generic Compatible",
            Self::Documented => "Documented",
            Self::Experimental => "Experimental",
            Self::Untested => "Untested",
            Self::Unsupported => "Unsupported",
            Self::PermissionRequired => "Permission Required",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HardwareCapabilitiesMap {
    pub internet_connection: CompatStatus,
    pub band_lock: CompatStatus,
    pub cell_lock: CompatStatus,
    pub carrier_aggregation: CompatStatus,
    pub esim: CompatStatus,
    pub gps: CompatStatus,
    pub fcc_lock: CompatStatus,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HardwareEntry {
    pub manufacturer: String,
    pub oem_manufacturer: String,
    pub model: String,
    pub commercial_name: String,
    pub aliases: Vec<String>,
    pub oem_aliases: Vec<String>,
    pub usb_ids: Vec<String>,
    pub pci_ids: Vec<String>,
    pub chipset: String,
    pub protocols: Vec<String>,
    pub firmware_restrictions: String,
    pub capabilities: HardwareCapabilitiesMap,
    pub known_issues: Vec<String>,
    pub official_references: Vec<String>,
    pub verification_status: CompatStatus,
    pub last_verified_date: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HardwareFamilyFile {
    pub schema_version: u32,
    pub family: String,
    pub last_updated: String,
    pub devices: Vec<HardwareEntry>,
}

#[derive(Debug, Clone)]
pub struct HardwareDatabase {
    pub entries: Vec<HardwareEntry>,
    pub family_counts: BTreeMap<String, usize>,
}

static HARDWARE_DB: OnceLock<HardwareDatabase> = OnceLock::new();

impl HardwareDatabase {
    pub fn global() -> &'static Self {
        HARDWARE_DB.get_or_init(Self::load_bundled)
    }

    pub fn load_bundled() -> Self {
        let sources: &[&str] = &[
            include_str!("../../resources/hardware/dell.json"),
            include_str!("../../resources/hardware/fibocom.json"),
            include_str!("../../resources/hardware/quectel.json"),
            include_str!("../../resources/hardware/sierra.json"),
            include_str!("../../resources/hardware/foxconn.json"),
            include_str!("../../resources/hardware/rolling-wireless.json"),
            include_str!("../../resources/hardware/huawei.json"),
            include_str!("../../resources/hardware/telit.json"),
            include_str!("../../resources/hardware/simcom.json"),
            include_str!("../../resources/hardware/generic.json"),
        ];

        let mut entries = Vec::new();
        let mut family_counts = BTreeMap::new();

        for raw in sources {
            if let Ok(parsed) = serde_json::from_str::<HardwareFamilyFile>(raw) {
                family_counts.insert(parsed.family, parsed.devices.len());
                entries.extend(parsed.devices);
            }
        }

        // Load optional user overrides (never contains executable commands).
        let override_path =
            crate::core::settings::config_home().join("modemdeck/hardware_overrides.json");
        if let Ok(text) = fs::read_to_string(override_path)
            && let Ok(parsed) = serde_json::from_str::<HardwareFamilyFile>(&text)
        {
            for custom in parsed.devices {
                entries.push(custom);
            }
        }

        Self {
            entries,
            family_counts,
        }
    }

    pub fn total_models(&self) -> usize {
        self.entries.len()
    }

    pub fn lookup(
        &self,
        manufacturer: &str,
        model: &str,
        usb_vid: Option<u16>,
        usb_pid: Option<u16>,
        pci_vid: Option<u16>,
        pci_pid: Option<u16>,
    ) -> Option<&HardwareEntry> {
        if let (Some(v), Some(p)) = (usb_vid, usb_pid) {
            let needle = format!("{v:04x}:{p:04x}");
            if let Some(found) = self
                .entries
                .iter()
                .find(|e| e.usb_ids.iter().any(|id| id.eq_ignore_ascii_case(&needle)))
            {
                return Some(found);
            }
        }
        if let (Some(v), Some(p)) = (pci_vid, pci_pid) {
            let needle = format!("{v:04x}:{p:04x}");
            if let Some(found) = self
                .entries
                .iter()
                .find(|e| e.pci_ids.iter().any(|id| id.eq_ignore_ascii_case(&needle)))
            {
                return Some(found);
            }
        }

        let combined = format!("{manufacturer} {model}").to_ascii_lowercase();
        if combined.trim().is_empty() {
            return None;
        }

        // Exact model or alias match first
        self.entries.iter().find(|e| {
            let m = e.model.to_ascii_lowercase();
            (!m.is_empty() && combined.contains(&m))
                || e.aliases
                    .iter()
                    .chain(e.oem_aliases.iter())
                    .any(|a| !a.is_empty() && combined.contains(&a.to_ascii_lowercase()))
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaptopInfo {
    pub sys_vendor: String,
    pub product_name: String,
    pub product_family: String,
    pub recognized_family: String,
}

impl LaptopInfo {
    pub fn detect() -> Self {
        let read_dmi = |file: &str| -> String {
            fs::read_to_string(Path::new("/sys/class/dmi/id").join(file))
                .unwrap_or_default()
                .trim()
                .to_string()
        };
        let sys_vendor = read_dmi("sys_vendor");
        let product_name = read_dmi("product_name");
        let product_family = read_dmi("product_family");
        let hay = format!("{sys_vendor} {product_name} {product_family}").to_ascii_lowercase();

        let recognized_family = if hay.contains("latitude") {
            "Dell Latitude"
        } else if hay.contains("precision") {
            "Dell Precision"
        } else if hay.contains("thinkpad") {
            "Lenovo ThinkPad"
        } else if hay.contains("elitebook") {
            "HP EliteBook"
        } else if hay.contains("probook") {
            "HP ProBook"
        } else if hay.contains("zbook") {
            "HP ZBook"
        } else if hay.contains("surface") {
            "Microsoft Surface"
        } else if hay.contains("toughbook") || hay.contains("fz-") || hay.contains("cf-") {
            "Panasonic Toughbook"
        } else if hay.contains("getac") {
            "Getac"
        } else if !sys_vendor.is_empty() {
            "Generic Laptop / PC"
        } else {
            "Unknown"
        }
        .to_string();

        Self {
            sys_vendor,
            product_name,
            product_family,
            recognized_family,
        }
    }

    pub fn display_label(&self) -> String {
        if self.product_name.is_empty() {
            self.recognized_family.clone()
        } else {
            format!("{} ({})", self.product_name, self.recognized_family)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SysfsHardwareDetails {
    pub usb_vid: Option<u16>,
    pub usb_pid: Option<u16>,
    pub pci_vid: Option<u16>,
    pub pci_pid: Option<u16>,
    pub kernel_driver: String,
}

/// Best-effort read of USB/PCI IDs and kernel driver from `/sys/class/net/<iface>/device` or `/dev/<port>`.
pub fn inspect_sysfs_hardware(net_ports: &[String], primary_port: &str) -> SysfsHardwareDetails {
    let mut details = SysfsHardwareDetails::default();
    let candidate_dirs: Vec<std::path::PathBuf> = net_ports
        .iter()
        .map(|iface| Path::new("/sys/class/net").join(iface).join("device"))
        .chain((!primary_port.is_empty()).then(|| {
            Path::new("/sys/class/wwan")
                .join(primary_port)
                .join("device")
        }))
        .chain((!primary_port.is_empty()).then(|| {
            Path::new("/sys/class/usbmisc")
                .join(primary_port)
                .join("device")
        }))
        .collect();

    for dev_path in candidate_dirs {
        if !dev_path.exists() {
            continue;
        }
        if details.kernel_driver.is_empty()
            && let Ok(link) = fs::read_link(dev_path.join("driver"))
            && let Some(name) = link.file_name().and_then(|n| n.to_str())
        {
            details.kernel_driver = name.to_string();
        }
        // Check PCI vendor/device
        if let (Some(v), Some(d)) = (
            read_hex_u16(&dev_path.join("vendor")),
            read_hex_u16(&dev_path.join("device")),
        ) {
            details.pci_vid = Some(v);
            details.pci_pid = Some(d);
        }
        // Check USB idVendor/idProduct on device or parent
        for p in [dev_path.clone(), dev_path.join("..")] {
            if let (Some(v), Some(d)) = (
                read_hex_u16(&p.join("idVendor")),
                read_hex_u16(&p.join("idProduct")),
            ) {
                details.usb_vid = Some(v);
                details.usb_pid = Some(d);
                break;
            }
        }
        if details.usb_vid.is_some() || details.pci_vid.is_some() {
            break;
        }
    }
    details
}

fn read_hex_u16(path: &Path) -> Option<u16> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim().trim_start_matches("0x");
    u16::from_str_radix(trimmed, 16).ok()
}
