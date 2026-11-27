//! Cross-platform System Tray integration.
//! On Linux uses `ksni` (StatusNotifierItem / AppIndicator over D-Bus, zero GTK dependency).

use crate::core::runtime_status::RuntimeStatus;
use std::sync::{Arc, Mutex, mpsc::Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    OpenWindow,
    Refresh,
    ResetBandsAuto,
    RunDoctor,
    Quit,
}

#[derive(Debug, Clone, Default)]
pub struct TrayViewState {
    pub status: RuntimeStatus,
    pub hotspot_active: bool,
    pub tray_host_detected: bool,
}

pub fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    let dir = crate::core::settings::config_home().join("autostart");
    let path = dir.join("net.milmit.ModemDeck.desktop");
    if enabled {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let desktop = "[Desktop Entry]\nType=Application\nName=ModemDeck\nExec=modemdeck --minimized\nIcon=net.milmit.ModemDeck\nTerminal=false\nCategories=Network;System;\nX-GNOME-Autostart-enabled=true\n";
        std::fs::write(path, desktop).map_err(|e| e.to_string())?;
    } else if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Checks if a StatusNotifierWatcher is registered on the session D-Bus.
#[cfg(target_os = "linux")]
pub fn detect_tray_host_available() -> bool {
    crate::core::block_on(async {
        let Ok(conn) = zbus::Connection::session().await else {
            return false;
        };
        let Ok(proxy) = zbus::Proxy::new(
            &conn,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )
        .await
        else {
            return false;
        };
        proxy
            .call::<_, _, bool>("NameHasOwner", &("org.kde.StatusNotifierWatcher",))
            .await
            .unwrap_or(false)
    })
}

#[cfg(not(target_os = "linux"))]
pub fn detect_tray_host_available() -> bool {
    false
}

#[cfg(target_os = "linux")]
pub struct LinuxTrayHandle {
    handle: ksni::blocking::Handle<ModemDeckTray>,
    state: Arc<Mutex<TrayViewState>>,
}

#[cfg(target_os = "linux")]
impl LinuxTrayHandle {
    pub fn update(&self, status: RuntimeStatus) {
        if let Ok(mut g) = self.state.lock() {
            g.status = status;
        }
        self.handle.update(|_| {});
    }
}

#[cfg(target_os = "linux")]
pub fn spawn_linux_tray(tx: Sender<TrayCommand>) -> Option<LinuxTrayHandle> {
    use ksni::blocking::TrayMethods;
    let state = Arc::new(Mutex::new(TrayViewState::default()));
    let tray = ModemDeckTray {
        state: state.clone(),
        tx,
    };
    tray.spawn()
        .ok()
        .map(|handle| LinuxTrayHandle { handle, state })
}

#[cfg(target_os = "linux")]
struct ModemDeckTray {
    state: Arc<Mutex<TrayViewState>>,
    tx: Sender<TrayCommand>,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for ModemDeckTray {
    fn id(&self) -> String {
        "net.milmit.ModemDeck".into()
    }

    fn title(&self) -> String {
        "ModemDeck".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let sig = self
            .state
            .lock()
            .map(|s| s.status.signal_percent)
            .unwrap_or(0);
        let online = self
            .state
            .lock()
            .map(|s| s.status.internet_health == "Connected")
            .unwrap_or(false);
        vec![render_signal_pixmap(sig, online)]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let s = self
            .state
            .lock()
            .map(|g| g.status.clone())
            .unwrap_or_default();
        let desc = format!(
            "Operator: {}\nModem: {}\nNetwork: {} ({})\nSignal: {}%\nRSRP: {:?} | SINR: {:?}\nInternet: {}",
            if s.operator.is_empty() {
                "—"
            } else {
                &s.operator
            },
            if s.modem_name.is_empty() {
                "No Modem"
            } else {
                &s.modem_name
            },
            s.network,
            s.band,
            s.signal_percent,
            s.rsrp,
            s.sinr,
            s.internet_health,
        );
        ksni::ToolTip {
            title: "ModemDeck by MilMit".into(),
            description: desc,
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayCommand::OpenWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let s = self
            .state
            .lock()
            .map(|g| g.status.clone())
            .unwrap_or_default();
        let summary = format!(
            "{} • {} {}% ({})",
            if s.operator.is_empty() {
                "No Operator"
            } else {
                &s.operator
            },
            s.network,
            s.signal_percent,
            s.internet_health
        );
        vec![
            StandardItem {
                label: "ModemDeck by MilMit".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: summary,
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: crate::i18n::tr("tray.open").into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::OpenWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: crate::i18n::tr("tray.refresh").into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::Refresh);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: crate::i18n::tr("tray.reset_bands").into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ResetBandsAuto);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: crate::i18n::tr("tray.doctor").into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::RunDoctor);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: crate::i18n::tr("tray.quit").into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Renders a crisp 22x22 ARGB32 signal-bar icon for StatusNotifierItem.
#[cfg(target_os = "linux")]
pub fn render_signal_pixmap(signal_percent: u32, connected: bool) -> ksni::Icon {
    let w = 22usize;
    let h = 22usize;
    let mut data = vec![0u8; w * h * 4];
    let bars = if signal_percent >= 75 {
        4
    } else if signal_percent >= 50 {
        3
    } else if signal_percent >= 25 {
        2
    } else if signal_percent > 0 {
        1
    } else {
        0
    };

    let (ar, ag, ab) = if !connected && signal_percent == 0 {
        (148u8, 163u8, 184u8)
    } else if connected {
        (56u8, 189u8, 248u8)
    } else {
        (245u8, 158u8, 11u8)
    };

    for bar in 0..4usize {
        let x0 = 2 + bar * 5;
        let bar_h = 4 + bar * 4;
        let y0 = h - 3 - bar_h;
        let active = bar < bars;
        for y in y0..(h - 3) {
            for x in x0..(x0 + 3) {
                let idx = (y * w + x) * 4;
                if active {
                    data[idx] = 255;
                    data[idx + 1] = ar;
                    data[idx + 2] = ag;
                    data[idx + 3] = ab;
                } else {
                    data[idx] = 90;
                    data[idx + 1] = 148;
                    data[idx + 2] = 163;
                    data[idx + 3] = 184;
                }
            }
        }
    }

    ksni::Icon {
        width: w as i32,
        height: h as i32,
        data,
    }
}
