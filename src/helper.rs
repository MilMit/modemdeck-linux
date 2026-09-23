use std::{
    process::Command,
    time::{Duration, Instant},
};

use gio::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    drivers::DriverKind,
    model::{lte_band_from_earfcn, ServingCellInfo},
};

const MM_BUS: &str = "org.freedesktop.ModemManager1";
const MM_ROOT: &str = "/org/freedesktop/ModemManager1";
const MM_MANAGER_IFACE: &str = "org.freedesktop.ModemManager1";
const MODEM_IFACE: &str = "org.freedesktop.ModemManager1.Modem";

#[derive(Debug, Serialize, Deserialize)]
struct HelperEnvelope<T> { ok: bool, data: Option<T>, error: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtResponse { pub command: String, pub response: String }
#[derive(Debug, Serialize, Deserialize)]
pub struct DirectVendorRead { pub port: String, pub responses: Vec<AtResponse>, pub warning: String }

pub fn maybe_run(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("--helper") { return None; }
    let action = args.get(2).map(String::as_str).unwrap_or("");
    let result = match action {
        "cell-info" => {
            let Some(object_path) = args.get(3) else { print_json_error::<Vec<ServingCellInfo>>("Missing modem object path."); return Some(2); };
            match root_cell_info(object_path) { Ok(cells) => { print_json_ok(cells); 0 }, Err(error) => { print_json_error::<Vec<ServingCellInfo>>(&error); 1 } }
        }
        "vendor-read" => {
            let Some(driver_key) = args.get(3) else { print_json_error::<DirectVendorRead>("Missing driver key."); return Some(2); };
            let Some(uid) = args.get(4) else { print_json_error::<DirectVendorRead>("Missing modem device UID."); return Some(2); };
            let Some(model) = args.get(5) else { print_json_error::<DirectVendorRead>("Missing modem model."); return Some(2); };
            let Some(driver) = DriverKind::from_key(driver_key) else { print_json_error::<DirectVendorRead>("Unknown driver key."); return Some(2); };
            if driver == DriverKind::Generic { print_json_error::<DirectVendorRead>("The generic driver has no privileged AT profile."); return Some(2); }
            let ports: Vec<String> = args.iter().skip(6).cloned().collect();
            match root_direct_vendor_read(uid, &ports, driver, model) { Ok(read) => { print_json_ok(read); 0 }, Err(error) => { print_json_error::<DirectVendorRead>(&error); 1 } }
        }
        _ => { print_json_error::<String>("Unknown or missing helper action."); 2 }
    };
    Some(result)
}

pub fn privileged_cell_info(object_path: &str) -> Result<Vec<ServingCellInfo>, String> {
    let stdout = run_pkexec(&["cell-info", object_path])?; parse_envelope::<Vec<ServingCellInfo>>(&stdout)
}

pub fn privileged_vendor_read(driver: DriverKind, uid: &str, model: &str, ports: &[String]) -> Result<DirectVendorRead, String> {
    if uid.trim().is_empty() { return Err("ModemManager did not report a device UID, so the modem cannot be safely inhibited.".into()); }
    if ports.is_empty() { return Err("No AT serial port was reported by ModemManager for this modem.".into()); }
    if !driver.has_vendor_deep_read() { return Err("No vendor AT read profile is available for this modem.".into()); }
    let mut args: Vec<&str> = vec!["vendor-read", driver.key(), uid, model];
    for port in ports { args.push(port.as_str()); }
    let stdout = run_pkexec(&args)?; parse_envelope::<DirectVendorRead>(&stdout)
}

fn run_pkexec(args: &[&str]) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|error| format!("Could not locate the ModemDeck executable: {error}"))?;
    let output = Command::new("pkexec").arg(exe).arg("--helper").args(args).output().map_err(|error| if error.kind() == std::io::ErrorKind::NotFound { "pkexec is not installed. Install PolicyKit/pkexec and try again.".to_string() } else { format!("Could not start the privileged helper: {error}") })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if matches!(output.status.code(), Some(126) | Some(127)) { return Err("Administrator authorization was cancelled or denied.".into()); }
        if !stdout.is_empty() { if let Ok(envelope) = serde_json::from_str::<HelperEnvelope<serde_json::Value>>(&stdout) { if !envelope.error.is_empty() { return Err(envelope.error); } } }
        if stderr.is_empty() { Err(format!("Privileged helper exited with status {}.", output.status)) } else { Err(format!("Privileged helper failed: {stderr}")) }
    } else { Ok(String::from_utf8_lossy(&output.stdout).to_string()) }
}

fn parse_envelope<T>(raw: &str) -> Result<T, String> where T: for<'de> Deserialize<'de> {
    let envelope: HelperEnvelope<T> = serde_json::from_str(raw.trim()).map_err(|error| format!("Invalid response from privileged helper: {error}"))?;
    if !envelope.ok { return Err(if envelope.error.is_empty() { "Privileged helper reported an unknown error.".into() } else { envelope.error }); }
    envelope.data.ok_or_else(|| "Privileged helper returned no data.".to_string())
}

fn print_json_ok<T: Serialize>(data: T) { let envelope = HelperEnvelope { ok: true, data: Some(data), error: String::new() }; println!("{}", serde_json::to_string(&envelope).unwrap_or_else(|_| "{\"ok\":false,\"data\":null,\"error\":\"serialization failed\"}".into())); }
fn print_json_error<T: Serialize>(error: &str) { let envelope: HelperEnvelope<T> = HelperEnvelope { ok: false, data: None, error: error.to_string() }; println!("{}", serde_json::to_string(&envelope).unwrap_or_else(|_| "{\"ok\":false,\"data\":null,\"error\":\"serialization failed\"}".into())); }

fn manager_proxy() -> Result<gio::DBusProxy, String> {
    gio::DBusProxy::for_bus_sync(gio::BusType::System, gio::DBusProxyFlags::NONE, None::<&gio::DBusInterfaceInfo>, MM_BUS, MM_ROOT, MM_MANAGER_IFACE, None::<&gio::Cancellable>).map_err(|error| format!("Could not connect to ModemManager manager interface: {error}"))
}
fn modem_proxy(object_path: &str) -> Result<gio::DBusProxy, String> {
    gio::DBusProxy::for_bus_sync(gio::BusType::System, gio::DBusProxyFlags::NONE, None::<&gio::DBusInterfaceInfo>, MM_BUS, object_path, MODEM_IFACE, None::<&gio::Cancellable>).map_err(|error| format!("Could not connect to modem {object_path}: {error}"))
}
fn root_cell_info(object_path: &str) -> Result<Vec<ServingCellInfo>, String> {
    let modem = modem_proxy(object_path)?;
    let response = modem.call_sync("GetCellInfo", None, gio::DBusCallFlags::NONE, 10_000, None::<&gio::Cancellable>).map_err(|error| format!("GetCellInfo failed even with administrator privileges: {error}"))?;
    parse_cell_info_response(&response)
}

pub fn parse_cell_info_response(response: &glib::Variant) -> Result<Vec<ServingCellInfo>, String> {
    if response.n_children() == 0 { return Err("ModemManager returned an empty GetCellInfo response.".into()); }
    let array = response.child_value(0); let mut cells = Vec::new();
    for index in 0..array.n_children() {
        let cell_variant = array.child_value(index); let dict = glib::VariantDict::new(Some(&cell_variant));
        let serving = dict.lookup::<bool>("serving").ok().flatten().unwrap_or(false); if !serving { continue; }
        let cell_type = dict.lookup::<u32>("cell-type").ok().flatten().unwrap_or(0);
        let earfcn = dict.lookup::<u32>("earfcn").ok().flatten(); let nrarfcn = dict.lookup::<u32>("nrarfcn").ok().flatten();
        let band = match cell_type { 5 => earfcn.and_then(lte_band_from_earfcn), _ => None };
        cells.push(ServingCellInfo { cell_type, serving_cell_type: dict.lookup::<u32>("serving-cell-type").ok().flatten(), operator_id: dict_string(&dict, "operator-id"), tac: dict_string(&dict, "tac"), ci: dict_string(&dict, "ci"), physical_ci: dict_string_or_u32_hex(&dict, "physical-ci"), earfcn, nrarfcn, band, bandwidth: dict.lookup::<u32>("bandwidth").ok().flatten(), rsrp: dict.lookup::<f64>("rsrp").ok().flatten(), rsrq: dict.lookup::<f64>("rsrq").ok().flatten(), sinr: dict.lookup::<f64>("sinr").ok().flatten() });
    }
    if cells.is_empty() { Err("GetCellInfo succeeded, but no serving cell was reported by this modem/driver.".into()) } else { Ok(cells) }
}

fn root_direct_vendor_read(uid: &str, ports: &[String], driver: DriverKind, model: &str) -> Result<DirectVendorRead, String> {
    let commands = driver.read_commands(model); if commands.is_empty() { return Err("This driver has no direct AT read profile.".into()); }
    let manager = manager_proxy()?; let inhibit_params = (uid, true).to_variant();
    manager.call_sync("InhibitDevice", Some(&inhibit_params), gio::DBusCallFlags::NONE, 10_000, None::<&gio::Cancellable>).map_err(|error| format!("Could not inhibit the modem safely: {error}"))?;
    std::thread::sleep(Duration::from_millis(250)); let read_result = try_at_ports(ports, &commands);
    let uninhibit_params = (uid, false).to_variant(); let uninhibit_result = manager.call_sync("InhibitDevice", Some(&uninhibit_params), gio::DBusCallFlags::NONE, 10_000, None::<&gio::Cancellable>);
    match (read_result, uninhibit_result) {
        (Ok(read), Ok(_)) => Ok(read),
        (Ok(mut read), Err(error)) => { read.warning = format!("The read completed, but explicit uninhibit returned: {error}. ModemManager also removes inhibition automatically when this helper exits."); Ok(read) },
        (Err(read_error), Ok(_)) => Err(read_error),
        (Err(read_error), Err(error)) => Err(format!("{read_error}; additionally, explicit uninhibit failed: {error}")),
    }
}

fn try_at_ports(ports: &[String], commands: &[&str]) -> Result<DirectVendorRead, String> {
    let mut failures = Vec::new();
    for name in ports {
        let path = if name.starts_with('/') { name.clone() } else { format!("/dev/{name}") };
        match read_vendor_port(&path, commands) { Ok(responses) => return Ok(DirectVendorRead { port: path, responses, warning: String::new() }), Err(error) => failures.push(format!("{path}: {error}")) }
    }
    Err(format!("No reported AT port answered the read-only {} profile. {}", commands.first().copied().unwrap_or("AT"), failures.join(" | ")))
}

fn read_vendor_port(path: &str, commands: &[&str]) -> Result<Vec<AtResponse>, String> {
    let mut port = serialport::new(path, 115_200).timeout(Duration::from_millis(180)).data_bits(serialport::DataBits::Eight).flow_control(serialport::FlowControl::None).parity(serialport::Parity::None).stop_bits(serialport::StopBits::One).open().map_err(|error| format!("open failed: {error}"))?;
    let probe = send_serial_at(&mut *port, "AT", Duration::from_secs(2))?; if !response_ok(&probe) { return Err(format!("AT probe did not return OK: {}", compact_response(&probe))); }
    let mut responses = Vec::new(); let mut first_command_succeeded = false;
    for (index, command) in commands.iter().enumerate() {
        match send_serial_at(&mut *port, command, Duration::from_secs(5)) {
            Ok(value) if response_ok(&value) => { if index == 0 { first_command_succeeded = true; } responses.push(AtResponse { command: (*command).to_string(), response: clean_at_response(&value, command) }); },
            Ok(value) => responses.push(AtResponse { command: (*command).to_string(), response: format!("Command unavailable: {}", compact_response(&value)) }),
            Err(error) => responses.push(AtResponse { command: (*command).to_string(), response: format!("Command unavailable: {error}") }),
        }
    }
    if !first_command_succeeded { return Err(format!("The AT port answered, but the driver's primary read command ({}) was rejected.", commands.first().copied().unwrap_or("unknown"))); }
    Ok(responses)
}

fn send_serial_at(port: &mut dyn serialport::SerialPort, command: &str, timeout: Duration) -> Result<String, String> {
    let _ = port.clear(serialport::ClearBuffer::All); port.write_all(format!("{command}\r").as_bytes()).map_err(|error| format!("write failed: {error}"))?; port.flush().map_err(|error| format!("flush failed: {error}"))?;
    let start = Instant::now(); let mut bytes = Vec::new(); let mut buffer = [0u8; 1024];
    while start.elapsed() < timeout {
        match port.read(&mut buffer) {
            Ok(0) => {}, Ok(count) => { bytes.extend_from_slice(&buffer[..count]); let text = String::from_utf8_lossy(&bytes); if text.contains("\r\nOK\r\n") || text.contains("\nOK\n") || text.contains("\r\nERROR\r\n") || text.contains("+CME ERROR") || text.contains("+CMS ERROR") { break; } },
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}, Err(error) => return Err(format!("read failed: {error}")),
        }
    }
    if bytes.is_empty() { return Err(format!("timeout waiting for a response to {command}")); }
    Ok(String::from_utf8_lossy(&bytes).replace('\0', ""))
}

fn response_ok(response: &str) -> bool { let normalized = response.replace('\r', ""); normalized.lines().any(|line| line.trim() == "OK") }
fn clean_at_response(raw: &str, command: &str) -> String { raw.replace('\r', "").lines().map(str::trim).filter(|line| !line.is_empty()).filter(|line| *line != command).filter(|line| *line != "OK").collect::<Vec<_>>().join("\n") }
fn compact_response(raw: &str) -> String { raw.replace('\r', " ").replace('\n', " ").split_whitespace().take(24).collect::<Vec<_>>().join(" ") }
fn dict_string(dict: &glib::VariantDict, key: &str) -> String { dict.lookup::<String>(key).ok().flatten().unwrap_or_default() }
fn dict_string_or_u32_hex(dict: &glib::VariantDict, key: &str) -> String { if let Some(value) = dict.lookup::<String>(key).ok().flatten() { return value; } if let Some(value) = dict.lookup::<u32>(key).ok().flatten() { return format!("{value:x}"); } String::new() }
