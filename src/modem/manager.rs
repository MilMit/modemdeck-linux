//! ModemManager D-Bus client powered by pure-Rust async `zbus` on a shared Tokio runtime.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use zbus::{OwnedObjectPath, OwnedVariant as OwnedValue};

use crate::{
    core::{block_on, profiles, safety},
    drivers::DriverKind,
    model::{
        BandChangeState, ModemInfo, RadioSignal, ServingCellInfo, SignalSnapshot, SimIdentity,
        VendorInfo,
    },
    modem::helper,
};

const MM_BUS: &str = "org.freedesktop.ModemManager1";
const MM_ROOT: &str = "/org/freedesktop/ModemManager1";
const MODEM_IFACE: &str = "org.freedesktop.ModemManager1.Modem";
const MODEM_3GPP_IFACE: &str = "org.freedesktop.ModemManager1.Modem.Modem3gpp";
const MODEM_SIGNAL_IFACE: &str = "org.freedesktop.ModemManager1.Modem.Signal";
const MODEM_LOCATION_IFACE: &str = "org.freedesktop.ModemManager1.Modem.Location";
const SIM_IFACE: &str = "org.freedesktop.ModemManager1.Sim";

type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

#[derive(Debug, Clone, Default)]
struct CellCacheEntry {
    cells: Vec<ServingCellInfo>,
    error: String,
    source: String,
}

#[derive(Debug, Clone, Default)]
struct SignalCacheEntry {
    lte: SignalSnapshot,
    nr5g: SignalSnapshot,
    context_key: String,
}

#[derive(Clone)]
pub struct ModemManagerClient {
    conn: zbus::Connection,
    signal_rates_changed: Arc<Mutex<HashMap<String, u32>>>,
    cell_cache: Arc<Mutex<HashMap<String, CellCacheEntry>>>,
    cell_privileged_attempts: Arc<Mutex<HashSet<String>>>,
    vendor_cache: Arc<Mutex<HashMap<String, VendorInfo>>>,
    band_change_cache: Arc<Mutex<HashMap<String, BandChangeState>>>,
    signal_cache: Arc<Mutex<HashMap<String, SignalCacheEntry>>>,
    sim_cache: Arc<Mutex<HashMap<String, SimIdentity>>>,
}

impl ModemManagerClient {
    pub fn new() -> Result<Self, String> {
        let conn = block_on(zbus::Connection::system())
            .map_err(|e| format!("Could not connect to system D-Bus: {e}"))?;
        Ok(Self {
            conn,
            signal_rates_changed: Arc::new(Mutex::new(HashMap::new())),
            cell_cache: Arc::new(Mutex::new(HashMap::new())),
            cell_privileged_attempts: Arc::new(Mutex::new(HashSet::new())),
            vendor_cache: Arc::new(Mutex::new(HashMap::new())),
            band_change_cache: Arc::new(Mutex::new(HashMap::new())),
            signal_cache: Arc::new(Mutex::new(HashMap::new())),
            sim_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn managed_objects(&self) -> ManagedObjects {
        block_on(async {
            let Ok(proxy) = zbus::Proxy::new(
                &self.conn,
                MM_BUS,
                MM_ROOT,
                "org.freedesktop.DBus.ObjectManager",
            )
            .await
            else {
                return HashMap::new();
            };
            proxy
                .call::<_, _, ManagedObjects>("GetManagedObjects", &())
                .await
                .unwrap_or_default()
        })
    }

    pub fn has_interface(&self, object_path: &str, interface_name: &str) -> bool {
        let objects = self.managed_objects();
        objects
            .iter()
            .find(|(p, _)| p.as_str() == object_path)
            .is_some_and(|(_, ifaces)| ifaces.contains_key(interface_name))
    }

    pub fn modems(&self) -> Vec<ModemInfo> {
        let objects = self.managed_objects();
        let mut modems: Vec<_> = objects
            .iter()
            .filter_map(|(path, ifaces)| self.read_modem(path.as_str(), ifaces))
            .collect();
        modems.sort_by(|a, b| a.object_path.cmp(&b.object_path));
        modems
    }

    pub fn ensure_signal_updates(&self, requested_rate: u32) {
        let objects = self.managed_objects();
        for (path, ifaces) in &objects {
            if let Some(sig_props) = ifaces.get(MODEM_SIGNAL_IFACE) {
                let path_str = path.as_str().to_string();
                let already = self
                    .signal_rates_changed
                    .lock()
                    .map(|m| m.contains_key(&path_str))
                    .unwrap_or(false);
                if already {
                    continue;
                }
                let prev = prop_u32(sig_props, "Rate").unwrap_or(0);
                if prev != 0 && prev <= requested_rate {
                    continue;
                }
                let ok = block_on(async {
                    let Ok(proxy) =
                        zbus::Proxy::new(&self.conn, MM_BUS, path.as_str(), MODEM_SIGNAL_IFACE)
                            .await
                    else {
                        return false;
                    };
                    proxy
                        .call::<_, _, ()>("Setup", &(requested_rate,))
                        .await
                        .is_ok()
                });
                if ok && let Ok(mut map) = self.signal_rates_changed.lock() {
                    map.insert(path_str, prev);
                }
            }
        }
    }

    pub fn restore_signal_updates(&self, requested_rate: u32) {
        let entries: Vec<(String, u32)> = self
            .signal_rates_changed
            .lock()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default();
        for (path, prev) in entries {
            block_on(async {
                if let Ok(proxy) =
                    zbus::Proxy::new(&self.conn, MM_BUS, path.as_str(), MODEM_SIGNAL_IFACE).await
                {
                    let cur = proxy.get_property::<u32>("Rate").await.unwrap_or(0);
                    if cur == requested_rate {
                        let _ = proxy.call::<_, _, ()>("Setup", &(prev,)).await;
                    }
                }
            });
        }
    }

    pub fn recover_pending_band_change(&self) -> Result<Option<String>, String> {
        let Some(pending) = safety::read_pending() else {
            return Ok(None);
        };
        let modems = self.modems();
        let target = modems.iter().find(|m| {
            (!pending.device_uid.is_empty() && m.device_uid == pending.device_uid)
                || m.object_path == pending.object_path
        });
        let Some(modem) = target else {
            return Ok(Some(
                "A previous band change was interrupted, but the modem is not currently attached."
                    .into(),
            ));
        };
        if modem.current_bands == pending.previous_bands {
            safety::clear_pending();
            return Ok(None);
        }
        self.set_bands_direct(&modem.object_path, &pending.previous_bands)?;
        safety::clear_pending();
        Ok(Some(
            "Recovered from an unconfirmed band change by restoring your previous bands.".into(),
        ))
    }

    pub fn apply_persistent_bands(&self, object_path: &str, bands: &[u32]) -> Result<(), String> {
        let modems = self.modems();
        let modem = modems
            .iter()
            .find(|m| m.object_path == object_path)
            .ok_or_else(|| "Modem not found.".to_string())?;
        if bands.is_empty() {
            return Err("Select at least one band.".into());
        }
        let _ = safety::backup_band_state(
            &modem.stable_id(),
            &modem.display_name(),
            &modem.current_bands,
            "Persistent Lock",
        );
        self.set_bands_direct(object_path, bands)?;
        self.invalidate_radio_snapshots(object_path, &modem.device_uid);
        Ok(())
    }

    pub fn set_bands_direct(&self, object_path: &str, bands: &[u32]) -> Result<(), String> {
        block_on(async {
            let proxy = zbus::Proxy::new(&self.conn, MM_BUS, object_path, MODEM_IFACE)
                .await
                .map_err(|e| format!("Could not open modem {object_path}: {e}"))?;
            proxy
                .call::<_, _, ()>("SetCurrentBands", &(bands.to_vec(),))
                .await
                .map_err(|e| format!("ModemManager rejected SetCurrentBands: {e}"))
        })
    }

    pub fn sim_unlock(&self, object_path: &str, pin: &str) -> Result<(), String> {
        let pin = pin.trim();
        if pin.len() < 4 || pin.len() > 8 || !pin.chars().all(|c| c.is_ascii_digit()) {
            return Err("SIM PIN must be 4 to 8 digits.".into());
        }
        block_on(async {
            let modem = zbus::Proxy::new(&self.conn, MM_BUS, object_path, MODEM_IFACE)
                .await
                .map_err(|e| format!("Could not open modem: {e}"))?;
            let sim_path: OwnedObjectPath = modem
                .get_property("Sim")
                .await
                .map_err(|e| format!("No SIM path on modem: {e}"))?;
            if sim_path.as_str().is_empty() || sim_path.as_str() == "/" {
                return Err("No SIM card detected in this modem.".into());
            }
            let sim = zbus::Proxy::new(&self.conn, MM_BUS, sim_path.as_str(), SIM_IFACE)
                .await
                .map_err(|e| format!("Could not open SIM: {e}"))?;
            sim.call::<_, _, ()>("SendPin", &(pin,))
                .await
                .map_err(|e| format!("SIM unlock failed: {e}"))
        })
    }

    pub fn set_sim_pin_enabled(
        &self,
        object_path: &str,
        pin: &str,
        enabled: bool,
    ) -> Result<(), String> {
        let pin = pin.trim();
        if pin.len() < 4 || pin.len() > 8 || !pin.chars().all(|c| c.is_ascii_digit()) {
            return Err("SIM PIN must be 4 to 8 digits.".into());
        }
        block_on(async {
            let modem = zbus::Proxy::new(&self.conn, MM_BUS, object_path, MODEM_IFACE)
                .await
                .map_err(|e| format!("Could not open modem: {e}"))?;
            let sim_path: OwnedObjectPath = modem
                .get_property("Sim")
                .await
                .map_err(|e| format!("No SIM path on modem: {e}"))?;
            let sim = zbus::Proxy::new(&self.conn, MM_BUS, sim_path.as_str(), SIM_IFACE)
                .await
                .map_err(|e| format!("Could not open SIM: {e}"))?;
            sim.call::<_, _, ()>("EnablePin", &(pin, enabled))
                .await
                .map_err(|e| format!("Could not update SIM PIN setting: {e}"))
        })
    }

    pub fn refresh_cell_info(&self, object_path: &str) -> Result<(), String> {
        let entry = block_on(async {
            let proxy = zbus::Proxy::new(&self.conn, MM_BUS, object_path, MODEM_IFACE)
                .await
                .map_err(|e| format!("Could not open modem {object_path}: {e}"))?;
            Ok::<_, String>(
                match proxy
                    .call::<_, _, Vec<HashMap<String, OwnedValue>>>("GetCellInfo", &())
                    .await
                {
                    Ok(resp) => match helper::parse_cell_info_response(&resp) {
                        Ok(cells) => CellCacheEntry {
                            cells,
                            error: String::new(),
                            source: "ModemManager GetCellInfo".into(),
                        },
                        Err(error) => CellCacheEntry {
                            cells: Vec::new(),
                            error,
                            source: "ModemManager GetCellInfo".into(),
                        },
                    },
                    Err(err) => CellCacheEntry {
                        cells: Vec::new(),
                        error: format!("GetCellInfo unavailable: {err}"),
                        source: "ModemManager GetCellInfo".into(),
                    },
                },
            )
        })?;
        // Some desktop launchers run inside a sandbox that cannot call
        // ModemManager's diagnostic method directly. Retry through the existing
        // privileged helper so cell data (including the serving LTE band) can
        // still be read with the user's authorization.
        let entry = if !entry.error.is_empty() {
            let first_privileged_attempt = self
                .cell_privileged_attempts
                .lock()
                .map(|mut attempts| attempts.insert(object_path.to_string()))
                .unwrap_or(false);
            if !first_privileged_attempt {
                // Keep the last successful privileged result and never launch
                // another password prompt on the periodic background refresh.
                if let Some(cached) = self
                    .cell_cache
                    .lock()
                    .ok()
                    .and_then(|cache| cache.get(object_path).cloned())
                {
                    return if cached.error.is_empty() {
                        Ok(())
                    } else {
                        Err(cached.error)
                    };
                }
                return Err(entry.error);
            }
            match helper::privileged_cell_info(object_path) {
                Ok(cells) if !cells.is_empty() => CellCacheEntry {
                    cells,
                    error: String::new(),
                    source: "Privileged ModemManager GetCellInfo".into(),
                },
                Ok(_) => entry,
                Err(helper_error) => CellCacheEntry {
                    error: format!(
                        "{}; privileged fallback failed: {helper_error}",
                        entry.error
                    ),
                    ..entry
                },
            }
        } else {
            entry
        };
        let is_err = !entry.error.is_empty();
        let err_msg = entry.error.clone();
        if let Ok(mut cache) = self.cell_cache.lock() {
            cache.insert(object_path.to_string(), entry);
        }
        if is_err { Err(err_msg) } else { Ok(()) }
    }

    pub fn refresh_vendor_info(&self, object_path: &str) -> Result<(), String> {
        let modems = self.modems();
        let modem = modems
            .iter()
            .find(|m| m.object_path == object_path)
            .ok_or_else(|| "Modem not found.".to_string())?;
        let driver = DriverKind::identify(&modem.manufacturer, &modem.model);
        if driver == DriverKind::Generic {
            return Err("No vendor-specific diagnostic commands for generic modems.".into());
        }
        let info = helper::privileged_vendor_read(
            driver,
            &modem.device_uid,
            &modem.model,
            &modem.at_ports,
        )
        .map(|read| {
            let pairs: Vec<(String, String)> = read
                .responses
                .into_iter()
                .map(|r| (r.command, r.response))
                .collect();
            let mut parsed = driver.parse(&modem.model, &pairs);
            if !read.warning.is_empty() {
                parsed.error = read.warning;
            }
            parsed
        })?;
        let key = if modem.device_uid.is_empty() {
            object_path.to_string()
        } else {
            modem.device_uid.clone()
        };
        if let Ok(mut cache) = self.vendor_cache.lock() {
            cache.insert(key, info);
        }
        Ok(())
    }

    pub fn cell_lock_preview(
        &self,
        object_path: &str,
        earfcn: u32,
        pci: u32,
    ) -> Result<String, String> {
        let modems = self.modems();
        let modem = modems
            .iter()
            .find(|m| m.object_path == object_path)
            .ok_or_else(|| "Modem not found.".to_string())?;
        let driver = DriverKind::identify(&modem.manufacturer, &modem.model);
        let _ = safety::backup_cell_intent(
            &modem.stable_id(),
            object_path,
            &modem.current_bands,
            &format!("EARFCN={earfcn}, PCI={pci}"),
        );
        driver.cell_lock_preview(earfcn, pci)
    }

    pub fn invalidate_radio_snapshots(&self, object_path: &str, device_uid: &str) {
        if let Ok(mut cache) = self.signal_cache.lock() {
            cache.remove(object_path);
        }
        if let Ok(mut cache) = self.cell_cache.lock() {
            cache.remove(object_path);
        }
        if !device_uid.is_empty()
            && let Ok(mut cache) = self.vendor_cache.lock()
        {
            cache.remove(device_uid);
        }
    }

    pub fn apply_band_selection(
        &self,
        object_path: &str,
        bands: &[u32],
        profile_name: Option<String>,
    ) -> Result<u64, String> {
        if bands.is_empty() {
            return Err("Select at least one band.".into());
        }
        let modems = self.modems();
        let modem = modems
            .iter()
            .find(|m| m.object_path == object_path)
            .ok_or_else(|| "Modem not found.".to_string())?;
        let previous = modem.current_bands.clone();
        let _ = safety::backup_band_state(
            &modem.stable_id(),
            &modem.display_name(),
            &previous,
            profile_name.as_deref().unwrap_or("Manual selection"),
        );
        let generation = now_unix();
        let token = format!("guard-{generation}");
        let tx = safety::BandTransaction {
            object_path: object_path.to_string(),
            device_uid: modem.device_uid.clone(),
            previous_bands: previous.clone(),
            requested_bands: bands.to_vec(),
            previous_profile_name: String::new(),
            requested_profile_name: profile_name.clone(),
            guard_token: token.clone(),
            created_unix: generation,
        };
        let _ = safety::write_pending(&tx);
        self.set_bands_direct(object_path, bands)?;
        self.invalidate_radio_snapshots(object_path, &modem.device_uid);
        let _ = safety::spawn_rollback_guard(47, &token);

        if let Ok(mut cache) = self.band_change_cache.lock() {
            cache.insert(
                object_path.to_string(),
                BandChangeState {
                    previous_bands: previous,
                    requested_bands: bands.to_vec(),
                    pending_profile_name: profile_name,
                    pending_confirmation: true,
                    generation,
                    message: "Band change applied. Confirm within 45 seconds or it will roll back."
                        .into(),
                },
            );
        }
        Ok(generation)
    }

    pub fn confirm_band_selection(&self, object_path: &str) {
        safety::clear_pending();
        if let Ok(mut cache) = self.band_change_cache.lock()
            && let Some(state) = cache.get_mut(object_path)
        {
            state.pending_confirmation = false;
            state.message = "Band selection confirmed.".into();
            if let Some(ref name) = state.pending_profile_name {
                let _ = profiles::set_active(name);
            }
        }
    }

    pub fn band_change_pending(&self, object_path: &str) -> bool {
        self.band_change_cache
            .lock()
            .ok()
            .and_then(|c| c.get(object_path).map(|s| s.pending_confirmation))
            .unwrap_or(false)
    }

    pub fn band_change_generation(&self, object_path: &str) -> Option<u64> {
        self.band_change_cache
            .lock()
            .ok()
            .and_then(|c| c.get(object_path).map(|s| s.generation))
    }

    pub fn rollback_band_selection(&self, object_path: &str) -> Result<(), String> {
        let prev = self
            .band_change_cache
            .lock()
            .ok()
            .and_then(|c| c.get(object_path).map(|s| s.previous_bands.clone()))
            .filter(|b| !b.is_empty())
            .or_else(|| safety::read_pending().map(|p| p.previous_bands))
            .unwrap_or_else(|| vec![256]);
        self.set_bands_direct(object_path, &prev)?;
        safety::clear_pending();
        if let Ok(mut cache) = self.band_change_cache.lock()
            && let Some(state) = cache.get_mut(object_path)
        {
            state.pending_confirmation = false;
            state.message = "Rolled back to previous bands.".into();
        }
        Ok(())
    }

    pub fn send_at_command(&self, object_path: &str, command: &str) -> Result<String, String> {
        block_on(async {
            let proxy = zbus::Proxy::new(&self.conn, MM_BUS, object_path, MODEM_IFACE)
                .await
                .map_err(|e| format!("Could not open modem: {e}"))?;
            proxy
                .call::<_, _, String>("Command", &(command, 5u32))
                .await
                .map_err(|e| format!("AT command rejected: {e}"))
        })
    }

    fn resolve_sim_identity(
        &self,
        sim_path: &str,
        slot: u32,
        operator_code: &str,
        operator_name: &str,
    ) -> SimIdentity {
        if sim_path.is_empty() || sim_path == "/" {
            return SimIdentity::default();
        }
        if let Ok(cache) = self.sim_cache.lock()
            && let Some(cached) = cache.get(sim_path)
            && cached.operator_id == operator_code
            && cached.operator_name == operator_name
        {
            return cached.clone();
        }
        let sim_identifier = block_on(async {
            let Ok(p) = zbus::Proxy::new(&self.conn, MM_BUS, sim_path, SIM_IFACE).await else {
                return String::new();
            };
            let iccid = p
                .get_property::<String>("SimIdentifier")
                .await
                .unwrap_or_default();
            if !iccid.is_empty() {
                iccid
            } else {
                p.get_property::<String>("Imsi").await.unwrap_or_default()
            }
        });

        let identity = SimIdentity::from_raw(
            sim_path,
            slot,
            operator_code,
            operator_name,
            &sim_identifier,
        );
        if let Ok(mut cache) = self.sim_cache.lock() {
            cache.insert(sim_path.to_string(), identity.clone());
        }
        identity
    }

    fn read_modem(
        &self,
        object_path: &str,
        ifaces: &HashMap<String, HashMap<String, OwnedValue>>,
    ) -> Option<ModemInfo> {
        let modem = ifaces.get(MODEM_IFACE)?;
        let manufacturer = prop_string(modem, "Manufacturer");
        let model = prop_string(modem, "Model");
        let revision = prop_string(modem, "Revision");
        let hardware_revision = prop_string(modem, "HardwareRevision");
        let equipment_id = prop_string(modem, "EquipmentIdentifier");
        let device_uid = prop_string(modem, "Device");
        let plugin = prop_string(modem, "Plugin");
        let primary_port = prop_string(modem, "PrimaryPort");
        let at_ports = ports_by_kind(modem, 3);
        let net_ports = ports_by_kind(modem, 2);
        let mbim_ports = ports_by_kind(modem, 6);
        let qmi_ports = ports_by_kind(modem, 5);
        let sim_path = prop_obj_path(modem, "Sim");
        let unlock_required = prop_u32(modem, "UnlockRequired").unwrap_or(0);
        let primary_sim_slot = prop_u32(modem, "PrimarySimSlot").unwrap_or(0);
        let state = prop_i32(modem, "State").unwrap_or(0);
        let current_capabilities = prop_u32(modem, "CurrentCapabilities").unwrap_or(0);
        let access_technologies = prop_u32(modem, "AccessTechnologies").unwrap_or(0);
        let (signal_percent, signal_recent) = prop_signal_quality(modem);
        let supported_bands = prop_vec_u32(modem, "SupportedBands");
        let current_bands = prop_vec_u32(modem, "CurrentBands");

        let (operator_name, operator_code, reg_state) = ifaces
            .get(MODEM_3GPP_IFACE)
            .map(|p| {
                (
                    prop_string(p, "OperatorName"),
                    prop_string(p, "OperatorCode"),
                    prop_u32(p, "RegistrationState").unwrap_or(0),
                )
            })
            .unwrap_or_default();
        let roaming = reg_state == 5;

        let (sim_op_id, sim_spn, sim_gid1, sim_eid) = if !sim_path.is_empty() && sim_path != "/" {
            block_on(async {
                let Ok(p) =
                    zbus::Proxy::new(&self.conn, MM_BUS, sim_path.as_str(), SIM_IFACE).await
                else {
                    return Default::default();
                };
                let gid1 = p
                    .get_property::<Vec<u8>>("Gid1")
                    .await
                    .map(|bytes| bytes.iter().map(|b| format!("{b:02x}")).collect())
                    .unwrap_or_default();
                (
                    p.get_property::<String>("OperatorIdentifier")
                        .await
                        .unwrap_or_default(),
                    p.get_property::<String>("OperatorName")
                        .await
                        .unwrap_or_default(),
                    gid1,
                    p.get_property::<String>("Eid").await.unwrap_or_default(),
                )
            })
        } else {
            Default::default()
        };

        let sim_identity =
            self.resolve_sim_identity(&sim_path, primary_sim_slot, &operator_code, &operator_name);

        let (lte_signal, nr5g_signal) = ifaces
            .get(MODEM_SIGNAL_IFACE)
            .map(|p| (prop_radio_signal(p, "Lte"), prop_radio_signal(p, "Nr5g")))
            .unwrap_or_default();

        let now = now_unix();
        let context_key = format!(
            "{}:{}:{:?}",
            sim_identity.stable_hash, operator_code, current_bands
        );

        let (lte_snapshot, nr5g_snapshot) = if let Ok(mut cache) = self.signal_cache.lock() {
            let entry = cache.entry(object_path.to_string()).or_default();
            if entry.context_key != context_key {
                entry.lte = SignalSnapshot::default();
                entry.nr5g = SignalSnapshot::default();
                entry.context_key = context_key.clone();
            }
            if lte_signal.has_any() {
                if !signal_eq(&entry.lte.signal, &lte_signal)
                    || entry.lte.updated_unix == 0
                    || signal_recent
                {
                    entry.lte.signal = lte_signal.clone();
                    entry.lte.updated_unix = now;
                    entry.lte.context_key = context_key.clone();
                }
                entry.lte.freshness = entry.lte.compute_freshness(now, 15, 60);
            } else {
                entry.lte = SignalSnapshot::default();
            }
            if nr5g_signal.has_any() {
                if !signal_eq(&entry.nr5g.signal, &nr5g_signal)
                    || entry.nr5g.updated_unix == 0
                    || signal_recent
                {
                    entry.nr5g.signal = nr5g_signal.clone();
                    entry.nr5g.updated_unix = now;
                    entry.nr5g.context_key = context_key.clone();
                }
                entry.nr5g.freshness = entry.nr5g.compute_freshness(now, 15, 60);
            } else {
                entry.nr5g = SignalSnapshot::default();
            }
            (entry.lte.clone(), entry.nr5g.clone())
        } else {
            Default::default()
        };

        let cell_entry = self
            .cell_cache
            .lock()
            .ok()
            .and_then(|c| c.get(object_path).cloned())
            .unwrap_or_default();

        let driver = DriverKind::identify(&manufacturer, &model);
        let vendor_key = if device_uid.trim().is_empty() {
            object_path
        } else {
            &device_uid
        };
        let vendor = self
            .vendor_cache
            .lock()
            .ok()
            .and_then(|c| c.get(vendor_key).cloned())
            .unwrap_or_else(|| VendorInfo {
                driver_name: driver.label().into(),
                deep_read_available: driver.has_vendor_deep_read(),
                ..VendorInfo::default()
            });
        let band_change = self
            .band_change_cache
            .lock()
            .ok()
            .and_then(|c| c.get(object_path).cloned())
            .unwrap_or_default();

        let sysfs = crate::hardware::inspect_sysfs_hardware(&net_ports, &primary_port);
        let gps_supported = ifaces.contains_key(MODEM_LOCATION_IFACE);
        let esim_supported = (!sim_eid.is_empty()).then_some(true);
        let fcc_locked = (state == 3 && unlock_required == 1).then_some(true);

        Some(ModemInfo {
            object_path: object_path.to_string(),
            manufacturer,
            model,
            revision,
            hardware_revision,
            equipment_id,
            device_uid,
            plugin,
            kernel_driver: sysfs.kernel_driver,
            usb_vid: sysfs.usb_vid,
            usb_pid: sysfs.usb_pid,
            pci_vid: sysfs.pci_vid,
            pci_pid: sysfs.pci_pid,
            primary_port,
            at_ports,
            mbim_ports,
            qmi_ports,
            net_ports,
            sim_path,
            sim_identity,
            unlock_required,
            primary_sim_slot,
            esim_supported,
            eid: sim_eid,
            fcc_locked,
            operator_name: if !sim_spn.is_empty() {
                sim_spn.clone()
            } else {
                operator_name.clone()
            },
            operator_code: if !sim_op_id.is_empty() {
                sim_op_id
            } else {
                operator_code.clone()
            },
            spn: sim_spn,
            gid1: sim_gid1,
            registered_operator_name: operator_name,
            registered_operator_code: operator_code,
            roaming,
            state,
            current_capabilities,
            access_technologies,
            gps_supported,
            signal_percent: signal_percent.min(100),
            signal_recent,
            supported_bands,
            current_bands,
            lte_signal,
            nr5g_signal,
            lte_snapshot,
            nr5g_snapshot,
            serving_cells: cell_entry.cells,
            cell_info_error: cell_entry.error,
            cell_info_source: cell_entry.source,
            vendor,
            band_change,
            ..ModemInfo::default()
        })
    }
}

fn prop_string(map: &HashMap<String, OwnedValue>, key: &str) -> String {
    map.get(key)
        .and_then(|v| <&str>::try_from(v).ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn prop_obj_path(map: &HashMap<String, OwnedValue>, key: &str) -> String {
    map.get(key)
        .and_then(|v| OwnedObjectPath::try_from(v.clone()).ok())
        .map(|p| p.to_string())
        .unwrap_or_default()
}

fn prop_u32(map: &HashMap<String, OwnedValue>, key: &str) -> Option<u32> {
    map.get(key).and_then(|v| u32::try_from(v).ok())
}

fn prop_i32(map: &HashMap<String, OwnedValue>, key: &str) -> Option<i32> {
    map.get(key).and_then(|v| i32::try_from(v).ok())
}

fn prop_vec_u32(map: &HashMap<String, OwnedValue>, key: &str) -> Vec<u32> {
    map.get(key)
        .and_then(|v| Vec::<u32>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

fn prop_signal_quality(map: &HashMap<String, OwnedValue>) -> (u32, bool) {
    map.get("SignalQuality")
        .and_then(|v| <(u32, bool)>::try_from(v.clone()).ok())
        .unwrap_or((0, false))
}

fn ports_by_kind(map: &HashMap<String, OwnedValue>, target_kind: u32) -> Vec<String> {
    map.get("Ports")
        .and_then(|v| Vec::<(String, u32)>::try_from(v.clone()).ok())
        .map(|items| {
            items
                .into_iter()
                .filter_map(|(name, kind)| (kind == target_kind).then_some(name))
                .collect()
        })
        .unwrap_or_default()
}

fn prop_radio_signal(map: &HashMap<String, OwnedValue>, key: &str) -> RadioSignal {
    let Some(val) = map.get(key) else {
        return RadioSignal::default();
    };
    let Ok(dict) = HashMap::<String, OwnedValue>::try_from(val.clone()) else {
        return RadioSignal::default();
    };
    RadioSignal {
        rssi: dict.get("rssi").and_then(|v| f64::try_from(v).ok()),
        rsrp: dict.get("rsrp").and_then(|v| f64::try_from(v).ok()),
        rsrq: dict.get("rsrq").and_then(|v| f64::try_from(v).ok()),
        snr: dict.get("snr").and_then(|v| f64::try_from(v).ok()),
    }
}

fn signal_eq(a: &RadioSignal, b: &RadioSignal) -> bool {
    a.rsrp == b.rsrp && a.rsrq == b.rsrq && a.snr == b.snr && a.rssi == b.rssi
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
