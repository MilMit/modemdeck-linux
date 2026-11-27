//! ModemManager Messaging (SMS), Voice Calls, and USSD over pure-Rust async zbus D-Bus.

use std::collections::HashMap;
use zbus::{OwnedObjectPath, Variant as Value};

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

    pub fn can_answer(&self) -> bool {
        self.state == 3 || self.state == 6
    }

    pub fn can_hangup(&self) -> bool {
        self.state != 7
    }
}

async fn system_conn() -> Result<zbus::Connection, String> {
    zbus::Connection::system()
        .await
        .map_err(|e| format!("D-Bus system bus unavailable: {e}"))
}

async fn make_proxy<'a>(
    conn: &'a zbus::Connection,
    path: &'a str,
    interface: &'a str,
) -> Result<zbus::Proxy<'a>, String> {
    zbus::Proxy::new(conn, MM_BUS, path, interface)
        .await
        .map_err(|e| format!("Could not open {interface}: {e}"))
}

pub fn list_sms(modem_path: &str) -> Result<Vec<SmsMessage>, String> {
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let messaging = make_proxy(&conn, modem_path, MESSAGING_IFACE).await?;
        let paths: Vec<OwnedObjectPath> = messaging
            .call("List", &())
            .await
            .map_err(|e| format!("Could not list SMS messages: {e}"))?;

        let mut messages = Vec::new();
        for path in paths {
            let p = path.as_str().to_string();
            if let Ok(sms) = make_proxy(&conn, &p, SMS_IFACE).await {
                messages.push(SmsMessage {
                    path: p.clone(),
                    number: sms
                        .get_property::<String>("Number")
                        .await
                        .unwrap_or_default(),
                    text: sms.get_property::<String>("Text").await.unwrap_or_default(),
                    timestamp: sms
                        .get_property::<String>("Timestamp")
                        .await
                        .unwrap_or_default(),
                    state: sms.get_property::<u32>("State").await.unwrap_or(0),
                });
            }
        }
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(messages)
    })
}

pub fn send_sms(modem_path: &str, number: &str, text: &str) -> Result<(), String> {
    let number = number.trim();
    if number.is_empty() || text.trim().is_empty() {
        return Err("Enter both a recipient number and a message.".into());
    }
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let messaging = make_proxy(&conn, modem_path, MESSAGING_IFACE).await?;
        let mut props: HashMap<&str, Value<'_>> = HashMap::new();
        props.insert("number", Value::from(number));
        props.insert("text", Value::from(text));

        let sms_path: OwnedObjectPath = messaging
            .call("Create", &(props,))
            .await
            .map_err(|e| format!("Could not create SMS: {e}"))?;
        let sms = make_proxy(&conn, sms_path.as_str(), SMS_IFACE).await?;
        sms.call::<_, _, ()>("Send", &())
            .await
            .map_err(|e| format!("Could not send SMS: {e}"))?;
        Ok(())
    })
}

pub fn delete_sms(modem_path: &str, sms_path: &str) -> Result<(), String> {
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let messaging = make_proxy(&conn, modem_path, MESSAGING_IFACE).await?;
        let obj = OwnedObjectPath::try_from(sms_path)
            .map_err(|e| format!("Invalid SMS D-Bus path: {e}"))?;
        messaging
            .call::<_, _, ()>("Delete", &(obj,))
            .await
            .map_err(|e| format!("Could not delete SMS: {e}"))?;
        Ok(())
    })
}

pub fn list_calls(modem_path: &str) -> Result<Vec<CallInfo>, String> {
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let voice = make_proxy(&conn, modem_path, VOICE_IFACE).await?;
        let paths: Vec<OwnedObjectPath> = voice
            .call("ListCalls", &())
            .await
            .map_err(|e| format!("Could not list calls: {e}"))?;

        let mut calls = Vec::new();
        for path in paths {
            let p = path.as_str().to_string();
            if let Ok(call) = make_proxy(&conn, &p, CALL_IFACE).await {
                calls.push(CallInfo {
                    path: p.clone(),
                    number: call
                        .get_property::<String>("Number")
                        .await
                        .unwrap_or_default(),
                    state: call.get_property::<i32>("State").await.unwrap_or(0),
                    direction: call.get_property::<i32>("Direction").await.unwrap_or(0),
                    audio_port: call
                        .get_property::<String>("AudioPort")
                        .await
                        .unwrap_or_default(),
                });
            }
        }
        Ok(calls)
    })
}

pub fn dial_call(modem_path: &str, number: &str) -> Result<(), String> {
    let number = number.trim();
    if number.is_empty() {
        return Err("Enter a phone number to dial.".into());
    }
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let voice = make_proxy(&conn, modem_path, VOICE_IFACE).await?;
        let mut props: HashMap<&str, Value<'_>> = HashMap::new();
        props.insert("number", Value::from(number));
        let call_path: OwnedObjectPath = voice
            .call("CreateCall", &(props,))
            .await
            .map_err(|e| format!("Could not create call: {e}"))?;
        let call = make_proxy(&conn, call_path.as_str(), CALL_IFACE).await?;
        call.call::<_, _, ()>("Start", &())
            .await
            .map_err(|e| format!("Could not start call: {e}"))?;
        Ok(())
    })
}

pub fn answer_call(call_path: &str) -> Result<(), String> {
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let call = make_proxy(&conn, call_path, CALL_IFACE).await?;
        call.call::<_, _, ()>("Accept", &())
            .await
            .map_err(|e| format!("Could not accept call: {e}"))
    })
}

pub fn hangup_call(call_path: &str) -> Result<(), String> {
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let call = make_proxy(&conn, call_path, CALL_IFACE).await?;
        call.call::<_, _, ()>("Hangup", &())
            .await
            .map_err(|e| format!("Could not hang up call: {e}"))
    })
}

pub fn ussd_initiate(modem_path: &str, command: &str) -> Result<String, String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("Enter a USSD code such as *555#.".into());
    }
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let ussd = make_proxy(&conn, modem_path, USSD_IFACE).await?;
        let state = ussd.get_property::<u32>("State").await.unwrap_or(0);
        let reply: String = if state == 3 {
            ussd.call("Respond", &(command,))
                .await
                .map_err(|e| format!("USSD response failed: {e}"))?
        } else {
            ussd.call("Initiate", &(command,))
                .await
                .map_err(|e| format!("USSD failed: {e}"))?
        };
        Ok(reply)
    })
}

pub fn ussd_cancel(modem_path: &str) -> Result<(), String> {
    crate::core::block_on(async {
        let conn = system_conn().await?;
        let ussd = make_proxy(&conn, modem_path, USSD_IFACE).await?;
        ussd.call::<_, _, ()>("Cancel", &())
            .await
            .map_err(|e| format!("Could not cancel USSD session: {e}"))
    })
}

pub fn ussd_state(modem_path: &str) -> u32 {
    crate::core::block_on(async {
        let Ok(conn) = system_conn().await else {
            return 0;
        };
        let Ok(ussd) = make_proxy(&conn, modem_path, USSD_IFACE).await else {
            return 0;
        };
        ussd.get_property::<u32>("State").await.unwrap_or(0)
    })
}
