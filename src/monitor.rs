use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gio::prelude::*;

use crate::{
    history,
    modem_manager::ModemManagerClient,
    power,
    profiles,
    runtime_status::{self, RuntimeStatus},
    settings::Settings,
    traffic,
};

pub fn run() -> i32 {
    let settings = Settings::load();
    let refresh = settings.normalized_refresh_seconds();
    let client = match ModemManagerClient::new() {
        Ok(client) => Rc::new(client),
        Err(error) => {
            eprintln!("ModemDeck monitor could not connect to ModemManager: {error}");
            return 2;
        }
    };

    client.ensure_signal_updates(refresh);
    if let Ok(Some(message)) = client.recover_pending_band_change() {
        eprintln!("ModemDeck safety recovery: {message}");
    }
    restore_saved_profile(&client, &settings);
    let last = Rc::new(RefCell::new(None::<RuntimeStatus>));
    publish_snapshot(&client, &last, &settings);
    sample_traffic(&client);

    {
        let client = client.clone();
        let last = last.clone();
        let settings = settings.clone();
        let manager = client.manager().clone();
        manager.connect_object_added(move |_, object| {
            client.ensure_signal_updates_for_object(object, refresh);
            publish_snapshot(&client, &last, &settings);
        });
    }
    {
        let client = client.clone();
        let last = last.clone();
        let settings = settings.clone();
        let manager = client.manager().clone();
        manager.connect_object_removed(move |_, _| {
            publish_snapshot(&client, &last, &settings);
        });
    }
    {
        let client = client.clone();
        let last = last.clone();
        let settings = settings.clone();
        let manager = client.manager().clone();
        manager.connect_local(
            "interface-proxy-properties-changed",
            false,
            move |_| {
                publish_snapshot(&client, &last, &settings);
                None
            },
        );
    }

    {
        let client = client.clone();
        glib::timeout_add_seconds_local(5, move || {
            sample_traffic(&client);
            glib::ControlFlow::Continue
        });
    }

    {
        let client = client.clone();
        let last = last.clone();
        let settings = settings.clone();
        let _ = power::watch_prepare_for_sleep(move |sleeping| {
            if !sleeping {
                client.ensure_signal_updates(settings.normalized_refresh_seconds());
                restore_saved_profile(&client, &settings);
                publish_snapshot(&client, &last, &settings);
                sample_traffic(&client);
            }
        });
    }

    let loop_ = glib::MainLoop::new(None, false);
    #[cfg(unix)]
    {
        let quit = loop_.clone();
        glib_unix::unix_signal_add_local(15, move || {
            quit.quit();
            glib::ControlFlow::Break
        });
        let quit = loop_.clone();
        glib_unix::unix_signal_add_local(2, move || {
            quit.quit();
            glib::ControlFlow::Break
        });
    }
    loop_.run();
    client.restore_signal_updates(refresh);
    0
}

fn publish_snapshot(client: &Rc<ModemManagerClient>, last: &Rc<RefCell<Option<RuntimeStatus>>>, settings: &Settings) {
    let modems = client.modems();
    history::record(&modems);
    let status = runtime_status::status_from_modems(&modems);
    let _ = runtime_status::publish(&status);

    if let Some(previous) = last.borrow().as_ref() {
        if settings.notifications {
            if previous.network != status.network && previous.network != "—" {
                notify("Mobile network changed", &format!("{} → {}", previous.network, status.network));
            }
            if previous.carrier_aggregation != status.carrier_aggregation && status.carrier_aggregation {
                notify("4G+ active", "Carrier aggregation was detected by ModemDeck.");
            }
            if previous.state != status.state && status.state == "Connected" {
                notify("Mobile data connected", &format!("{} • {}", status.operator, status.network));
            }
        }
    }
    *last.borrow_mut() = Some(status);
}

fn restore_saved_profile(client: &Rc<ModemManagerClient>, settings: &Settings) {
    if !settings.restore_profile_on_startup || crate::safety::read_pending().is_some() { return; }
    let store = profiles::load();
    let Some(profile) = store.profiles.iter().find(|p| p.name == store.active_profile) else { return; };
    if let Some(modem) = client.modems().into_iter().next() {
        let _ = client.apply_persistent_bands(&modem.object_path, &profile.bands);
    }
}

fn sample_traffic(client: &Rc<ModemManagerClient>) {
    let modems = client.modems();
    let modem = modems.iter().find(|m| m.state == 11).or_else(|| modems.first());
    if let Some(modem) = modem {
        if let Some(interface) = traffic::infer_interface(&modem.net_ports) {
            let _ = traffic::sample(&interface);
        }
    }
}

fn notify(title: &str, body: &str) {
    let Ok(proxy) = gio::DBusProxy::for_bus_sync(
        gio::BusType::Session,
        gio::DBusProxyFlags::NONE,
        None::<&gio::DBusInterfaceInfo>,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        None::<&gio::Cancellable>,
    ) else { return; };

    let actions: Vec<String> = Vec::new();
    let hints: HashMap<String, glib::Variant> = HashMap::new();
    let params = ("ModemDeck",0u32,"network-cellular",title,body,actions,hints,5000i32).to_variant();
    let _ = proxy.call_sync("Notify",Some(&params),gio::DBusCallFlags::NONE,5_000,None::<&gio::Cancellable>);
}
