//! Desktop Notifications (Freedesktop D-Bus `org.freedesktop.Notifications`)
//! and internal Notification Center with duplicate-suppression cooldowns.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotificationEvent {
    ModemConnected,
    ModemDisconnected,
    InternetLost,
    InternetRestored,
    SimRemoved,
    NewSms,
    BandChanged,
    CaActivated,
    PersistentLockRestored,
    PersistentLockFailed,
    BandAnalyzerCompleted,
    SpeedTestCompleted,
    HotspotStarted,
    DataUsageWarning,
    ThermalWarning,
    FirmwareUpdateAvailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRecord {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub timestamp_unix: u64,
    pub read: bool,
}

static COOLDOWNS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
static CENTER: OnceLock<Mutex<Vec<NotificationRecord>>> = OnceLock::new();

pub fn notify_with_cooldown(
    key: &str,
    title: &str,
    body: &str,
    cooldown_secs: u64,
    desktop_enabled: bool,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let map = COOLDOWNS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        if let Some(&last) = guard.get(key)
            && now.saturating_sub(last) < cooldown_secs
        {
            return;
        }
        guard.insert(key.to_string(), now);
    }

    let center = CENTER.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut list) = center.lock() {
        list.push(NotificationRecord {
            id: now,
            title: title.to_string(),
            body: body.to_string(),
            timestamp_unix: now,
            read: false,
        });
        if list.len() > 100 {
            let drain = list.len() - 100;
            list.drain(0..drain);
        }
    }

    if desktop_enabled {
        let _ = send_desktop_notification(title, body);
    }
}

pub fn list_notifications() -> Vec<NotificationRecord> {
    CENTER
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default()
}

pub fn send_desktop_notification(summary: &str, body: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        crate::core::block_on(async {
            let conn = zbus::Connection::session()
                .await
                .map_err(|e| format!("Session D-Bus unavailable: {e}"))?;
            let proxy = zbus::Proxy::new(
                &conn,
                "org.freedesktop.Notifications",
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
            )
            .await
            .map_err(|e| format!("Notifications proxy error: {e}"))?;
            let actions: Vec<&str> = Vec::new();
            let hints: HashMap<&str, zbus::Variant<'_>> = HashMap::new();
            let _: u32 = proxy
                .call(
                    "Notify",
                    &(
                        "ModemDeck",
                        0u32,
                        "net.milmit.ModemDeck",
                        summary,
                        body,
                        actions,
                        hints,
                        5000i32,
                    ),
                )
                .await
                .map_err(|e| format!("Notify call failed: {e}"))?;
            Ok(())
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (summary, body);
        Ok(())
    }
}
