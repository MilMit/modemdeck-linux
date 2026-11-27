//! Background monitoring service (pure Rust + zbus, zero GLib/GTK).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    analyzer::stability::StabilityTracker,
    core::{
        power, profiles,
        runtime_status::{self, RuntimeStatus},
        safety,
        settings::Settings,
    },
    locking::{
        band_lock::{LockMode, LockVerificationStatus, verify_band_lock},
        persistent::{self, can_attempt_restore},
    },
    modem::ModemManagerClient,
    notifications,
    services::traffic,
    storage::history_db,
};

pub fn run() -> i32 {
    let settings = Settings::load();
    let refresh = settings.normalized_refresh_seconds();
    let client = match ModemManagerClient::new() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("ModemDeck monitor could not connect to ModemManager: {error}");
            return 2;
        }
    };

    client.ensure_signal_updates(refresh);
    if let Ok(Some(message)) = client.recover_pending_band_change() {
        eprintln!("ModemDeck safety recovery: {message}");
    }
    restore_saved_locks_and_profiles(&client, &settings);

    let last = Arc::new(Mutex::new(None::<RuntimeStatus>));
    let tracker = Arc::new(Mutex::new(StabilityTracker::default()));
    publish_snapshot(&client, &last, &tracker, &settings);
    sample_traffic(&client);

    if settings.remote_management_enabled {
        let token = crate::core::settings::config_home()
            .join("modemdeck/remote_token.txt")
            .to_str()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if token.len() >= 12 {
            let _ = crate::remote::start_localhost_server(crate::remote::RemoteServerConfig {
                enabled: true,
                port: settings.remote_management_port,
                auth_token: token,
                read_only: true,
            });
        }
    }

    {
        let client_c = client.clone();
        let last_c = last.clone();
        let tracker_c = tracker.clone();
        let _ = power::watch_prepare_for_sleep(move |sleeping| {
            if !sleeping {
                let cur_settings = Settings::load();
                client_c.ensure_signal_updates(cur_settings.normalized_refresh_seconds());
                restore_saved_locks_and_profiles(&client_c, &cur_settings);
                publish_snapshot(&client_c, &last_c, &tracker_c, &cur_settings);
            }
        });
    }

    let mut tick: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_secs(5));
        tick += 1;
        let cur_settings = Settings::load();
        sample_traffic(&client);

        if tick.is_multiple_of((cur_settings.normalized_refresh_seconds() / 5).max(1) as u64) {
            let modems = client.modems();
            restore_persistent_locks(&client, &cur_settings, &modems);
            publish_snapshot(&client, &last, &tracker, &cur_settings);
        }
        if tick.is_multiple_of(12) {
            run_scheduler_tick(&client);
        }
        if tick.is_multiple_of(720) {
            let _ = history_db::prune_retention(cur_settings.history_retention_days);
        }
    }
}

fn run_scheduler_tick(client: &ModemManagerClient) {
    let sched = crate::scheduler::SchedulerConfig::load();
    if sched.tasks.is_empty() {
        return;
    }
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let minute_of_day = ((now_secs / 60) % 1440) as u32;
    let cur_h = (minute_of_day / 60) as u8;
    let cur_m = (minute_of_day % 60) as u8;
    let in_quiet = sched.quiet_hours_enabled
        && cur_h >= sched.quiet_start_hour
        && cur_h <= sched.quiet_end_hour;
    if in_quiet {
        return;
    }
    for task in &sched.tasks {
        if task.enabled && task.hour == cur_h && task.minute == cur_m {
            match task.action {
                crate::scheduler::ScheduledAction::RadioDiagnosticsOnly => {
                    let _ = crate::doctor::run_doctor(&client.modems());
                }
                crate::scheduler::ScheduledAction::DataUsageReport => {
                    let _ = crate::diagnostics::export_privacy_safe(
                        &client.modems(),
                        &Settings::load(),
                    );
                }
                _ => {}
            }
        }
    }
}

fn sample_traffic(client: &ModemManagerClient) {
    let modems = client.modems();
    let candidates: Vec<String> = modems.iter().flat_map(|m| m.net_ports.clone()).collect();
    if let Some(interface) = traffic::infer_interface(&candidates) {
        let _ = traffic::sample(&interface);
    }
}

fn restore_saved_locks_and_profiles(client: &ModemManagerClient, settings: &Settings) {
    let modems = client.modems();
    let lock_applied_devices = restore_persistent_locks(client, settings, &modems);

    if !settings.restore_profile_on_startup {
        return;
    }
    let store = profiles::load();
    let Some(profile) = profiles::find(&store.active_profile) else {
        return;
    };
    if profile.bands.is_empty() {
        return;
    }
    for modem in &modems {
        if lock_applied_devices.contains_key(&modem.stable_id()) {
            continue;
        }
        if modem.current_bands != profile.bands {
            let _ = client.set_bands_direct(&modem.object_path, &profile.bands);
        }
    }
}

fn restore_persistent_locks(
    client: &ModemManagerClient,
    settings: &Settings,
    modems: &[crate::model::ModemInfo],
) -> HashMap<String, bool> {
    let lock_store = persistent::load();
    let mut lock_applied_devices: HashMap<String, bool> = HashMap::new();

    for modem in modems {
        let key = modem.stable_id();
        if let Some(mut lock) = persistent::find_for_modem(&lock_store, modem).cloned()
            && lock.mode == LockMode::PersistentLock
            && !lock.bands.is_empty()
            && modem.current_bands != lock.bands
        {
            if safety::current_operation_owner(&key).is_some() {
                continue;
            }
            if !can_attempt_restore(&mut lock)
                || persistent::upsert_for_modem(modem, lock.clone()).is_err()
            {
                continue;
            }
            if client
                .apply_persistent_bands(&modem.object_path, &lock.bands)
                .is_ok()
            {
                let updated_modems = client.modems();
                let verified = updated_modems
                    .iter()
                    .find(|m| m.object_path == modem.object_path)
                    .map(|m| verify_band_lock(&lock.bands, m))
                    .unwrap_or(LockVerificationStatus::Failed);
                lock.last_status = verified;
                lock.last_reason = format!("Background restore: {}", verified.label());
                let _ = persistent::upsert_for_modem(modem, lock);
                lock_applied_devices.insert(key.clone(), true);
                notifications::notify_with_cooldown(
                    "persistent-lock-restored",
                    "ModemDeck",
                    &format!(
                        "Persistent band lock restored on {} ({}).",
                        modem.display_name(),
                        verified.label()
                    ),
                    120,
                    settings.notifications,
                );
            }
        }
    }
    lock_applied_devices
}

fn publish_snapshot(
    client: &ModemManagerClient,
    last: &Arc<Mutex<Option<RuntimeStatus>>>,
    tracker: &Arc<Mutex<StabilityTracker>>,
    settings: &Settings,
) {
    let modems = client.modems();
    history_db::record_modems(&modems);

    if let Some(primary) = modems.first()
        && let Ok(mut trk) = tracker.lock()
    {
        let alerts = trk.evaluate(primary, None, &settings.stability);
        for alert in alerts {
            notifications::notify_with_cooldown(
                &format!("stability-{}", alert.kind.label()),
                "ModemDeck Stability Alert",
                &alert.message,
                settings.stability.notification_cooldown_secs,
                settings.notifications,
            );
        }
    }

    let status = runtime_status::status_from_modems(&modems);
    if let Ok(mut guard) = last.lock() {
        if let Some(prev) = guard.as_ref()
            && settings.notifications
            && prev.network != status.network
            && status.network != "—"
        {
            notifications::notify_with_cooldown(
                "net-change",
                "ModemDeck",
                &format!(
                    "{} switched to {} ({}%)",
                    status.operator, status.network, status.signal_percent
                ),
                15,
                true,
            );
        }
        *guard = Some(status.clone());
    }
    let _ = runtime_status::publish(&status);
}
