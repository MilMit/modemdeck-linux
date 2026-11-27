//! Slint UI Controller, ViewModels, Async Workers, Close-to-Tray, and Single-Instance IPC.

use crate::core::profiles;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{
    analyzer::{
        band_scan, ca_scan,
        signal_score::{RadioTech, evaluate_signal},
        speed_test,
    },
    antenna, backup, cells,
    core::{
        power, runtime_status,
        settings::{self, Settings},
    },
    diagnostics, doctor, esim, failover, fcc, firmware, gps,
    hardware::{HardwareDatabase, LaptopInfo},
    hotspot::wifi_ap,
    i18n,
    locking::{
        band_lock::{LockMode, LockVerificationStatus, verify_band_lock},
        persistent,
    },
    model::{BandCategory, ModemInfo, band_label, classify_mm_band, lte_band_from_earfcn},
    modem::ModemManagerClient,
    netdiag, netmode,
    operators::OperatorDatabase,
    rf_lab, scanner,
    services::{apn, communications, traffic},
    storage::history_db::{self, HistoryFilter, TimeRange},
    terminal, tray,
};

slint::include_modules!();

pub fn run_gui(start_minimized: bool) -> i32 {
    // Single-instance check on Linux via Unix domain socket
    #[cfg(target_os = "linux")]
    if signal_existing_instance() {
        return 0;
    }

    let app = match AppWindow::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Slint UI initialization failed: {e}");
            return 1;
        }
    };
    app.set_app_version(env!("CARGO_PKG_VERSION").into());
    // Match the Wayland/X11 window identity to the installed desktop entry so
    // Ubuntu can associate the running window with its name and application icon.
    let _ = slint::set_xdg_app_id("net.milmit.ModemDeck");

    let settings = Settings::load();
    apply_settings_to_slint(&app, &settings);

    // Tray icon setup
    let (tray_tx, tray_rx) = mpsc::channel::<tray::TrayCommand>();
    #[cfg(target_os = "linux")]
    let tray_host_detected = tray::detect_tray_host_available();
    #[cfg(not(target_os = "linux"))]
    let tray_host_detected = false;

    #[cfg(target_os = "linux")]
    let tray_handle = tray::spawn_linux_tray(tray_tx);
    #[cfg(not(target_os = "linux"))]
    let tray_handle: Option<()> = None;

    let client = ModemManagerClient::new().ok().map(Arc::new);
    if let Some(ref c) = client {
        c.ensure_signal_updates(settings.normalized_refresh_seconds());
        let _ = c.recover_pending_band_change();
    }

    let cached_modems: Rc<RefCell<Vec<ModemInfo>>> = Rc::new(RefCell::new(Vec::new()));
    let selected_bands: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let antenna_session = Rc::new(RefCell::new(antenna::PositioningSession::default()));
    let gps_fixes: Rc<RefCell<Vec<gps::GpsFix>>> = Rc::new(RefCell::new(Vec::new()));
    let analyzer_cancel = Arc::new(AtomicBool::new(false));
    let pending_fixes: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let refresh_all = {
        let app_weak = app.as_weak();
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let sel_bands_c = selected_bands.clone();
        #[cfg(target_os = "linux")]
        let tray_c = tray_handle.as_ref().map(|_| ());
        let _ = tray_c;
        move || {
            let modems = client_c.as_ref().map(|c| c.modems()).unwrap_or_default();
            history_db::record_modems(&modems);
            let status = runtime_status::status_from_modems(&modems);
            let _ = runtime_status::publish(&status);
            *cached_c.borrow_mut() = modems.clone();
            if let Some(w) = app_weak.upgrade() {
                sync_ui(&w, &modems, &sel_bands_c);
            }
        }
    };
    refresh_all();

    // Background polling / D-Bus sync thread (low overhead, updates UI via invoke_from_event_loop)
    {
        let app_weak = app.as_weak();
        let client_c = client.clone();
        let quit_flag = Arc::new(AtomicBool::new(false));
        let quit_c = quit_flag.clone();
        std::thread::spawn(move || {
            while !quit_c.load(Ordering::Relaxed) {
                let interval = Settings::load().normalized_refresh_seconds().max(5) as u64;
                std::thread::sleep(Duration::from_secs(interval));
                let Some(ref c) = client_c else {
                    continue;
                };
                let modems = c.modems();
                history_db::record_modems(&modems);
                let status = runtime_status::status_from_modems(&modems);
                let _ = runtime_status::publish(&status);
                let weak = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak.upgrade() {
                        let dummy = Rc::new(RefCell::new(Vec::new()));
                        sync_ui(&w, &modems, &dummy);
                    }
                });
            }
        });
    }

    // Power sleep/resume watcher
    if let Some(ref c) = client {
        let c_clone = c.clone();
        let app_weak = app.as_weak();
        let _ = power::watch_prepare_for_sleep(move |sleeping| {
            if !sleeping {
                let s = Settings::load();
                c_clone.ensure_signal_updates(s.normalized_refresh_seconds());
                let modems = c_clone.modems();
                let weak = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak.upgrade() {
                        let dummy = Rc::new(RefCell::new(Vec::new()));
                        sync_ui(&w, &modems, &dummy);
                        w.set_status_banner("Resume detected • modem state resynchronized.".into());
                    }
                });
            }
        });
    }

    // Tray & Single-Instance event pump
    {
        let app_weak = app.as_weak();
        let client_c = client.clone();
        let timer = slint::Timer::default();
        let mut tick_count: u32 = 0;
        let mut cell_refresh_pending = true;
        let poll_in_flight = Arc::new(AtomicBool::new(false));
        let (poll_tx, poll_rx) = std::sync::mpsc::channel::<Vec<ModemInfo>>();
        let cached_poll = cached_modems.clone();
        let sel_poll = selected_bands.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                tick_count = tick_count.wrapping_add(1);
                while let Ok(list) = poll_rx.try_recv() {
                    if let Some(w) = app_weak.upgrade() {
                        *cached_poll.borrow_mut() = list.clone();
                        sync_ui(&w, &list, &sel_poll);
                    }
                }
                while let Ok(cmd) = tray_rx.try_recv() {
                    let Some(w) = app_weak.upgrade() else {
                        continue;
                    };
                    match cmd {
                        tray::TrayCommand::OpenWindow => {
                            let _ = w.show();
                        }
                        tray::TrayCommand::Refresh => {
                            w.invoke_refresh_clicked();
                        }
                        tray::TrayCommand::ResetBandsAuto => {
                            w.invoke_apply_bands_auto();
                        }
                        tray::TrayCommand::RunDoctor => {
                            w.set_active_page(7);
                            w.invoke_run_doctor();
                            let _ = w.show();
                        }
                        tray::TrayCommand::Quit => {
                            let _ = slint::quit_event_loop();
                        }
                    }
                }
                if tick_count.is_multiple_of(12)
                    && let Some(w) = app_weak.upgrade()
                    && w.window().is_visible()
                    && !w.get_is_busy()
                    && !poll_in_flight.swap(true, Ordering::Relaxed)
                    && let Some(ref c) = client_c
                {
                    let refresh_cells = cell_refresh_pending || tick_count.is_multiple_of(120);
                    cell_refresh_pending = false;
                    let c_bg = c.clone();
                    let flag = poll_in_flight.clone();
                    let tx_bg = poll_tx.clone();
                    std::thread::spawn(move || {
                        let mut list = c_bg.modems();
                        if refresh_cells {
                            for modem in &list {
                                let _ = c_bg.refresh_cell_info(&modem.object_path);
                            }
                            list = c_bg.modems();
                        }
                        let _ = tx_bg.send(list);
                        flag.store(false, Ordering::Relaxed);
                    });
                }
                #[cfg(target_os = "linux")]
                if tick_count.is_multiple_of(12)
                    && let Some(ref th) = tray_handle
                    && let Some(ref c) = client_c
                {
                    let status = runtime_status::status_from_modems(&c.modems());
                    th.update(status);
                }
            },
        );
        std::mem::forget(timer);
    }

    // Single-instance listener thread
    #[cfg(target_os = "linux")]
    {
        let app_weak = app.as_weak();
        spawn_single_instance_listener(move || {
            let weak = app_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    let _ = w.show();
                }
            });
        });
    }

    // Close-to-Tray handling
    {
        let app_weak = app.as_weak();
        app.window().on_close_requested(move || {
            let s = Settings::load();
            if s.close_to_tray && tray_host_detected {
                if let Some(w) = app_weak.upgrade() {
                    let _ = w.hide();
                }
                slint::CloseRequestResponse::HideWindow
            } else {
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
    }

    // Wire Callbacks
    {
        let app_weak = app.as_weak();
        app.on_clear_debug_log(move || {
            if let Some(w) = app_weak.upgrade() {
                w.set_debug_log_text("".into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_copy_debug_log(move || {
            if let Some(w) = app_weak.upgrade() {
                let text = w.get_debug_log_text().to_string();
                let mut copied = false;
                for (cmd, args) in [
                    ("wl-copy", &[][..]),
                    ("xclip", &["-selection", "clipboard"][..]),
                    ("xsel", &["--clipboard", "--input"][..]),
                ] {
                    if let Ok(mut child) = std::process::Command::new(cmd)
                        .args(args)
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                    {
                        use std::io::Write;
                        if let Some(mut stdin) = child.stdin.take() {
                            let _ = stdin.write_all(text.as_bytes());
                        }
                        if child.wait().map(|s| s.success()).unwrap_or(false) {
                            copied = true;
                            break;
                        }
                    }
                }
                let path = downloads_dir().join("modemdeck-debug.log");
                let _ = std::fs::write(&path, &text);
                w.set_status_banner(
                    if copied {
                        format!(
                            "Debug log copied to clipboard & saved to {}",
                            path.display()
                        )
                    } else {
                        format!("Debug log saved to {}", path.display())
                    }
                    .into(),
                );
            }
        });
    }

    {
        let refresh = refresh_all.clone();
        let app_weak = app.as_weak();
        app.on_refresh_clicked(move || {
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, "UI: Refresh clicked");
            }
            refresh();
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_cycle_theme(move || {
            let mut s = Settings::load();
            s.theme = match s.theme.as_str() {
                "dark" => "light",
                "light" => "high-contrast",
                _ => "dark",
            }
            .into();
            let _ = s.save();
            if let Some(w) = app_weak.upgrade() {
                apply_settings_to_slint(&w, &s);
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_set_complexity(move |idx| {
            let mut s = Settings::load();
            s.ui_mode = match idx {
                0 => "basic",
                2 => "expert",
                _ => "advanced",
            }
            .into();
            let _ = s.save();
            if let Some(w) = app_weak.upgrade() {
                apply_settings_to_slint(&w, &s);
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_set_font_size(move |preset| {
            let mut s = Settings::load();
            s.font_size_preset = preset.to_string();
            let _ = s.save();
            if let Some(w) = app_weak.upgrade() {
                apply_settings_to_slint(&w, &s);
            }
        });
    }

    {
        let sel_c = selected_bands.clone();
        let app_weak = app.as_weak();
        let cached_c = cached_modems.clone();
        app.on_select_all_bands(move || {
            let modems = cached_c.borrow();
            if let Some(m) = modems.first() {
                *sel_c.borrow_mut() = m
                    .supported_bands
                    .iter()
                    .copied()
                    .filter(|b| *b != 256 && *b != 0)
                    .collect();
                if let Some(w) = app_weak.upgrade() {
                    update_band_model(&w, m, &sel_c.borrow());
                }
            }
        });
    }

    {
        let sel_c = selected_bands.clone();
        let app_weak = app.as_weak();
        let cached_c = cached_modems.clone();
        app.on_deselect_all_bands(move || {
            sel_c.borrow_mut().clear();
            let modems = cached_c.borrow();
            if let (Some(w), Some(m)) = (app_weak.upgrade(), modems.first()) {
                update_band_model(&w, m, &sel_c.borrow());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_search_operators(move |query| {
            let q = query.trim().to_ascii_lowercase();
            let db = OperatorDatabase::global();
            let matches: Vec<String> = db
                .operators
                .iter()
                .filter(|op| {
                    q.is_empty()
                        || op.name.to_ascii_lowercase().contains(&q)
                        || op.country.to_ascii_lowercase().contains(&q)
                        || format!("{}-{}", op.mcc, op.mnc).contains(&q)
                })
                .take(12)
                .map(|op| {
                    format!(
                        "{} ({}) • {}-{} • {}",
                        op.name,
                        op.country,
                        op.mcc,
                        op.mnc,
                        op.technologies.join("/")
                    )
                })
                .collect();
            if let Some(w) = app_weak.upgrade() {
                w.set_op_search_results(
                    if matches.is_empty() {
                        i18n::tr("status.unavailable").into()
                    } else {
                        matches.join("\n")
                    }
                    .into(),
                );
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_set_language(move |lang| {
            let mut s = Settings::load();
            s.language = lang.to_string();
            let _ = s.save();
            i18n::init(&s.language);
            if let Some(w) = app_weak.upgrade() {
                apply_settings_to_slint(&w, &s);
                w.set_status_banner(SharedString::from(format!(
                    "Language set to {} (RTL={})",
                    s.language,
                    i18n::is_rtl()
                )));
            }
        });
    }

    // Band Toggle & Apply
    {
        let sel_c = selected_bands.clone();
        let app_weak = app.as_weak();
        let cached_c = cached_modems.clone();
        app.on_toggle_band(move |mm_enum| {
            let val = mm_enum as u32;
            let mut list = sel_c.borrow_mut();
            if let Some(pos) = list.iter().position(|&b| b == val) {
                list.remove(pos);
            } else {
                list.push(val);
            }
            drop(list);
            if let Some(w) = app_weak.upgrade() {
                let modems = cached_c.borrow();
                if let Some(m) = modems.first() {
                    update_band_model(&w, m, &sel_c.borrow());
                }
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_apply_bands_auto(move || {
            let Some(ref c) = client_c else { return };
            let path = cached_c
                .borrow()
                .first()
                .map(|m| m.object_path.clone())
                .unwrap_or_default();
            if path.is_empty() {
                return;
            }
            match c.apply_band_selection(&path, &[256], Some("Auto".into())) {
                Ok(_) => {
                    if let Some(w) = app_weak.upgrade() {
                        w.set_band_confirm_pending(true);
                        w.set_band_change_msg("Auto bands applied. Confirm within 45s.".into());
                    }
                }
                Err(e) => {
                    if let Some(w) = app_weak.upgrade() {
                        w.set_status_banner(e.into());
                    }
                }
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let sel_c = selected_bands.clone();
        let app_weak = app.as_weak();
        app.on_apply_bands_custom(move || {
            let Some(ref c) = client_c else { return };
            let path = cached_c
                .borrow()
                .first()
                .map(|m| m.object_path.clone())
                .unwrap_or_default();
            let bands = sel_c.borrow().clone();
            if path.is_empty() || bands.is_empty() {
                if let Some(w) = app_weak.upgrade() {
                    w.set_status_banner("Select at least one band first.".into());
                }
                return;
            }
            match c.apply_band_selection(&path, &bands, None) {
                Ok(_) => {
                    if let Some(w) = app_weak.upgrade() {
                        w.set_band_confirm_pending(true);
                        w.set_band_change_msg(
                            "Band selection applied. Confirm within 45s or it rolls back.".into(),
                        );
                    }
                }
                Err(e) => {
                    if let Some(w) = app_weak.upgrade() {
                        w.set_status_banner(e.into());
                    }
                }
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_confirm_band_change(move || {
            if let (Some(c), Some(m)) = (&client_c, cached_c.borrow().first()) {
                c.confirm_band_selection(&m.object_path);
                if let Some(w) = app_weak.upgrade() {
                    w.set_band_confirm_pending(false);
                    w.set_band_change_msg("Band selection confirmed.".into());
                }
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_rollback_band_change(move || {
            if let (Some(c), Some(m)) = (&client_c, cached_c.borrow().first()) {
                let _ = c.rollback_band_selection(&m.object_path);
                if let Some(w) = app_weak.upgrade() {
                    w.set_band_confirm_pending(false);
                    w.set_band_change_msg("Rolled back to previous bands.".into());
                }
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let sel_c = selected_bands.clone();
        let app_weak = app.as_weak();
        app.on_save_persistent_lock(move || {
            let Some(ref c) = client_c else {
                if let Some(w) = app_weak.upgrade() {
                    w.set_status_banner("Modem service is unavailable; the persistent lock was not saved.".into());
                }
                return;
            };
            let Some(m) = cached_c.borrow().first().cloned() else {
                if let Some(w) = app_weak.upgrade() {
                    w.set_status_banner("No modem is connected; the persistent lock was not saved.".into());
                }
                return;
            };
            let bands = sel_c.borrow().clone();
            if bands.is_empty() {
                if let Some(w) = app_weak.upgrade() {
                    w.set_status_banner("Select at least one band before saving a persistent lock.".into());
                }
                return;
            }
            if let Err(e) = c.apply_persistent_bands(&m.object_path, &bands) {
                if let Some(w) = app_weak.upgrade() {
                    w.set_status_banner(format!("Could not apply the persistent band lock: {e}").into());
                }
                return;
            }

            let verified = c
                .modems()
                .into_iter()
                .find(|x| x.object_path == m.object_path)
                .map(|x| verify_band_lock(&bands, &x))
                .unwrap_or(LockVerificationStatus::Failed);
            let entry = persistent::PersistentLockEntry {
                modem_uid: m.stable_id(),
                sim_hash: m.sim_identity.stable_hash.clone(),
                operator_code: m.operator_code.clone(),
                mode: LockMode::PersistentLock,
                policy: crate::locking::band_lock::LockPolicy::Safe,
                bands: bands.clone(),
                expires_unix: None,
                updated_unix: 0,
                last_status: verified,
                last_reason: "Locked via Slint UI".into(),
                recent_restore_attempts: Vec::new(),
            };
            match persistent::upsert_for_modem(&m, entry) {
                Ok(()) => {
                    let monitor_result = settings::set_background_monitor_enabled(true);
                    let mut prefs = Settings::load();
                    prefs.background_monitor = monitor_result.is_ok();
                    let _ = prefs.save();
                    if let Some(w) = app_weak.upgrade() {
                        w.set_lock_status(format!("Persistent ({})", verified.label()).into());
                        w.set_pref_bg_monitor(prefs.background_monitor);
                        let message = match monitor_result {
                            Ok(_) => format!(
                                "Persistent lock saved for {} band(s); automatic restoration is enabled.",
                                bands.len()
                            ),
                            Err(e) => format!(
                                "Persistent lock saved, but automatic restoration could not be enabled: {e}"
                            ),
                        };
                        w.set_status_banner(message.into());
                    }
                }
                Err(e) => {
                    if let Some(w) = app_weak.upgrade() {
                        w.set_status_banner(format!(
                            "Bands were applied, but the persistent lock could not be saved: {e}"
                        ).into());
                    }
                }
            }
        });
    }

    // Band Analyzer & Speed Test (async off UI thread)
    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let cancel_c = analyzer_cancel.clone();
        let app_weak = app.as_weak();
        app.on_start_band_analyzer(move |duration_secs, generation_mode, with_speedtest| {
            let Some(ref c) = client_c else { return };
            let Some(m) = cached_c.borrow().first().cloned() else {
                return;
            };
            let bands = analyzer_bands_for_generation(&m.supported_bands, generation_mode);
            if bands.is_empty() {
                let generation = match generation_mode {
                    0 => "3G",
                    1 => "4G LTE",
                    2 => "5G NR",
                    _ => "selected",
                };
                let message = i18n::tr("analyzer.no_bands").replace("{generation}", generation);
                if let Some(w) = app_weak.upgrade() {
                    w.set_analyzer_summary(message.clone().into());
                    w.set_status_banner(message.into());
                }
                return;
            }
            cancel_c.store(false, Ordering::Relaxed);
            if let Some(w) = app_weak.upgrade() {
                w.set_is_busy(true);
                w.set_busy_message(i18n::tr("analyzer.title").into());
                append_debug(&w, &format!("UI: Band Analyzer started ({duration_secs}s)"));
                w.set_analyzer_running(true);
                w.set_analyzer_progress(0);
                w.set_analyzer_stage("Preparing bands…".into());
                w.set_analyzer_live_rf("".into());
                w.set_analyzer_results(ModelRc::new(VecModel::from(
                    Vec::<AnalyzerResultRow>::new(),
                )));
                w.set_analyzer_summary("Running Band Analyzer…".into());
            }
            let c_thread = c.clone();
            let cancel_thread = cancel_c.clone();
            let weak = app_weak.clone();
            std::thread::spawn(move || {
                let total = bands.len().max(1);
                let mut results = Vec::new();
                let mut ui_rows: Vec<AnalyzerResultRow> = Vec::new();
                let iface = m.net_ports.first().cloned().unwrap_or_default();
                for (i, band) in bands.iter().enumerate() {
                    if cancel_thread.load(Ordering::Relaxed) {
                        break;
                    }
                    let b_name = band_label(*band);
                    let w_stage = weak.clone();
                    let stage_txt = format!(
                        "Testing {b_name} ({}/{total}) — Switching & stabilizing…",
                        i + 1
                    );
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = w_stage.upgrade() {
                            w.set_analyzer_stage(stage_txt.into());
                        }
                    });
                    let _ = c_thread.set_bands_direct(&m.object_path, &[*band]);
                    let wait = (duration_secs as u64).clamp(4, 900);
                    for _ in 0..(wait / 2).max(1) {
                        if cancel_thread.load(Ordering::Relaxed) {
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(2));
                    }
                    let cur = c_thread
                        .modems()
                        .into_iter()
                        .find(|x| x.object_path == m.object_path);
                    if let Some(cm) = cur {
                        let (_, snap) = cm.preferred_snapshot();
                        let live_rf = format!(
                            "{b_name}: RSRP {} • RSRQ {} • SINR {}",
                            fmt_opt(snap.signal.rsrp, "dBm"),
                            fmt_opt(snap.signal.rsrq, "dB"),
                            fmt_opt(snap.signal.snr, "dB")
                        );
                        let (dl_mbps, ping_ms) = if with_speedtest
                            && !iface.is_empty()
                            && !cancel_thread.load(Ordering::Relaxed)
                        {
                            let w_sp = weak.clone();
                            let sp_txt = format!(
                                "Testing {b_name} ({}/{total}) — Measuring speed & latency…",
                                i + 1
                            );
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = w_sp.upgrade() {
                                    w.set_analyzer_stage(sp_txt.into());
                                    w.set_analyzer_live_rf(live_rf.into());
                                }
                            });
                            let cfg = speed_test::SpeedTestConfig {
                                server: speed_test::default_servers()
                                    .into_iter()
                                    .next()
                                    .unwrap_or_default(),
                                interface: iface.clone(),
                                run_download: true,
                                run_upload: false,
                                data_cap_mb: 50,
                                probe_count: 1,
                            };
                            let sp = speed_test::run_speed_test(&cfg, &cancel_thread);
                            (sp.download_mbps, sp.idle_latency_ms)
                        } else {
                            (None, None)
                        };
                        let res = band_scan::build_band_result_from_samples(
                            *band,
                            &cm,
                            std::slice::from_ref(&snap.signal),
                            0,
                            0,
                            100.0,
                            dl_mbps,
                            None,
                            ping_ms,
                            None,
                            None,
                            None,
                            None,
                            band_scan::BandTestOutcome::Passed,
                            band_scan::verify_serving_band(*band, &cm).unwrap_or(false),
                            if with_speedtest {
                                "Speedtest"
                            } else {
                                "Radio-only"
                            }
                            .into(),
                        );
                        ui_rows.push(AnalyzerResultRow {
                            mm_enum: *band as i32,
                            band: b_name.into(),
                            rsrp: fmt_opt(snap.signal.rsrp, "dBm").into(),
                            sinr: fmt_opt(snap.signal.snr, "dB").into(),
                            dl_mbps: dl_mbps
                                .map(|v| format!("{v:.1} Mbps"))
                                .unwrap_or_else(|| "—".into())
                                .into(),
                            ping_ms: ping_ms
                                .map(|v| format!("{v} ms"))
                                .unwrap_or_else(|| "—".into())
                                .into(),
                            score: res
                                .score
                                .map(|v| format!("{v}/100"))
                                .unwrap_or_else(|| "—".into())
                                .into(),
                            status: if res.verified_serving_band {
                                "Verified".into()
                            } else {
                                "Sampled".into()
                            },
                        });
                        results.push(res);
                    }
                    let pct = (((i + 1) * 100) / total) as i32;
                    let w2 = weak.clone();
                    let rows_clone = ui_rows.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = w2.upgrade() {
                            w.set_analyzer_progress(pct);
                            w.set_analyzer_results(ModelRc::new(VecModel::from(rows_clone)));
                        }
                    });
                }
                let restore = if m.current_bands.is_empty() {
                    vec![256]
                } else {
                    m.current_bands.clone()
                };
                let _ = c_thread.set_bands_direct(&m.object_path, &restore);
                std::thread::sleep(Duration::from_millis(1500));
                let summary = band_scan::summarize_results(&results);
                let w2 = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = w2.upgrade() {
                        w.set_is_busy(false);
                        w.set_busy_message("".into());
                        append_debug(&w, "OK(BandAnalyzer): finished & restored bands");
                        w.set_analyzer_running(false);
                        w.set_analyzer_summary(
                            format!(
                                "Completed {} band(s). Best Overall: {}",
                                results.len(),
                                summary.best_overall.as_deref().unwrap_or("—")
                            )
                            .into(),
                        );
                        w.invoke_refresh_clicked();
                    }
                });
            });
        });
    }

    {
        let cancel_c = analyzer_cancel.clone();
        app.on_cancel_band_analyzer(move || {
            cancel_c.store(true, Ordering::Relaxed);
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_apply_analyzed_band(move |mm_enum| {
            let b = mm_enum as u32;
            let b_name = band_label(b);
            let msg = if let (Some(c), Some(m)) = (&client_c, cached_c.borrow().first()) {
                match c.apply_band_selection(&m.object_path, &[b], None) {
                    Ok(_) => format!("Applied {b_name} from Analyzer (45s rollback armed)"),
                    Err(e) => e,
                }
            } else {
                "No modem available".into()
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &format!("UI(AnalyzerApply): {msg}"));
                w.set_band_confirm_pending(true);
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_run_speedtest(move || {
            if let Some(w) = app_weak.upgrade() {
                w.set_is_busy(true);
                w.set_busy_message(i18n::tr("speedtest.title").into());
                append_debug(&w, "UI: Speed Test started");
            }
            let iface = cached_c
                .borrow()
                .first()
                .and_then(|m| m.net_ports.first().cloned())
                .unwrap_or_default();
            let weak = app_weak.clone();
            std::thread::spawn(move || {
                let server = speed_test::default_servers()
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                let cfg = speed_test::SpeedTestConfig {
                    server,
                    interface: iface,
                    run_download: true,
                    run_upload: true,
                    data_cap_mb: 250,
                    probe_count: 3,
                };
                let cancel = Arc::new(AtomicBool::new(false));
                let res = speed_test::run_speed_test(&cfg, &cancel);
                let is_err = res.error.is_some();
                let msg = if let Some(err) = res.error {
                    err
                } else {
                    format!(
                        "DL: {:.1} Mbps | UL: {:.1} Mbps | Ping: {:?} ms",
                        res.download_mbps.unwrap_or(0.0),
                        res.upload_mbps.unwrap_or(0.0),
                        res.idle_latency_ms
                    )
                };
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak.upgrade() {
                        w.set_is_busy(false);
                        w.set_busy_message("".into());
                        append_debug(
                            &w,
                            &format!(
                                "{}: {}",
                                if is_err {
                                    "ERROR(SpeedTest)"
                                } else {
                                    "OK(SpeedTest)"
                                },
                                msg
                            ),
                        );
                        w.set_speedtest_result(msg.into());
                    }
                });
            });
        });
    }

    // Operators & APN & SIM / eSIM
    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_create_recommended_apn(move || {
            let modems = cached_c.borrow();
            let Some(m) = modems.first() else { return };
            let recs = apn::recommended_apns(&m.operator_code, &m.spn, &m.gid1);
            if let Some(first) = recs.first() {
                let msg = match apn::apply_recommended_apn(first, false) {
                    Ok(()) => format!("Created APN '{}' (backup saved).", first.apn),
                    Err(e) => e,
                };
                if let Some(w) = app_weak.upgrade() {
                    w.set_status_banner(msg.into());
                }
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_activate_apn(move |uuid| {
            let msg = match apn::activate(uuid.as_str()) {
                Ok(()) => "Activated APN profile.".into(),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_unlock_sim_pin(move |pin| {
            if let (Some(c), Some(m)) = (&client_c, cached_c.borrow().first()) {
                let msg = match c.sim_unlock(&m.object_path, pin.as_str()) {
                    Ok(()) => "SIM unlocked.".into(),
                    Err(e) => e,
                };
                if let Some(w) = app_weak.upgrade() {
                    w.set_status_banner(msg.into());
                }
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_switch_sim_slot(move |slot| {
            let idx = cached_c
                .borrow()
                .first()
                .and_then(|m| m.object_path.rsplit('/').next().map(|s| s.to_string()))
                .unwrap_or_else(|| "0".into());
            let msg = match esim::switch_primary_sim_slot(&idx, slot as u32) {
                Ok(()) => format!("Switched primary SIM slot to {slot}."),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_load_esim_profiles(move || {
            let msg = match esim::list_profiles_via_lpac() {
                Ok(list) => format!("{} eSIM profile(s) found.", list.len()),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_esim_status_text(msg.into());
            }
        });
    }

    // SMS & USSD
    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_refresh_sms(move || {
            let Some(m) = cached_c.borrow().first().cloned() else {
                return;
            };
            if let Ok(list) = communications::list_sms(&m.object_path)
                && let Some(w) = app_weak.upgrade()
            {
                let items: Vec<SmsItem> = list
                    .into_iter()
                    .map(|s| SmsItem {
                        state: s.state_label().into(),
                        sender: s.number.into(),
                        text: s.text.into(),
                        timestamp: s.timestamp.into(),
                    })
                    .collect();
                w.set_sms_items(ModelRc::new(VecModel::from(items)));
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_send_sms(move |num, body| {
            let Some(m) = cached_c.borrow().first().cloned() else {
                return;
            };
            let msg = match communications::send_sms(&m.object_path, num.as_str(), body.as_str()) {
                Ok(()) => "SMS sent.".into(),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_send_ussd(move |code| {
            let Some(m) = cached_c.borrow().first().cloned() else {
                return;
            };
            let reply =
                communications::ussd_initiate(&m.object_path, code.as_str()).unwrap_or_else(|e| e);
            if let Some(w) = app_weak.upgrade() {
                w.set_ussd_reply_text(reply.into());
            }
        });
    }

    // Doctor, Netdiag, GPS, Hotspot, Failover, Scanner, Antenna, AT, Backup
    {
        let cached_c = cached_modems.clone();
        let pending_c = pending_fixes.clone();
        let app_weak = app.as_weak();
        app.on_run_doctor(move || {
            let modems = cached_c.borrow().clone();
            let rep = doctor::run_doctor(&modems);
            let mut fixes = Vec::new();
            let items: Vec<DoctorItem> = rep
                .checks
                .into_iter()
                .map(|c| {
                    if let Some(fx) = c.safe_auto_fix {
                        fixes.push(fx);
                    }
                    DoctorItem {
                        title: i18n::translate_ui_text(&c.title).into(),
                        severity: i18n::translate_ui_text(c.severity.label()).into(),
                        explanation: i18n::translate_ui_text(&c.explanation).into(),
                    }
                })
                .collect();
            *pending_c.borrow_mut() = fixes;
            if let Some(w) = app_weak.upgrade() {
                append_debug(
                    &w,
                    &format!("UI: Doctor finished ({})", rep.overall.label()),
                );
                w.set_doctor_overall(i18n::translate_ui_text(rep.overall.label()).into());
                w.set_doctor_items(ModelRc::new(VecModel::from(items)));
            }
        });
    }

    {
        let pending_c = pending_fixes.clone();
        let app_weak = app.as_weak();
        app.on_fix_doctor_safe(move || {
            let list = pending_c.borrow().clone();
            let res: Vec<String> = list
                .iter()
                .map(|fx| doctor::apply_safe_auto_fix(fx).unwrap_or_else(|e| e))
                .collect();
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(if res.is_empty() {
                    "No safe auto-fixes pending.".into()
                } else {
                    res.join(" • ").into()
                });
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_run_netdiag(move || {
            if let Some(w) = app_weak.upgrade() {
                w.set_is_busy(true);
                w.set_busy_message(i18n::tr("netdiag.title").into());
                append_debug(&w, "UI: NetDiag probes started");
            }
            let (iface, sig) = cached_c
                .borrow()
                .first()
                .map(|m| {
                    (
                        m.net_ports.first().cloned().unwrap_or_default(),
                        m.signal_percent,
                    )
                })
                .unwrap_or_default();
            let weak = app_weak.clone();
            std::thread::spawn(move || {
                let rep = netdiag::run_quick_diagnostics(&iface, sig);
                let mut text = format!("{}: {}\n", rep.diagnosis.label(), rep.evidence_summary);
                for p in rep.probes {
                    text.push_str(&format!(
                        "• {}: {} (DNS {:?}ms, TCP {:?}ms)\n",
                        p.label,
                        if p.reachable { "OK" } else { &p.error },
                        p.dns_ms,
                        p.tcp_ms
                    ));
                }
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak.upgrade() {
                        w.set_is_busy(false);
                        w.set_busy_message("".into());
                        append_debug(&w, "OK(NetDiag): probes finished");
                        w.set_netdiag_text(text.into());
                    }
                });
            });
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_export_html_health(move || {
            let modems = cached_c.borrow();
            let Some(m) = modems.first() else { return };
            let dest = downloads_dir().join("modemdeck-health-card.html");
            let msg = match rf_lab::export_html_health_card(m, &dest) {
                Ok(()) => format!("Saved HTML health card to {}", dest.display()),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_export_diagnostics(move || {
            let msg = match diagnostics::export_privacy_safe(&cached_c.borrow(), &Settings::load())
            {
                Ok(p) => format!("Exported diagnostics to {}", p.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_toggle_gps(move |enable| {
            let idx = mm_idx(&cached_c.borrow());
            let msg = match gps::set_gps_enabled(&idx, enable) {
                Ok(()) => if enable {
                    "GPS enabled."
                } else {
                    "GPS disabled."
                }
                .into(),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_gps_status_text(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let fixes_c = gps_fixes.clone();
        let app_weak = app.as_weak();
        app.on_read_gps(move || {
            let idx = mm_idx(&cached_c.borrow());
            let msg = match gps::read_location(&idx) {
                Ok(fix) => {
                    let s = if fix.has_fix {
                        format!(
                            "Lat {:?}, Lon {:?}, Alt {:?}m",
                            fix.latitude, fix.longitude, fix.altitude_m
                        )
                    } else {
                        "No GNSS fix yet.".into()
                    };
                    fixes_c.borrow_mut().push(fix);
                    s
                }
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_gps_status_text(msg.into());
            }
        });
    }

    {
        let fixes_c = gps_fixes.clone();
        let app_weak = app.as_weak();
        app.on_export_gps_gpx(move || {
            let path = downloads_dir().join("modemdeck-gps.gpx");
            let msg = match std::fs::write(&path, gps::export_gpx(&fixes_c.borrow())) {
                Ok(()) => format!("Exported GPX to {}", path.display()),
                Err(e) => e.to_string(),
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let fixes_c = gps_fixes.clone();
        let app_weak = app.as_weak();
        app.on_export_gps_csv(move || {
            let path = downloads_dir().join("modemdeck-gps.csv");
            let msg = match std::fs::write(&path, gps::export_csv(&fixes_c.borrow())) {
                Ok(()) => format!("Exported CSV to {}", path.display()),
                Err(e) => e.to_string(),
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_toggle_hotspot(move |enable| {
            let msg = if enable {
                "Set a hotspot password in the Hotspot page before starting sharing.".to_string()
            } else {
                match wifi_ap::stop_hotspot() {
                    Ok(()) => "Hotspot stopped.".into(),
                    Err(e) => e,
                }
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_hotspot_active(wifi_ap::current_status().active);
                w.set_hotspot_status_text(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_start_hotspot_with(move |ssid, pass| {
            let msg = match wifi_ap::start_hotspot(
                &ssid,
                &pass,
                wifi_ap::HotspotBand::Bg24Ghz,
                None,
                false,
            ) {
                Ok(m) => m,
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_hotspot_active(wifi_ap::current_status().active);
                append_debug(&w, &format!("UI(Hotspot): {msg}"));
                w.set_hotspot_status_text(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_export_history_csv(move || {
            let recs = history_db::query(TimeRange::Last30Days, &HistoryFilter::default());
            let path = downloads_dir().join("modemdeck-history.csv");
            let msg = match history_db::export_csv(&recs, &path) {
                Ok(()) => format!("Exported {} records to {}", recs.len(), path.display()),
                Err(e) => format!("CSV export failed: {e}"),
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_export_history_json(move || {
            let recs = history_db::query(TimeRange::Last30Days, &HistoryFilter::default());
            let path = downloads_dir().join("modemdeck-history.json");
            let msg = match history_db::export_json(&recs, &path) {
                Ok(()) => format!("Exported {} records to {}", recs.len(), path.display()),
                Err(e) => format!("JSON export failed: {e}"),
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_clear_history(move || {
            let msg = match history_db::clear() {
                Ok(()) => "Signal history cleared.".into(),
                Err(e) => format!("Clear history failed: {e}"),
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                w.set_history_summary_text("0 telemetry samples recorded in last 24h.".into());
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let sel_c = selected_bands.clone();
        let app_weak = app.as_weak();
        app.on_save_band_profile(move |name| {
            let bands = sel_c.borrow().clone();
            let msg = match profiles::upsert(&name, bands) {
                Ok(()) => format!("Saved band profile: {name}"),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                update_band_profiles_model(&w);
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_apply_band_profile(move |name| {
            let Some(prof) = profiles::find(&name) else {
                return;
            };
            let _ = profiles::set_active(&name);
            let msg = if let (Some(c), Some(m)) = (&client_c, cached_c.borrow().first()) {
                match c.apply_band_selection(&m.object_path, &prof.bands, Some(name.to_string())) {
                    Ok(_) => format!("Applied profile {name} (45s rollback armed)"),
                    Err(e) => e,
                }
            } else {
                "No modem available".into()
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                update_band_profiles_model(&w);
                w.set_band_confirm_pending(true);
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_delete_band_profile(move |name| {
            let msg = match profiles::remove(&name) {
                Ok(()) => format!("Removed band profile: {name}"),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                update_band_profiles_model(&w);
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let refresh = refresh_all.clone();
        app.on_create_manual_apn(move |name, apn_str| {
            let msg = match apn::create(&name, &apn_str, "", "", true) {
                Ok(()) => format!("Created APN profile: {name} ({apn_str})"),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                w.set_status_banner(msg.into());
            }
            refresh();
        });
    }

    {
        let app_weak = app.as_weak();
        let refresh = refresh_all.clone();
        app.on_delete_apn(move |uuid| {
            let msg = match apn::delete(&uuid) {
                Ok(()) => "Deleted APN profile.".into(),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &msg);
                w.set_status_banner(msg.into());
            }
            refresh();
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_dial_call(move |number| {
            let path = cached_c
                .borrow()
                .first()
                .map(|m| m.object_path.clone())
                .unwrap_or_default();
            let msg = match communications::dial_call(&path, &number) {
                Ok(()) => format!("Dialing {number}..."),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &format!("UI(Voice): {msg}"));
                w.set_call_status_text(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_hangup_calls(move || {
            let path = cached_c
                .borrow()
                .first()
                .map(|m| m.object_path.clone())
                .unwrap_or_default();
            let msg = match communications::list_calls(&path) {
                Ok(calls) if !calls.is_empty() => {
                    for c in &calls {
                        let _ = communications::hangup_call(&c.path);
                    }
                    format!("Hung up {} call(s).", calls.len())
                }
                Ok(_) => "No active calls.".into(),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &format!("UI(Voice): {msg}"));
                w.set_call_status_text(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_refresh_calls(move || {
            let path = cached_c
                .borrow()
                .first()
                .map(|m| m.object_path.clone())
                .unwrap_or_default();
            let msg = match communications::list_calls(&path) {
                Ok(calls) if calls.is_empty() => "No active calls.".into(),
                Ok(calls) => calls
                    .into_iter()
                    .map(|c| {
                        format!(
                            "{} ({} • {})",
                            c.number,
                            c.direction_label(),
                            c.state_label()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &format!("UI(Voice): {msg}"));
                w.set_call_status_text(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let sel_c = selected_bands.clone();
        let app_weak = app.as_weak();
        app.on_select_modem(move |idx| {
            if let Some(w) = app_weak.upgrade() {
                w.set_selected_modem_idx(idx);
                let list = cached_c.borrow();
                sync_ui(&w, &list, &sel_c);
                append_debug(&w, &format!("UI: Switched active modem index to {idx}"));
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_select_uplink(move |id| {
            if let Some(w) = app_weak.upgrade() {
                w.set_selected_uplink_id(id.clone());
                append_debug(&w, &format!("UI(Hotspot): Selected uplink '{id}'"));
                w.set_path_verification_label("Unverified".into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_toggle_vpn_fail_closed(move |enable| {
            let st = wifi_ap::current_status();
            let vpn = crate::hotspot::uplink::inspect_vpn_path();
            let (is_err, msg) = match crate::hotspot::uplink::apply_vpn_fail_closed(
                &st.interface,
                &vpn.tunnel_iface,
                enable,
            ) {
                Ok(m) => (false, m),
                Err(e) => (true, e),
            };
            if let Some(w) = app_weak.upgrade() {
                if is_err {
                    w.set_vpn_fail_closed(false);
                } else {
                    w.set_vpn_fail_closed(enable);
                    if enable {
                        w.set_path_verification_label("Verified".into());
                    }
                }
                append_debug(&w, &format!("UI(FailClosed): {msg}"));
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_start_hotspot_configured(move |ssid, pass, band_idx, use_wpa3| {
            let band = match band_idx {
                0 => wifi_ap::HotspotBand::Auto,
                2 => wifi_ap::HotspotBand::A5Ghz,
                _ => wifi_ap::HotspotBand::Bg24Ghz,
            };
            let msg = match wifi_ap::start_hotspot(&ssid, &pass, band, None, use_wpa3) {
                Ok(m) => m,
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_hotspot_active(wifi_ap::current_status().active);
                append_debug(&w, &format!("UI(Hotspot): {msg}"));
                w.set_hotspot_status_text(msg.into());
            }
        });
    }

    {
        let client_c = client.clone();
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_read_vendor_telemetry(move || {
            let Some(ref c) = client_c else { return };
            let modems = cached_c.borrow();
            let sel_idx = app_weak
                .upgrade()
                .map(|w| w.get_selected_modem_idx().max(0) as usize)
                .unwrap_or(0);
            let Some(m) = modems.get(sel_idx).or_else(|| modems.first()) else {
                return;
            };
            let path = m.object_path.clone();
            if let Some(w) = app_weak.upgrade() {
                w.set_is_busy(true);
                w.set_busy_message("Reading serving band & CA telemetry…".into());
                append_debug(&w, "UI: Reading vendor serving band / CA telemetry");
            }
            let c_bg = c.clone();
            let weak = app_weak.clone();
            std::thread::spawn(move || {
                let res = c_bg.refresh_vendor_info(&path);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak.upgrade() {
                        w.set_is_busy(false);
                        w.set_busy_message("".into());
                        match res {
                            Ok(()) => {
                                append_debug(&w, "OK(VendorTelemetry): Serving band & CA updated");
                                w.invoke_refresh_clicked();
                            }
                            Err(e) => {
                                append_debug(&w, &format!("ERROR(VendorTelemetry): {e}"));
                                w.set_status_banner(e.into());
                            }
                        }
                    }
                });
            });
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_register_operator(move |code| {
            let modems = cached_c.borrow();
            let sel_idx = app_weak
                .upgrade()
                .map(|w| w.get_selected_modem_idx().max(0) as usize)
                .unwrap_or(0);
            let idx = modems
                .get(sel_idx)
                .and_then(|m| m.object_path.rsplit('/').next())
                .unwrap_or("0")
                .to_string();
            let msg = match scanner::register_manual(&idx, &code) {
                Ok(()) => format!("Registered manually on operator {code}"),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(&w, &format!("UI(RegisterOp): {msg}"));
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_apply_failover(move |idx| {
            let policy = match idx {
                1 => failover::FailoverPolicy::PreferEthernet,
                2 => failover::FailoverPolicy::PreferCellular,
                _ => failover::FailoverPolicy::PreferWifi,
            };
            let msg = failover::apply_failover_policy(policy).unwrap_or_else(|e| e);
            if let Some(w) = app_weak.upgrade() {
                w.set_failover_status_text(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_run_network_scan(move || {
            if let Some(w) = app_weak.upgrade() {
                w.set_is_busy(true);
                w.set_busy_message(i18n::tr("pro.scanner").into());
                append_debug(&w, "UI: 3GPP Network Scan started");
            }
            let sel_idx = app_weak
                .upgrade()
                .map(|w| w.get_selected_modem_idx().max(0) as usize)
                .unwrap_or(0);
            let idx = {
                let list = cached_c.borrow();
                list.get(sel_idx)
                    .or_else(|| list.first())
                    .and_then(|m| m.object_path.rsplit('/').next())
                    .unwrap_or("0")
                    .to_string()
            };
            let weak = app_weak.clone();
            std::thread::spawn(move || {
                let (is_err, text, rows) = match scanner::run_scan(&idx, "") {
                    Ok(res) => {
                        let rows: Vec<ScanNetworkRow> = res
                            .networks
                            .iter()
                            .map(|n| ScanNetworkRow {
                                operator: n.operator_name.clone().into(),
                                code: n.operator_code.clone().into(),
                                tech: n.technology.clone().into(),
                                availability: n.availability.label().into(),
                            })
                            .collect();
                        let summary = res
                            .networks
                            .into_iter()
                            .map(|n| {
                                format!(
                                    "{} ({}) • {} • {}",
                                    n.operator_name,
                                    n.operator_code,
                                    n.technology,
                                    n.availability.label()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        (false, summary, rows)
                    }
                    Err(e) => (true, e, Vec::new()),
                };
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak.upgrade() {
                        w.set_is_busy(false);
                        w.set_busy_message("".into());
                        append_debug(
                            &w,
                            &format!(
                                "{}: {}",
                                if is_err { "ERROR(Scan)" } else { "OK(Scan)" },
                                if is_err { &text } else { "completed" }
                            ),
                        );
                        w.set_scan_results(ModelRc::new(VecModel::from(rows)));
                        w.set_scanner_output_text(text.into());
                    }
                });
            });
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_register_auto_network(move || {
            let idx = mm_idx(&cached_c.borrow());
            let (is_err, msg) = match scanner::register_automatic(&idx) {
                Ok(()) => (false, "Returned to automatic network registration.".into()),
                Err(e) => (true, e),
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(
                    &w,
                    &format!(
                        "{}: {}",
                        if is_err {
                            "ERROR(AutoReg)"
                        } else {
                            "OK(AutoReg)"
                        },
                        msg
                    ),
                );
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_set_network_mode(move |mode| {
            let idx = mm_idx(&cached_c.borrow());
            let preset = match mode {
                3 => netmode::NetworkModePreset::Only4G,
                6 => netmode::NetworkModePreset::Preferred5G,
                _ => netmode::NetworkModePreset::Automatic,
            };
            let (is_err, msg) = match netmode::apply_network_mode(&idx, preset) {
                Ok(()) => (false, format!("Applied mode: {}", preset.label())),
                Err(e) => (true, e),
            };
            if let Some(w) = app_weak.upgrade() {
                append_debug(
                    &w,
                    &format!(
                        "{}: {}",
                        if is_err {
                            "ERROR(NetMode)"
                        } else {
                            "OK(NetMode)"
                        },
                        msg
                    ),
                );
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let sess_c = antenna_session.clone();
        let app_weak = app.as_weak();
        app.on_record_antenna_pos(move |label| {
            let modems = cached_c.borrow();
            let (rsrp, rsrq, sinr) = modems
                .first()
                .map(|m| {
                    let (_, s) = m.preferred_snapshot();
                    (s.signal.rsrp, s.signal.rsrq, s.signal.snr)
                })
                .unwrap_or((None, None, None));
            let mut sess = sess_c.borrow_mut();
            let lbl = if label.trim().is_empty() {
                "Position"
            } else {
                label.trim()
            };
            sess.record(lbl, rsrp, rsrq, sinr);
            let best = sess
                .best_position()
                .map(|b| format!("{} ({:.1})", b.label, b.score))
                .unwrap_or_else(|| "—".into());
            if let Some(w) = app_weak.upgrade() {
                w.set_antenna_output_text(
                    format!(
                        "Recorded {} position(s) • Best: {best}",
                        sess.positions.len()
                    )
                    .into(),
                );
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_run_at_command(move |cmd, allow_write| {
            let port = cached_c
                .borrow()
                .first()
                .and_then(|m| m.at_ports.first().cloned())
                .unwrap_or_default();
            let out = if port.is_empty() {
                "No AT port exposed by this modem.".into()
            } else {
                terminal::execute_at_command(&port, cmd.as_str(), allow_write, 1500)
                    .map(|r| terminal::sanitize_at_output(&r))
                    .unwrap_or_else(|e| e)
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_at_output_text(out.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_export_backup(move || {
            let dest = downloads_dir().join("modemdeck-backup.json");
            let msg = match backup::export_backup(&dest) {
                Ok(()) => format!("Backup exported to {}", dest.display()),
                Err(e) => e,
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let cached_c = cached_modems.clone();
        let app_weak = app.as_weak();
        app.on_export_hw_report(move || {
            let modems = cached_c.borrow();
            let Some(m) = modems.first() else { return };
            let rep = backup::build_privacy_safe_hw_report(m, "User-verified export");
            let dest = downloads_dir().join("modemdeck-hw-report.json");
            let msg = match std::fs::write(
                &dest,
                serde_json::to_string_pretty(&rep).unwrap_or_default(),
            ) {
                Ok(()) => format!("Saved HW report to {}", dest.display()),
                Err(e) => e.to_string(),
            };
            if let Some(w) = app_weak.upgrade() {
                w.set_status_banner(msg.into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_save_preferences(
            move |close_to_tray, autostart, bg_mon, notifs, reduced_motion, remote_srv| {
                let mut s = Settings::load();
                let prev_bg = s.background_monitor;
                s.close_to_tray = close_to_tray;
                s.start_with_system = autostart;
                s.background_monitor = bg_mon;
                s.notifications = notifs;
                s.reduced_motion = reduced_motion;
                s.remote_management_enabled = remote_srv;
                let _ = s.save();
                let _ = tray::set_autostart_enabled(autostart);
                if prev_bg != bg_mon {
                    let _ = settings::set_background_monitor_enabled(bg_mon);
                }
                if let Some(w) = app_weak.upgrade() {
                    apply_settings_to_slint(&w, &s);
                    w.set_status_banner("Preferences saved.".into());
                }
            },
        );
    }

    app.on_quit_app(|| {
        let _ = slint::quit_event_loop();
    });

    if start_minimized && tray_host_detected {
        let _ = app.run();
    } else {
        let _ = app.show();
        let _ = app.run();
    }

    if let Some(ref c) = client {
        c.restore_signal_updates(settings.normalized_refresh_seconds());
    }
    0
}

fn apply_settings_to_slint(app: &AppWindow, s: &Settings) {
    let theme = app.global::<Theme>();
    theme.set_mode(match s.theme.as_str() {
        "light" => AppThemeMode::Light,
        "high-contrast" => AppThemeMode::HighContrast,
        _ => AppThemeMode::Dark,
    });
    theme.set_reduced_motion(s.reduced_motion);
    theme.set_is_rtl(i18n::is_rtl());
    theme.set_font_scale(match s.font_size_preset.as_str() {
        "small" => 0.9,
        "large" => 1.15,
        "xlarge" => 1.3,
        _ => 1.0,
    });
    app.set_font_size_preset(s.font_size_preset.clone().into());

    app.set_complexity(match s.ui_mode.as_str() {
        "basic" => UiComplexityMode::Basic,
        "expert" => UiComplexityMode::Expert,
        _ => UiComplexityMode::Advanced,
    });
    app.set_pref_close_to_tray(s.close_to_tray);
    app.set_pref_start_with_system(s.start_with_system);
    app.set_pref_bg_monitor(s.background_monitor);
    app.set_pref_notifications(s.notifications);
    app.set_pref_reduced_motion(s.reduced_motion);
    app.set_pref_remote_server(s.remote_management_enabled);
    app.set_pref_language(s.language.clone().into());

    // Live-update all UI translations in Slint Tr global from Single Source of Truth (i18n::tr)
    let tr = app.global::<Tr>();
    let t = |key: &str| SharedString::from(i18n::tr(key));
    tr.set_home_sharing(t("home.sharing"));
    tr.set_home_route(t("home.route"));
    tr.set_home_manage_sharing(t("home.manage_sharing"));
    tr.set_scan_live_title(t("scan.live_title"));
    tr.set_scan_live_subtitle(t("scan.live_subtitle"));
    tr.set_scan_impact(t("scan.impact"));
    tr.set_scan_start(t("scan.start"));
    tr.set_scan_confirm(t("scan.confirm"));
    tr.set_scan_confirm_impact(t("scan.confirm_impact"));
    tr.set_scan_auto(t("scan.auto"));
    tr.set_scan_db_note(t("scan.db_note"));
    tr.set_scan_db_title(t("scan.db_title"));
    tr.set_scan_manual_note(t("scan.manual_note"));
    tr.set_hotspot_source(t("hotspot.source"));
    tr.set_hotspot_source_note(t("hotspot.source_note"));
    tr.set_hotspot_vpn_warning(t("hotspot.vpn_warning"));
    tr.set_hotspot_clients(t("hotspot.clients"));
    tr.set_failover_priority_note(t("failover.priority_note"));
    tr.set_modem_selection_unavailable(t("modem.selection_unavailable"));
    tr.set_analyzer_networks_prompt(t("analyzer.networks_prompt"));
    tr.set_analyzer_net_3g(SharedString::from("3G"));
    tr.set_analyzer_net_4g(SharedString::from("4G LTE"));
    tr.set_analyzer_net_5g(SharedString::from("5G NR"));
    tr.set_analyzer_net_all(t("analyzer.networks_all"));
    tr.set_btn_debug(t("btn.debug"));
    tr.set_debug_title(t("debug.title"));
    tr.set_debug_clear(t("debug.clear"));
    tr.set_debug_copy(t("debug.copy"));
    tr.set_status_please_wait(t("status.please_wait"));
    tr.set_nav_overview(t("nav.overview"));
    tr.set_nav_network(t("nav.network"));
    tr.set_nav_analyzer(t("nav.analyzer"));
    tr.set_nav_operators(t("nav.operators"));
    tr.set_nav_sim(t("nav.sim"));
    tr.set_nav_messages(t("nav.messages"));
    tr.set_nav_gps(t("nav.gps"));
    tr.set_nav_doctor(t("nav.doctor"));
    tr.set_nav_hardware(t("nav.hardware"));
    tr.set_nav_hotspot(t("nav.hotspot"));
    tr.set_nav_history(t("nav.history"));
    tr.set_nav_pro(t("nav.pro"));
    tr.set_nav_settings(t("nav.settings"));
    tr.set_btn_refresh(t("btn.refresh"));
    tr.set_btn_basic(t("mode.basic"));
    tr.set_btn_advanced(t("mode.advanced"));
    tr.set_btn_expert(t("mode.expert"));
    tr.set_btn_dark(t("theme.dark"));
    tr.set_btn_light(t("theme.light"));
    tr.set_btn_contrast(t("theme.contrast"));
    tr.set_lbl_rf_metrics(t("overview.rf_metrics"));
    tr.set_lbl_cell_traffic(t("overview.cell_traffic"));
    tr.set_lbl_operator(t("lbl.operator"));
    tr.set_lbl_download(t("lbl.download"));
    tr.set_lbl_upload(t("lbl.upload"));
    tr.set_lbl_today(t("lbl.today"));
    tr.set_lbl_month(t("lbl.this_month"));
    tr.set_lbl_band_lock(t("lbl.band_lock"));
    tr.set_btn_speed_test(t("btn.run_speedtest"));
    tr.set_btn_reset_auto(t("btn.reset_auto"));
    tr.set_lbl_band_control(t("bands.control_title"));
    tr.set_lbl_supported_bands(t("bands.supported_title"));
    tr.set_btn_apply_bands(t("btn.apply_bands"));
    tr.set_btn_save_lock(t("btn.save_lock"));
    tr.set_btn_keep(t("btn.keep"));
    tr.set_btn_rollback(t("btn.rollback"));
    tr.set_lbl_analyzer(t("analyzer.title"));
    tr.set_btn_scan_5m(t("btn.scan_5m"));
    tr.set_btn_scan_30s(t("btn.scan_30s"));
    tr.set_btn_cancel(t("btn.cancel"));
    tr.set_lbl_speedtest(t("speedtest.title"));
    tr.set_btn_run_speedtest(t("btn.run_speedtest"));
    tr.set_lbl_op_center(t("op.title"));
    tr.set_btn_create_apn(t("btn.create_apn"));
    tr.set_lbl_sim_sec(t("sim.title"));
    tr.set_btn_unlock_pin(t("btn.unlock_pin"));
    tr.set_lbl_apn_profiles(t("apn.title"));
    tr.set_btn_activate(t("btn.activate"));
    tr.set_lbl_sms(t("sms.title"));
    tr.set_btn_send_sms(t("btn.send_sms"));
    tr.set_lbl_ussd(t("ussd.title"));
    tr.set_btn_send_ussd(t("btn.send_ussd"));
    tr.set_lbl_gps(t("gps.title"));
    tr.set_lbl_cells(t("cells.title"));
    tr.set_btn_run_doctor(t("btn.run_doctor"));
    tr.set_btn_fix_safe(t("btn.fix_safe"));
    tr.set_lbl_netdiag(t("netdiag.title"));
    tr.set_lbl_hw(t("hw.title"));
    tr.set_lbl_fw(t("fw.title"));
    tr.set_lbl_hotspot(t("hotspot.title"));
    tr.set_lbl_failover(t("failover.title"));
    tr.set_lbl_history(t("history.title"));
    tr.set_lbl_scanner(t("pro.scanner"));
    tr.set_lbl_antenna(t("pro.antenna"));
    tr.set_lbl_at(t("pro.at"));
    tr.set_lbl_settings(t("settings.title"));
    tr.set_set_tray(t("settings.tray"));
    tr.set_set_autostart(t("settings.autostart"));
    tr.set_set_bg(t("settings.bg"));
    tr.set_set_notif(t("settings.notif"));
    tr.set_set_motion(t("settings.motion"));
    tr.set_set_remote(t("settings.remote"));
    tr.set_lbl_language(t("settings.language"));
    tr.set_btn_save_prefs(t("btn.save_prefs"));
    tr.set_btn_quit(t("btn.quit"));
    tr.set_lbl_about(t("about.title"));
    tr.set_sub_rf(t("overview.rf_sub"));
    tr.set_sub_band_ctrl(t("bands.control_sub"));
    tr.set_sub_band_chips(t("bands.supported_sub"));
    tr.set_sub_analyzer(t("analyzer.sub"));
    tr.set_sub_speedtest(t("speedtest.sub"));
    tr.set_sub_gps(t("gps.sub"));
    tr.set_sub_doctor(t("doctor.sub"));
    tr.set_banner_rollback(t("bands.confirm_banner"));
    tr.set_chart_title(t("overview.chart_title"));
    tr.set_lbl_country(t("lbl.country"));
    tr.set_lbl_confidence(t("lbl.confidence"));
    tr.set_lbl_ca(t("lbl.ca"));
    tr.set_lbl_sim_lock(t("sim.lock_lbl"));
    tr.set_lbl_sim_slot(t("sim.slot_lbl"));
    tr.set_ph_pin(t("sim.pin_ph"));
    tr.set_ph_phone(t("sms.num_ph"));
    tr.set_ph_sms(t("sms.body_ph"));
    tr.set_ph_antenna(t("pro.antenna_ph"));
    tr.set_empty_bands_t(t("bands.empty_title"));
    tr.set_empty_bands_s(t("bands.empty_sub"));
    tr.set_empty_apn_t(t("apn.empty_title"));
    tr.set_empty_apn_s(t("apn.empty_sub"));
    tr.set_empty_sms_t(t("sms.empty_title"));
    tr.set_empty_sms_s(t("sms.empty_sub"));
    tr.set_empty_doc_t(t("doctor.empty_title"));
    tr.set_empty_doc_s(t("doctor.empty_sub"));
    tr.set_btn_slot1(t("sim.slot1"));
    tr.set_btn_slot2(t("sim.slot2"));
    tr.set_btn_esim(t("btn.list_esim"));
    tr.set_btn_gps_on(t("gps.enable"));
    tr.set_btn_gps_off(t("gps.disable"));
    tr.set_btn_gps_read(t("gps.read"));
    tr.set_btn_netdiag(t("btn.run_netdiag"));
    tr.set_btn_backup(t("btn.export_backup"));
    tr.set_btn_hw_rep(t("btn.export_hw"));
    tr.set_btn_hs_on(t("hotspot.enable"));
    tr.set_btn_hs_off(t("hotspot.disable"));
    tr.set_btn_wifi(t("failover.wifi"));
    tr.set_btn_eth(t("failover.eth"));
    tr.set_btn_cell(t("failover.cell"));
    tr.set_btn_scan_net(t("pro.scan_btn"));
    tr.set_btn_auto_reg(t("pro.auto_reg"));
    tr.set_btn_rec_pos(t("pro.record_pos"));
    tr.set_btn_run_at(t("pro.run_at"));
    tr.set_lbl_at_write(t("pro.at_write"));
    tr.set_sub_at_write(t("pro.at_write_sub"));
    tr.set_sub_tray(t("settings.tray_sub"));
    tr.set_sub_autostart(t("settings.autostart_sub"));
    tr.set_sub_bg(t("settings.bg_sub"));
    tr.set_sub_notif(t("settings.notif_sub"));
    tr.set_sub_motion(t("settings.motion_sub"));
    tr.set_sub_remote(t("settings.remote_sub"));
}

fn detected_serving_band_label(modem: &ModemInfo) -> Option<String> {
    modem
        .vendor
        .nr_serving_band
        .map(|band| format!("n{band}"))
        .or_else(|| modem.vendor.serving_band.map(|band| format!("B{band}")))
        .or_else(|| {
            modem
                .detected_serving_nr_band()
                .map(|band| format!("n{band}"))
        })
        .or_else(|| {
            modem
                .detected_serving_lte_band()
                .map(|band| format!("B{band}"))
        })
        .or_else(|| {
            modem
                .vendor
                .earfcn
                .and_then(lte_band_from_earfcn)
                .map(|band| format!("B{band}"))
        })
}

fn modem_is_in_automatic_band_mode(modem: &ModemInfo) -> bool {
    if modem.current_bands.is_empty() || modem.current_bands.contains(&256) {
        return true;
    }

    // ModemManager versions and modem drivers differ: automatic selection may
    // be exposed either as the Auto enum or as the complete supported-band set.
    let normalize = |bands: &[u32]| {
        let mut bands: Vec<_> = bands
            .iter()
            .copied()
            .filter(|band| *band != 0 && *band != 256)
            .collect();
        bands.sort_unstable();
        bands.dedup();
        bands
    };
    let current = normalize(&modem.current_bands);
    let supported = normalize(&modem.supported_bands);
    !supported.is_empty() && current == supported
}

fn sync_ui(app: &AppWindow, modems: &[ModemInfo], selected_bands: &Rc<RefCell<Vec<u32>>>) {
    let op_db = OperatorDatabase::global();
    let hw_db = HardwareDatabase::global();
    let laptop = LaptopInfo::detect();

    app.set_has_modem(!modems.is_empty());
    let names: Vec<SharedString> = modems.iter().map(|x| x.display_name().into()).collect();
    app.set_modem_names(ModelRc::new(VecModel::from(names)));
    let sel_idx =
        (app.get_selected_modem_idx().max(0) as usize).min(modems.len().saturating_sub(1));
    let selected_modem_opt = modems.get(sel_idx);

    // Sync Hotspot Uplink, VPN, AP Capabilities & Clients
    let (def_iface, uplinks) = crate::hotspot::uplink::detect_uplinks(selected_modem_opt);
    let uplink_rows: Vec<UplinkItem> = uplinks
        .into_iter()
        .map(|u| UplinkItem {
            id: u.id.into(),
            kind: u.kind.into(),
            name: u.name.into(),
            interface: u.interface.into(),
            state: u.state.into(),
            health: u.health.into(),
            note: u.note.into(),
            available: u.available,
        })
        .collect();
    app.set_uplink_items(ModelRc::new(VecModel::from(uplink_rows)));

    let vpn = crate::hotspot::uplink::inspect_vpn_path();
    app.set_vpn_detected(vpn.detected);
    app.set_vpn_summary_text(vpn.summary.into());

    let wifi_caps = wifi_ap::detect_capabilities();
    app.set_ap_wifi_device(
        wifi_caps
            .wifi_device
            .clone()
            .unwrap_or_else(|| "—".into())
            .into(),
    );
    app.set_ap_5g_supported(wifi_caps.band_5ghz_supported);
    app.set_ap_wpa3_supported(wifi_caps.wpa3_supported);
    app.set_ap_sta_ap_concurrent(wifi_caps.concurrent_sta_ap);
    app.set_ap_wifi_active(wifi_caps.active_wifi_connected);
    app.set_ap_cap_note(wifi_caps.status_note.into());

    let hs = wifi_ap::current_status();
    app.set_hotspot_active(hs.active);
    app.set_hotspot_clients_text(hs.connected_clients.len().to_string().into());
    app.set_hotspot_traffic_text(
        format!(
            "{} RX / {} TX (aggregate interface)",
            traffic::human_bytes(hs.rx_bytes),
            traffic::human_bytes(hs.tx_bytes)
        )
        .into(),
    );
    let client_rows: Vec<HotspotClientItem> = hs
        .client_entries
        .into_iter()
        .map(|c| HotspotClientItem {
            mac: c.mac.into(),
            ip: c.ip.into(),
            hostname: c.hostname.into(),
        })
        .collect();
    app.set_hotspot_clients(ModelRc::new(VecModel::from(client_rows)));
    let ap_if = if hs.interface.is_empty() {
        "AP: inactive".into()
    } else {
        format!("AP: {}", hs.interface)
    };
    let up_if = if def_iface.is_empty() {
        "No default route".into()
    } else {
        format!("Uplink: {def_iface}")
    };
    app.set_active_path_diagram(
        format!("[Clients] ──▶ [{ap_if}] ──▶ [{up_if}] ──▶ [Internet]").into(),
    );

    let Some(m) = selected_modem_opt else {
        app.set_is_connected(false);
        app.set_supports_5g(false);
        app.set_has_slot2(false);
        app.set_has_esim(false);
        app.set_modem_title(i18n::tr("status.no_modem").into());
        app.set_modem_subtitle(i18n::tr("status.no_modem_sub").into());
        app.set_connection_state(i18n::tr("status.offline").into());
        app.set_network_tech("—".into());
        app.set_signal_percent(0);
        app.set_op_db_coverage(
            format!(
                "{} operators in {} countries",
                op_db.operator_count(),
                op_db.country_count()
            )
            .into(),
        );
        return;
    };

    let state_raw = m.state_label();
    let is_conn = m.state == 11 || state_raw == "Connected";
    let supports_5g =
        (m.current_capabilities & (1 << 5) != 0) || m.supported_bands.iter().any(|b| *b >= 301);
    app.set_is_connected(is_conn);
    app.set_supports_5g(supports_5g);
    app.set_has_slot2(m.sim_slots.len() > 1);
    app.set_has_esim(esim::detect_esim_capability(m).hardware_capable == Some(true));
    let hotspot_status = crate::hotspot::wifi_ap::current_status();
    app.set_hotspot_active(hotspot_status.active);
    app.set_hotspot_clients_text(
        format!("{} connected", hotspot_status.connected_clients.len()).into(),
    );
    app.set_hotspot_uplink_text(
        if hotspot_status.active {
            "System-selected route (source not verified)"
        } else {
            "Not sharing"
        }
        .into(),
    );

    app.set_modem_title(m.display_name().into());
    app.set_modem_subtitle(
        format!(
            "{} • {}",
            m.operator_label(),
            i18n::translate_ui_text(state_raw)
        )
        .into(),
    );
    app.set_operator_label(m.operator_label().into());
    let net_lbl = m.network_label();
    let effective_net = if net_lbl == "—" && m.lte_snapshot.signal.rsrp.is_some() {
        "4G".to_string()
    } else {
        net_lbl
    };
    app.set_network_tech(effective_net.into());
    app.set_connection_state(i18n::translate_ui_text(state_raw).into());

    let (_, snap) = m.preferred_snapshot();
    let tech = if m.is_5g_active() {
        RadioTech::Nr5g
    } else {
        RadioTech::Lte
    };
    let score = evaluate_signal(tech, snap, &[]);
    let effective_pct = if m.signal_percent == 0 {
        score.score.unwrap_or(0) as i32
    } else {
        m.signal_percent as i32
    };
    app.set_signal_percent(effective_pct);
    app.set_radio_score_label(
        format!(
            "{} ({}/100)",
            score.grade.label(),
            score
                .score
                .map(|v| v.to_string())
                .unwrap_or_else(|| "—".into())
        )
        .into(),
    );
    app.set_rsrp_val(fmt_opt(snap.signal.rsrp, "dBm").into());
    app.set_rsrq_val(fmt_opt(snap.signal.rsrq, "dB").into());
    app.set_sinr_val(fmt_opt(snap.signal.snr, "dB").into());
    app.set_rssi_val(fmt_opt(snap.signal.rssi, "dBm").into());
    app.set_rsrp_quality(rate_rsrp(snap.signal.rsrp).into());
    app.set_rsrq_quality(rate_rsrq(snap.signal.rsrq).into());
    app.set_sinr_quality(rate_sinr(snap.signal.snr).into());
    app.set_rssi_quality(rate_rssi(snap.signal.rssi).into());
    let automatic_bands = modem_is_in_automatic_band_mode(m);
    let single_locked = if !automatic_bands && m.current_bands.len() == 1 {
        Some(band_label(m.current_bands[0]))
    } else {
        None
    };
    let serving_band = detected_serving_band_label(m);
    app.set_serving_band(
        serving_band
            .clone()
            .or(single_locked)
            .unwrap_or_else(|| "Unknown".into())
            .into(),
    );
    app.set_earfcn_val(
        m.vendor
            .earfcn
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
            .into(),
    );
    app.set_pci_val(
        m.vendor
            .pci
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
            .into(),
    );
    app.set_ca_status(
        if m.carrier_aggregation_active() {
            m.ca_layout_label()
        } else {
            "Inactive".into()
        }
        .into(),
    );
    app.set_ca_details_text(ca_scan::analyze_ca(m).mode_label.into());

    if let Some(iface) = traffic::infer_interface(&m.net_ports)
        && let Some(t) = traffic::sample(&iface)
    {
        app.set_dl_speed(traffic::human_bps(t.rx_bps).into());
        app.set_ul_speed(traffic::human_bps(t.tx_bps).into());
        app.set_today_usage(traffic::human_bytes(t.today_rx + t.today_tx).into());
        app.set_month_usage(traffic::human_bytes(t.month_rx + t.month_tx).into());
    }

    let lock_store = persistent::load();
    if let Some(lk) = persistent::find_for_modem(&lock_store, m) {
        app.set_lock_status(format!("{} ({})", lk.mode.label(), lk.last_status.label()).into());
    } else {
        app.set_lock_status("Auto".into());
    }

    app.set_signal_chart_path(build_signal_svg_path(&m.stable_id()).into());

    // Band list
    if selected_bands.borrow().is_empty() && !m.current_bands.contains(&256) {
        *selected_bands.borrow_mut() = m.current_bands.clone();
    }
    update_band_model(app, m, &selected_bands.borrow());
    update_band_profiles_model(app);
    let band_str = if automatic_bands {
        format!(
            "Auto • Connected: {}",
            serving_band.unwrap_or_else(|| "band unavailable".into())
        )
    } else {
        m.current_bands
            .iter()
            .map(|b| band_label(*b))
            .collect::<Vec<_>>()
            .join(", ")
    };
    app.set_current_bands_text(band_str.into());

    // Operator match
    let matched = op_db.identify(
        &m.operator_code,
        &m.spn,
        &m.gid1,
        &m.registered_operator_code,
        &m.registered_operator_name,
        m.roaming,
        None,
    );
    app.set_op_confidence(matched.confidence.label().into());
    app.set_op_db_coverage(
        format!(
            "{} operators / {} countries",
            op_db.operator_count(),
            op_db.country_count()
        )
        .into(),
    );
    if let Some(ref op) = matched.sim_operator {
        app.set_country_label(op.country.clone().into());
        app.set_op_mcc_mnc(format!("{}-{}", op.mcc, op.mnc).into());
        app.set_op_mvno_info(
            if op.is_mvno {
                format!("MVNO (Host: {})", op.host_network)
            } else {
                "Primary MNO".into()
            }
            .into(),
        );
        let ussd_s = op
            .ussd_codes
            .iter()
            .map(|u| format!("{} ({})", u.code, u.description))
            .collect::<Vec<_>>()
            .join(" • ");
        app.set_op_ussd_text(
            if ussd_s.is_empty() {
                "—".into()
            } else {
                ussd_s
            }
            .into(),
        );
    }

    // APN list
    if let Ok(apns) = apn::list() {
        let items: Vec<ApnItem> = apns
            .into_iter()
            .map(|a| ApnItem {
                name: a.name.into(),
                apn: a.apn.into(),
                uuid: a.uuid.into(),
                active: a.active,
            })
            .collect();
        app.set_apn_items(ModelRc::new(VecModel::from(items)));
    }

    // SIM / eSIM / Hardware / Cells / History
    app.set_sim_lock_label(m.sim_lock_label().into());
    app.set_sim_slot_label(format!("Slot {}", m.primary_sim_slot.max(1)).into());
    app.set_esim_status_text(esim::detect_esim_capability(m).reason.into());
    app.set_fcc_status_text(fcc::inspect_fcc_lock(m).state.label().into());

    let cells_list = cells::summarize_cells(m);
    app.set_cells_summary_text(
        if cells_list.is_empty() {
            "No serving cell details cached yet.".into()
        } else {
            cells_list
                .iter()
                .map(|c| {
                    format!(
                        "{} • Cell {} • PCI {} • {}",
                        c.technology, c.cell_id, c.pci, c.band_label
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        .into(),
    );

    let hw_match = hw_db.lookup(
        &m.manufacturer,
        &m.model,
        m.usb_vid,
        m.usb_pid,
        m.pci_vid,
        m.pci_pid,
    );
    app.set_hw_host_val(laptop.display_label().into());
    app.set_hw_driver_val(m.kernel_driver.clone().into());
    app.set_hw_match_val(
        hw_match
            .map(|e| format!("{} ({})", e.commercial_name, e.verification_status.label()))
            .unwrap_or_else(|| "Generic cellular modem".into())
            .into(),
    );
    app.set_hw_details_text(
        format!(
            "Host: {} | DB Models: {} | Driver: {} | Match: {}",
            laptop.display_label(),
            hw_db.total_models(),
            m.kernel_driver,
            hw_match
                .map(|e| format!("{} ({})", e.commercial_name, e.verification_status.label()))
                .unwrap_or_else(|| "Generic cellular modem".into())
        )
        .into(),
    );
    let bus = rf_lab::inspect_bus_link(m);
    let therm = rf_lab::inspect_thermal();
    app.set_hw_bus_val(format!("{} ({})", bus.bus_type, bus.speed_label).into());
    app.set_hw_pm_val(bus.runtime_pm.clone().into());
    app.set_hw_thermal_val(therm.throttling_risk.clone().into());
    app.set_bus_thermal_text(
        format!(
            "Bus: {} ({}) • PM: {} • Thermal: {}",
            bus.bus_type, bus.speed_label, bus.runtime_pm, therm.throttling_risk
        )
        .into(),
    );
    let fw = firmware::inspect_firmware(m);
    app.set_fw_ver_val(fw.current_version.clone().into());
    app.set_fw_upd_val(if fw.fwupd_available { "Yes" } else { "No" }.into());
    app.set_fw_lvfs_val(
        if fw.lvfs_updates_available.is_empty() {
            "None".into()
        } else {
            fw.lvfs_updates_available.join(", ")
        }
        .into(),
    );
    app.set_fw_details_text(
        format!(
            "Firmware: {} | fwupd: {} | LVFS: {}",
            fw.current_version,
            fw.fwupd_available,
            if fw.lvfs_updates_available.is_empty() {
                "None".into()
            } else {
                fw.lvfs_updates_available.join(", ")
            }
        )
        .into(),
    );

    let hist = history_db::query(
        TimeRange::Last24Hours,
        &HistoryFilter {
            modem_uid: Some(m.stable_id()),
            ..HistoryFilter::default()
        },
    );
    if !hist.is_empty() {
        let min_s = hist.iter().map(|r| r.signal_percent).min().unwrap_or(0);
        let max_s = hist.iter().map(|r| r.signal_percent).max().unwrap_or(0);
        let avg_s =
            hist.iter().map(|r| r.signal_percent as u64).sum::<u64>() / (hist.len() as u64).max(1);
        app.set_chart_stats_text(
            format!(
                "Min {min_s}% • Avg {avg_s}% • Max {max_s}% ({} pts)",
                hist.len()
            )
            .into(),
        );
    }
    app.set_history_summary_text(
        format!("{} telemetry samples recorded in last 24h.", hist.len()).into(),
    );
}

fn update_band_profiles_model(app: &AppWindow) {
    let store = profiles::load();
    let items: Vec<BandProfileItem> = store
        .profiles
        .into_iter()
        .map(|p| {
            let btxt = if p.bands.is_empty() || p.bands.contains(&256) {
                "Auto".to_string()
            } else {
                p.bands
                    .iter()
                    .map(|b| band_label(*b))
                    .collect::<Vec<_>>()
                    .join("+")
            };
            let active = p.name == store.active_profile;
            BandProfileItem {
                name: p.name.into(),
                bands_text: btxt.into(),
                built_in: p.built_in,
                active,
            }
        })
        .collect();
    app.set_band_profiles(ModelRc::new(VecModel::from(items)));
}

fn rate_rsrp(v: Option<f64>) -> String {
    let lbl = match v {
        None => "Unknown",
        Some(x) if x >= -80.0 => "Excellent",
        Some(x) if x >= -90.0 => "Very Good",
        Some(x) if x >= -100.0 => "Good",
        Some(x) if x >= -110.0 => "Fair",
        _ => "Poor",
    };
    i18n::translate_ui_text(lbl)
}

fn rate_rsrq(v: Option<f64>) -> String {
    let lbl = match v {
        None => "Unknown",
        Some(x) if x >= -8.0 => "Excellent",
        Some(x) if x >= -12.0 => "Good",
        Some(x) if x >= -16.0 => "Fair",
        _ => "Poor",
    };
    i18n::translate_ui_text(lbl)
}

fn rate_sinr(v: Option<f64>) -> String {
    let lbl = match v {
        None => "Unknown",
        Some(x) if x >= 20.0 => "Excellent",
        Some(x) if x >= 13.0 => "Very Good",
        Some(x) if x >= 5.0 => "Good",
        Some(x) if x >= 0.0 => "Fair",
        _ => "Poor",
    };
    i18n::translate_ui_text(lbl)
}

fn rate_rssi(v: Option<f64>) -> String {
    let lbl = match v {
        None => "Unknown",
        Some(x) if x >= -65.0 => "Excellent",
        Some(x) if x >= -75.0 => "Good",
        Some(x) if x >= -85.0 => "Fair",
        _ => "Poor",
    };
    i18n::translate_ui_text(lbl)
}

fn update_band_model(app: &AppWindow, m: &ModemInfo, selected: &[u32]) {
    let mut all = Vec::new();
    let mut b3g = Vec::new();
    let mut bfdd = Vec::new();
    let mut btdd = Vec::new();
    let mut b5g = Vec::new();

    for &b in m.supported_bands.iter().filter(|b| **b != 256 && **b != 0) {
        let info = classify_mm_band(b);
        let item = BandItem {
            mm_enum: b as i32,
            label: info.short_label.clone().into(),
            selected: selected.contains(&b),
        };
        all.push(item.clone());
        match info.category {
            BandCategory::Umts3G | BandCategory::Gsm2G => b3g.push(item),
            BandCategory::LteFdd => bfdd.push(item),
            BandCategory::LteTdd => btdd.push(item),
            BandCategory::Nr5g => b5g.push(item),
            _ => bfdd.push(item),
        }
    }
    app.set_band_items(ModelRc::new(VecModel::from(all)));
    app.set_bands_3g(ModelRc::new(VecModel::from(b3g)));
    app.set_bands_lte_fdd(ModelRc::new(VecModel::from(bfdd)));
    app.set_bands_lte_tdd(ModelRc::new(VecModel::from(btdd)));
    app.set_bands_5g(ModelRc::new(VecModel::from(b5g)));
}

pub(crate) fn build_signal_svg_path(modem_uid: &str) -> String {
    let records = history_db::query(
        TimeRange::Last24Hours,
        &HistoryFilter {
            modem_uid: Some(modem_uid.to_string()),
            ..HistoryFilter::default()
        },
    );
    let pts: Vec<f64> = records
        .iter()
        .rev()
        .take(40)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|r| r.signal_percent as f64)
        .collect();
    if pts.len() < 2 {
        return "M 0 80 L 900 80 L 900 100 L 0 100 Z".into();
    }
    let n = pts.len();
    let mut cmd = String::from("M 0 100 ");
    for (i, pct) in pts.iter().enumerate() {
        let x = 900.0 * (i as f64 / (n - 1) as f64);
        let y = 95.0 - (pct.clamp(0.0, 100.0) * 0.90);
        cmd.push_str(&format!("L {x:.1} {y:.1} "));
    }
    cmd.push_str("L 900 100 Z");
    cmd
}

fn fmt_opt(v: Option<f64>, unit: &str) -> String {
    v.map(|x| format!("{x:.1} {unit}"))
        .unwrap_or_else(|| "—".into())
}

fn mm_idx(modems: &[ModemInfo]) -> String {
    modems
        .first()
        .and_then(|m| m.object_path.rsplit('/').next().map(|s| s.to_string()))
        .unwrap_or_else(|| "0".into())
}

fn analyzer_bands_for_generation(supported_bands: &[u32], generation_mode: i32) -> Vec<u32> {
    let mut bands: Vec<u32> = supported_bands
        .iter()
        .copied()
        .filter(|b| *b >= 31 && *b != 256)
        .collect();
    bands.extend(
        supported_bands
            .iter()
            .copied()
            .filter(|b| *b > 0 && *b < 31),
    );
    bands.retain(|band| {
        let category = classify_mm_band(*band).category;
        match generation_mode {
            0 => category == BandCategory::Umts3G,
            1 => matches!(category, BandCategory::LteFdd | BandCategory::LteTdd),
            2 => category == BandCategory::Nr5g,
            _ => true,
        }
    });
    bands.sort_unstable();
    bands.dedup();
    bands
}

fn append_debug(app: &AppWindow, entry: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() % 86400)
        .unwrap_or(0);
    let hh = ts / 3600;
    let mm = (ts % 3600) / 60;
    let ss = ts % 60;
    let line = format!("[{hh:02}:{mm:02}:{ss:02}] {entry}");
    let prev = app.get_debug_log_text().to_string();
    let mut lines: Vec<&str> = prev.lines().collect();
    lines.push(&line);
    if lines.len() > 80 {
        lines.drain(0..(lines.len() - 80));
    }
    app.set_debug_log_text(lines.join("\n").into());
}

fn downloads_dir() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Downloads")
}

#[cfg(target_os = "linux")]
fn instance_socket_path() -> std::path::PathBuf {
    crate::core::runtime_status::runtime_dir().join("modemdeck-ui.sock")
}

#[cfg(target_os = "linux")]
fn signal_existing_instance() -> bool {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let sock = instance_socket_path();
    if let Ok(mut stream) = UnixStream::connect(&sock) {
        let _ = stream.write_all(b"SHOW\n");
        return true;
    }
    false
}

#[cfg(target_os = "linux")]
fn spawn_single_instance_listener<F>(on_show: F)
where
    F: Fn() + Send + 'static,
{
    use std::os::unix::net::UnixListener;
    let sock = instance_socket_path();
    if let Some(parent) = sock.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&sock);
    if let Ok(listener) = UnixListener::bind(&sock) {
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                drop(stream);
                on_show();
            }
        });
    }
}
