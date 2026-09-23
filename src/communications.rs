use gio::prelude::*;
use glib::variant::ObjectPath;

const MM_BUS: &str = "org.freedesktop.ModemManager1";
const MESSAGING_IFACE: &str = "org.freedesktop.ModemManager1.Modem.Messaging";
const SMS_IFACE: &str = "org.freedesktop.ModemManager1.Sms";
const VOICE_IFACE: &str = "org.freedesktop.ModemManager1.Modem.Voice";
const CALL_IFACE: &str = "org.freedesktop.ModemManager1.Call";
const USSD_IFACE: &str = "org.freedesktop.ModemManager1.Modem.Modem3gpp.Ussd";

#[derive(Debug, Clone, Default)]
pub struct SmsMessage {
    pub path: String,
    pub number: String,
    pub text: String,
    pub timestamp: String,
    pub state: u32,
}

impl SmsMessage {
    pub fn state_label(&self) -> &'static str {
        match self.state {
            1 => "Stored",
            2 => "Receiving",
            3 => "Received",
            4 => "Sending",
            5 => "Sent",
            _ => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CallInfo {
    pub path: String,
    pub number: String,
    pub state: i32,
    pub direction: i32,
    pub audio_port: String,
}

impl CallInfo {
    pub fn state_label(&self) -> &'static str {
        match self.state {
            1 => "Dialing",
            2 => "Ringing out",
            3 => "Ringing in",
            4 => "Active",
            5 => "Held",
            6 => "Waiting",
            7 => "Terminated",
            _ => "Unknown",
        }
    }

    pub fn direction_label(&self) -> &'static str {
        match self.direction {
            1 => "Incoming",
            2 => "Outgoing",
            _ => "Unknown",
        }
    }

    pub fn can_answer(&self) -> bool { self.state == 3 || self.state == 6 }
    pub fn can_hangup(&self) -> bool { self.state != 7 }
}

fn proxy(path: &str, interface: &str) -> Result<gio::DBusProxy, String> {
    gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::NONE,
        None::<&gio::DBusInterfaceInfo>,
        MM_BUS,
        path,
        interface,
        None::<&gio::Cancellable>,
    )
    .map_err(|error| format!("Could not open {interface}: {error}"))
}

pub fn list_sms(modem_path: &str) -> Result<Vec<SmsMessage>, String> {
    let messaging = proxy(modem_path, MESSAGING_IFACE)?;
    let response = messaging
        .call_sync("List", None, gio::DBusCallFlags::NONE, 10_000, None::<&gio::Cancellable>)
        .map_err(|error| format!("Could not list SMS messages: {error}"))?;
    if response.n_children() == 0 { return Ok(Vec::new()); }
    let paths = response.child_value(0);
    let mut messages = Vec::new();
    for index in 0..paths.n_children() {
        let Some(path) = paths.child_value(index).get::<ObjectPath>() else { continue; };
        let path = path.as_str().to_string();
        let sms = proxy(&path, SMS_IFACE)?;
        messages.push(SmsMessage {
            path,
            number: string_prop(&sms, "Number"),
            text: string_prop(&sms, "Text"),
            timestamp: string_prop(&sms, "Timestamp"),
            state: u32_prop(&sms, "State"),
        });
    }
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(messages)
}

pub fn send_sms(modem_path: &str, number: &str, text: &str) -> Result<(), String> {
    let number = number.trim();
    let text = text.trim();
    if number.is_empty() { return Err("Enter a destination number.".into()); }
    if text.is_empty() { return Err("Message text is empty.".into()); }

    let messaging = proxy(modem_path, MESSAGING_IFACE)?;
    let properties = glib::VariantDict::new(None);
    properties.insert("number", number.to_string());
    properties.insert("text", text.to_string());
    let params = glib::Variant::tuple_from_iter([properties.end()]);
    let response = messaging
        .call_sync("Create", Some(&params), gio::DBusCallFlags::NONE, 15_000, None::<&gio::Cancellable>)
        .map_err(|error| format!("Could not create SMS: {error}"))?;
    if response.n_children() == 0 { return Err("ModemManager created no SMS object.".into()); }
    let path = response.child_value(0).get::<ObjectPath>()
        .ok_or_else(|| "ModemManager returned an invalid SMS object path.".to_string())?;
    let sms = proxy(path.as_str(), SMS_IFACE)?;
    sms.call_sync("Send", None, gio::DBusCallFlags::NONE, 60_000, None::<&gio::Cancellable>)
        .map(|_| ())
        .map_err(|error| format!("SMS send failed: {error}"))
}

pub fn delete_sms(modem_path: &str, sms_path: &str) -> Result<(), String> {
    let path = ObjectPath::try_from(sms_path)
        .map_err(|_| "Invalid SMS object path.".to_string())?;
    let messaging = proxy(modem_path, MESSAGING_IFACE)?;
    messaging
        .call_sync("Delete", Some(&(path,).to_variant()), gio::DBusCallFlags::NONE, 15_000, None::<&gio::Cancellable>)
        .map(|_| ())
        .map_err(|error| format!("Could not delete SMS: {error}"))
}

pub fn list_calls(modem_path: &str) -> Result<Vec<CallInfo>, String> {
    let voice = proxy(modem_path, VOICE_IFACE)?;
    let response = voice
        .call_sync("ListCalls", None, gio::DBusCallFlags::NONE, 10_000, None::<&gio::Cancellable>)
        .map_err(|error| format!("Could not list calls: {error}"))?;
    if response.n_children() == 0 { return Ok(Vec::new()); }
    let paths = response.child_value(0);
    let mut calls = Vec::new();
    for index in 0..paths.n_children() {
        let Some(path) = paths.child_value(index).get::<ObjectPath>() else { continue; };
        let path = path.as_str().to_string();
        let call = proxy(&path, CALL_IFACE)?;
        calls.push(CallInfo {
            path,
            number: string_prop(&call, "Number"),
            state: i32_prop(&call, "State"),
            direction: i32_prop(&call, "Direction"),
            audio_port: string_prop(&call, "AudioPort"),
        });
    }
    Ok(calls)
}

pub fn dial(modem_path: &str, number: &str) -> Result<(), String> {
    let number = number.trim();
    if number.is_empty() { return Err("Enter a phone number.".into()); }
    let voice = proxy(modem_path, VOICE_IFACE)?;
    let properties = glib::VariantDict::new(None);
    properties.insert("number", number.to_string());
    let params = glib::Variant::tuple_from_iter([properties.end()]);
    let response = voice
        .call_sync("CreateCall", Some(&params), gio::DBusCallFlags::NONE, 15_000, None::<&gio::Cancellable>)
        .map_err(|error| format!("Could not create call: {error}"))?;
    if response.n_children() == 0 { return Err("ModemManager created no call object.".into()); }
    let path = response.child_value(0).get::<ObjectPath>()
        .ok_or_else(|| "ModemManager returned an invalid call object path.".to_string())?;
    proxy(path.as_str(), CALL_IFACE)?
        .call_sync("Start", None, gio::DBusCallFlags::NONE, 60_000, None::<&gio::Cancellable>)
        .map(|_| ())
        .map_err(|error| format!("Could not start call: {error}"))
}

pub fn call_action(call_path: &str, action: &str) -> Result<(), String> {
    match action {
        "Accept" | "Hangup" => {},
        _ => return Err("Unsupported call action.".into()),
    }
    proxy(call_path, CALL_IFACE)?
        .call_sync(action, None, gio::DBusCallFlags::NONE, 30_000, None::<&gio::Cancellable>)
        .map(|_| ())
        .map_err(|error| format!("Call {action} failed: {error}"))
}

pub fn initiate_ussd(modem_path: &str, command: &str) -> Result<String, String> {
    let command = command.trim();
    if command.is_empty() { return Err("Enter a USSD code.".into()); }
    let ussd = proxy(modem_path, USSD_IFACE)?;
    let response = ussd
        .call_sync("Initiate", Some(&(command,).to_variant()), gio::DBusCallFlags::NONE, 60_000, None::<&gio::Cancellable>)
        .map_err(|error| format!("USSD request failed: {error}"))?;
    Ok(response.child_value(0).get::<String>().unwrap_or_default())
}

pub fn respond_ussd(modem_path: &str, response_text: &str) -> Result<String, String> {
    let response_text = response_text.trim();
    if response_text.is_empty() { return Err("Enter a USSD response.".into()); }
    let ussd = proxy(modem_path, USSD_IFACE)?;
    let response = ussd
        .call_sync("Respond", Some(&(response_text,).to_variant()), gio::DBusCallFlags::NONE, 60_000, None::<&gio::Cancellable>)
        .map_err(|error| format!("USSD response failed: {error}"))?;
    Ok(response.child_value(0).get::<String>().unwrap_or_default())
}

pub fn cancel_ussd(modem_path: &str) -> Result<(), String> {
    proxy(modem_path, USSD_IFACE)?
        .call_sync("Cancel", None, gio::DBusCallFlags::NONE, 10_000, None::<&gio::Cancellable>)
        .map(|_| ())
        .map_err(|error| format!("Could not cancel USSD session: {error}"))
}

fn string_prop(proxy: &gio::DBusProxy, name: &str) -> String {
    proxy.cached_property(name).and_then(|value| value.get::<String>()).unwrap_or_default()
}

fn u32_prop(proxy: &gio::DBusProxy, name: &str) -> u32 {
    proxy.cached_property(name).and_then(|value| value.get::<u32>()).unwrap_or_default()
}

fn i32_prop(proxy: &gio::DBusProxy, name: &str) -> i32 {
    proxy.cached_property(name).and_then(|value| value.get::<i32>()).unwrap_or_default()
}
