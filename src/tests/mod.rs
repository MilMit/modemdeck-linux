use crate::analyzer::{
    band_scan::{
        BandTestOutcome, build_band_result_from_samples, summarize_results, verify_serving_band,
    },
    ca_scan, comparison,
    signal_score::{ConfidenceLevel, RadioTech, SignalGrade, evaluate_signal},
    speed_test,
    stability::{ConnectivityProbe, StabilityTracker},
};
use crate::core::{profiles, safety, settings::Settings};
use crate::drivers::{DriverKind, dw5821e, fibocom, huawei, quectel, sierra, simcom, telit};
use crate::locking::{
    band_lock::{LockMode, LockPolicy, LockVerificationStatus, verify_band_lock},
    cell_lock,
    persistent::{self, PersistentLockEntry},
};
use crate::model::{
    BandCategory, ModemInfo, RadioSignal, SignalFreshness, SignalSnapshot, SimIdentity, band_label,
    classify_mm_band, lte_band_from_earfcn,
};
use crate::optimization::smart_select::{OptimizationProfile, recommend_best_band};
use crate::storage::history_db::{self, HistoryFilter, HistoryRecord, TimeRange};

#[test]
fn test_signal_scoring_strong_rsrp_poor_snr_is_not_high_quality() {
    // Prompt requirement: RSRP = -86 dBm + SNR = 1 dB must NOT be rated Good/Excellent
    let snap = SignalSnapshot {
        signal: RadioSignal {
            rsrp: Some(-86.0),
            rsrq: Some(-12.0),
            snr: Some(1.0),
            rssi: Some(-68.0),
        },
        updated_unix: 1_700_000_000,
        freshness: SignalFreshness::Fresh,
        context_key: "test".into(),
    };
    let report = evaluate_signal(RadioTech::Lte, &snap, &[]);
    let score = report.score.expect("score should exist");
    assert!(
        score <= 52,
        "Expected score <= 52 for SNR 1 dB, got {score}"
    );
    assert!(matches!(
        report.grade,
        SignalGrade::Fair | SignalGrade::Poor
    ));
    assert_eq!(report.confidence, ConfidenceLevel::High);
}

#[test]
fn test_signal_scoring_5g_excellent_and_unavailable() {
    let good_5g = SignalSnapshot {
        signal: RadioSignal {
            rsrp: Some(-78.0),
            rsrq: Some(-8.0),
            snr: Some(24.0),
            rssi: Some(-62.0),
        },
        updated_unix: 1_700_000_000,
        freshness: SignalFreshness::Fresh,
        context_key: "5g".into(),
    };
    let rep = evaluate_signal(RadioTech::Nr5g, &good_5g, &[]);
    assert!(rep.score.unwrap() >= 88);
    assert_eq!(rep.grade, SignalGrade::Excellent);

    let empty = SignalSnapshot::default();
    let rep_empty = evaluate_signal(RadioTech::Lte, &empty, &[]);
    assert_eq!(rep_empty.score, None);
    assert_eq!(rep_empty.grade, SignalGrade::Unknown);
}

#[test]
fn test_signal_freshness_transitions() {
    let snap = SignalSnapshot {
        signal: RadioSignal {
            rsrp: Some(-90.0),
            ..RadioSignal::default()
        },
        updated_unix: 1000,
        freshness: SignalFreshness::Fresh,
        context_key: "k".into(),
    };
    assert_eq!(snap.compute_freshness(1010, 15, 60), SignalFreshness::Fresh);
    assert_eq!(
        snap.compute_freshness(1030, 15, 60),
        SignalFreshness::Cached
    );
    assert_eq!(snap.compute_freshness(1100, 15, 60), SignalFreshness::Stale);
}

#[test]
fn test_dw5821e_parser_and_ca() {
    let debug = "\
BAND: 3
EARFCN(DL/UL): 1850/19850
BW: 20 MHz
MIMO: 2x2
MODULATION: 64QAM
eNB ID(PCI): 12345(42)";
    let ca = "\
PCC: LTE_B3 Band_width is 20.0 MHz EARFCN 1850 PCI 42
SCC1: LTE_B7 Band_width is 20.0 MHz EARFCN 3100 PCI 42";
    let info = dw5821e::parse(debug, ca);
    assert_eq!(info.serving_band, Some(3));
    assert_eq!(info.earfcn, Some(1850));
    assert_eq!(info.pci, Some(42));
    assert_eq!(info.bandwidth, "20 MHz");
    assert_eq!(info.mimo, "2x2");
    assert_eq!(info.modulation, "64QAM");
    assert_eq!(info.ca_active, Some(true));
    assert_eq!(info.ca_components.len(), 2);
}

#[test]
fn test_all_driver_parsers() {
    let q = quectel::parse(&[
        (
            "AT+QENG=\"servingcell\"".into(),
            "+QENG: \"servingcell\",\"NOCONN\",\"LTE\",\"FDD\",432,35,1A2B,42,1850,3,5,5,123,-90,-10,-60,15,0".into(),
        ),
        ("AT+QCAINFO".into(), "+QCAINFO: \"SCC\",3100,100,\"LTE BAND 7\"".into()),
    ]);
    assert_eq!(q.serving_band, Some(3));
    assert_eq!(q.ca_active, Some(true));

    let f = fibocom::parse(
        "FM350",
        &[("AT+GTCCINFO?".into(), "PCC:103,1850,42".into())],
    );
    assert_eq!(f.serving_band, Some(3));

    let s = sierra::parse(&[(
        "AT!GSTATUS?".into(),
        "LTE band: B7\nLTE Rx chan: 3100\nPCI: 88\nLTE bw: 20 MHz".into(),
    )]);
    assert_eq!(s.serving_band, Some(7));
    assert_eq!(s.pci, Some(88));

    let t = telit::parse(&[("AT#MONI".into(), "BAND: 42 EARFCN: 42590 PCI: 12".into())]);
    assert_eq!(t.serving_band, Some(42));

    let sc = simcom::parse(&[("AT+CPSI?".into(), "LTE BAND 3".into())]);
    assert_eq!(sc.serving_band, Some(3));

    let h = huawei::parse(&[("AT^SYSINFOEX".into(), "OK".into())]);
    assert!(h.deep_read_available);
}

#[test]
fn test_band_classification_and_earfcn() {
    assert_eq!(lte_band_from_earfcn(42590), Some(42));
    assert_eq!(band_label(72), "LTE B42");
    let b42 = classify_mm_band(72);
    assert_eq!(b42.category, BandCategory::LteTdd);
    assert_eq!(b42.number, Some(42));
    let n78 = classify_mm_band(378);
    assert_eq!(n78.category, BandCategory::Nr5g);
    assert_eq!(n78.number, Some(78));
}

#[test]
fn test_driver_capability_matrix_and_cell_lock() {
    let dw = DriverKind::identify("Dell Inc.", "DW5821e Snapdragon X20 LTE");
    assert_eq!(dw, DriverKind::Dw5821e);
    let matrix = dw.capability_matrix("DW5821e");
    assert!(!matrix.protocols.is_empty());
    let preview = dw.cell_lock_preview(1850, 42).unwrap();
    assert!(preview.contains("pci_lock"));
    let cap = cell_lock::inspect_capability("Dell", "DW5821e");
    assert!(cap.preview_supported);
    assert!(!cap.execute_supported);
}

#[test]
fn test_persistent_lock_and_verification() {
    let mut modem = ModemInfo {
        device_uid: "pci-0000:00:14.0".into(),
        state: 11,
        current_bands: vec![33], // LTE B3
        ..ModemInfo::default()
    };
    modem.vendor.serving_band = Some(3);
    assert_eq!(
        verify_band_lock(&[33], &modem),
        LockVerificationStatus::LockedAndVerified
    );
    modem.vendor.serving_band = Some(7);
    assert_eq!(
        verify_band_lock(&[33], &modem),
        LockVerificationStatus::ConfiguredUnverified
    );
    assert_eq!(
        verify_band_lock(&[37], &modem),
        LockVerificationStatus::RestorePending
    );

    let mut entry = PersistentLockEntry {
        modem_uid: modem.stable_id(),
        sim_hash: "sim-1".into(),
        operator_code: "43235".into(),
        mode: LockMode::PersistentLock,
        policy: LockPolicy::Safe,
        bands: vec![33],
        expires_unix: None,
        updated_unix: 0,
        last_status: LockVerificationStatus::LockedAndVerified,
        last_reason: String::new(),
        recent_restore_attempts: Vec::new(),
    };
    assert!(persistent::can_attempt_restore(&mut entry));
    assert!(persistent::can_attempt_restore(&mut entry));
    assert!(persistent::can_attempt_restore(&mut entry));
    assert!(!persistent::can_attempt_restore(&mut entry)); // anti-loop cap at 3
}

#[test]
fn test_operation_lock_mutual_exclusion() {
    let key = "test-modem-uid";
    assert!(
        safety::try_acquire_operation_lock(key, safety::ModemOperationOwner::BandAnalyzer).is_ok()
    );
    assert!(
        safety::try_acquire_operation_lock(key, safety::ModemOperationOwner::UiBandChange).is_err()
    );
    safety::release_operation_lock(key, safety::ModemOperationOwner::BandAnalyzer);
    assert!(
        safety::try_acquire_operation_lock(key, safety::ModemOperationOwner::UiBandChange).is_ok()
    );
    safety::release_operation_lock(key, safety::ModemOperationOwner::UiBandChange);
}

#[test]
fn test_band_analyzer_and_smart_selection() {
    let mut m1 = ModemInfo {
        state: 11,
        operator_name: "Irancell".into(),
        lte_snapshot: SignalSnapshot {
            signal: RadioSignal {
                rsrp: Some(-85.0),
                rsrq: Some(-9.0),
                snr: Some(18.0),
                rssi: Some(-65.0),
            },
            updated_unix: 1_700_000_000,
            freshness: SignalFreshness::Fresh,
            context_key: "test".into(),
        },
        ..ModemInfo::default()
    };
    m1.vendor.serving_band = Some(3);
    assert_eq!(verify_serving_band(33, &m1), Some(true));
    assert_eq!(verify_serving_band(37, &m1), Some(false));

    let s1 = vec![RadioSignal {
        rsrp: Some(-85.0),
        rsrq: Some(-9.0),
        snr: Some(18.0),
        rssi: Some(-65.0),
    }];
    let r1 = build_band_result_from_samples(
        33,
        &m1,
        &s1,
        0,
        0,
        100.0,
        Some(2.0),
        Some(1.0),
        Some(85.0),
        Some(25.0),
        Some(28.0),
        Some(3.0),
        Some(0.0),
        BandTestOutcome::Passed,
        true,
        "OK".into(),
    );
    let summary = summarize_results(std::slice::from_ref(&r1));
    assert_eq!(summary.best_overall.as_deref(), Some("LTE B3"));
    assert_eq!(summary.fastest_download.as_deref(), Some("LTE B3"));

    let rec = recommend_best_band(
        &[r1],
        OptimizationProfile::Gaming.weights(),
        ConfidenceLevel::Low,
    );
    assert!(rec.is_some());
}

#[test]
fn test_ca_analyzer_and_operator_lookup() {
    let mut modem = ModemInfo {
        manufacturer: "Dell".into(),
        model: "DW5821e".into(),
        ..ModemInfo::default()
    };
    modem.vendor.ca_active = Some(true);
    let ca = ca_scan::analyze_ca(&modem);
    assert!(ca.active);
    assert!(!ca.documented_combos.is_empty());

    assert_eq!(
        comparison::resolve_operator_display("43235", ""),
        "Irancell"
    );
    assert_eq!(
        comparison::resolve_operator_display("43293", ""),
        "SamanTel"
    );
}

#[test]
fn test_stability_tracker_and_sim_privacy() {
    let mut trk = StabilityTracker::default();
    let settings = Settings::default();
    let mut modem = ModemInfo {
        state: 11,
        operator_name: "Irancell".into(),
        ..ModemInfo::default()
    };
    assert!(trk.evaluate(&modem, None, &settings.stability).is_empty());
    modem.state = 3;
    let probe = ConnectivityProbe {
        dns_ok: false,
        internet_ok: false,
        latency_ms: None,
        packet_loss_percent: 100.0,
    };
    let alerts = trk.evaluate(&modem, Some(&probe), &settings.stability);
    assert!(alerts.len() >= 3);

    let sim = SimIdentity::from_raw(
        "/org/freedesktop/ModemManager1/SIM/0",
        1,
        "43235",
        "Irancell",
        "899801234567890",
    );
    assert!(sim.stable_hash.starts_with("sim-"));
    assert!(!sim.stable_hash.contains("899801234567890"));
}

#[test]
fn test_settings_migration_and_profiles_no_premature_save() {
    let mut s = Settings {
        schema_version: 1,
        theme: String::new(),
        history_retention_days: 0,
        speedtest_data_cap_mb: 0,
        ..Settings::default()
    };
    s.migrate();
    assert_eq!(
        s.schema_version,
        crate::core::settings::CURRENT_SETTINGS_SCHEMA
    );
    assert_eq!(s.theme, "system");
    assert_eq!(s.history_retention_days, 30);

    let est = speed_test::estimate_test_mb(true, true, 3, 25);
    assert_eq!(est, 25);
    let _ = profiles::default_profiles();
}

#[test]
fn test_history_filter_and_export() {
    let rec = HistoryRecord {
        timestamp: 1000,
        modem_uid: "m1".into(),
        sim_hash: "sim-a".into(),
        operator: "Irancell".into(),
        band: "B3".into(),
        signal_percent: 80,
        rsrp: Some(-88.0),
        ..HistoryRecord::default()
    };
    let tmp = std::env::temp_dir().join("modemdeck-test-hist.csv");
    history_db::export_csv(&[rec], &tmp).unwrap();
    let content = std::fs::read_to_string(&tmp).unwrap();
    assert!(content.contains("Irancell"));
    let _ = std::fs::remove_file(tmp);
    let _ = TimeRange::LastHour.bounds(2000);
    let _ = HistoryFilter::default();
}

#[test]
fn test_hardware_database_and_vid_pid_mapping() {
    let db = crate::hardware::HardwareDatabase::global();
    assert!(db.total_models() >= 50);

    let dw = db
        .lookup("Dell", "DW5821e", Some(0x413c), Some(0x81d7), None, None)
        .expect("DW5821e should exist in hardware DB");
    assert_eq!(dw.oem_manufacturer, "Foxconn");
    assert_eq!(
        dw.capabilities.internet_connection,
        crate::hardware::CompatStatus::HardwareVerified
    );

    let fm350 = db
        .lookup(
            "Fibocom",
            "FM350-GL",
            None,
            None,
            Some(0x14c3),
            Some(0x4d75),
        )
        .expect("FM350-GL should match PCI ID");
    assert_eq!(fm350.chipset, "MediaTek T700");

    let laptop = crate::hardware::LaptopInfo::detect();
    assert!(!laptop.recognized_family.is_empty());
}

#[test]
fn test_global_operator_database_mvno_and_xml_parser() {
    let db = crate::operators::OperatorDatabase::global();
    assert!(db.operator_count() >= 19);
    // The bundled registries cover eight countries. Additional countries may
    // come from the optional host mobile-broadband-provider-info XML database.
    assert!(db.country_count() >= 8);

    // Irancell exact match
    let ir = db.identify("432-35", "Irancell", "", "432-35", "Irancell", false, None);
    assert_eq!(
        ir.confidence,
        crate::operators::IdentificationConfidence::Exact
    );
    assert_eq!(ir.sim_operator.unwrap().name, "MTN Irancell");

    // MVNO Anarestan shares 432-11 with MCI; SPN disambiguates instead of collapsing to MCI
    let anar = db.identify("432-11", "Anarestan", "", "432-11", "MCI", false, None);
    assert_eq!(
        anar.confidence,
        crate::operators::IdentificationConfidence::MvnoMatched
    );
    assert_eq!(anar.sim_operator.unwrap().name, "Anarestan");

    // Ambiguous when multiple operators share 432-11 without SPN/GID1
    let amb = db.identify("432-11", "", "", "432-11", "", false, None);
    assert_eq!(
        amb.confidence,
        crate::operators::IdentificationConfidence::Ambiguous
    );

    // XML parser sanity check
    let sample_xml = r#"
<serviceproviders format="2.0">
<country code="no">
  <provider>
    <name>Telenor</name>
    <network-id mcc="242" mnc="01"/>
    <apn value="telenor.smart"/>
  </provider>
</country>
</serviceproviders>
"#;
    let parsed = crate::operators::parse_serviceproviders_xml(sample_xml);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].country, "Norway");
    assert_eq!(parsed[0].apns[0].apn, "telenor.smart");
}

#[test]
fn test_operator_db_integrity_and_esim_and_gps() {
    let payload = br#"{"schema_version":1,"operators":[{"id":"t1","name":"TestNet","mcc":"999","mnc":"01"}]}"#;
    let tag = crate::operators::compute_integrity_tag(payload);
    assert!(tag.starts_with("fnv1a64:"));
    assert!(crate::operators::install_operator_db_update(payload, Some("fnv1a64:bad")).is_err());

    // DW5821e physical SIM vs eSIM SKU
    let dw_phys = ModemInfo {
        model: "DW5821e".into(),
        usb_vid: Some(0x413c),
        usb_pid: Some(0x81d7),
        ..ModemInfo::default()
    };
    assert_eq!(
        crate::esim::detect_esim_capability(&dw_phys).hardware_capable,
        Some(false)
    );
    let dw_esim = ModemInfo {
        model: "DW5821e".into(),
        usb_vid: Some(0x413c),
        usb_pid: Some(0x81e0),
        ..ModemInfo::default()
    };
    assert_eq!(
        crate::esim::detect_esim_capability(&dw_esim).hardware_capable,
        Some(true)
    );
    assert!(crate::esim::validate_activation_code("LPA:1$smdp.example$ABC123").is_ok());
    assert!(crate::esim::validate_activation_code("invalid").is_err());

    // GPS GPX/CSV export
    let fix = crate::gps::GpsFix {
        enabled: true,
        has_fix: true,
        latitude: Some(35.6892),
        longitude: Some(51.3890),
        altitude_m: Some(1200.0),
        timestamp_unix: 1700000000,
        ..crate::gps::GpsFix::default()
    };
    assert!(crate::gps::export_gpx(std::slice::from_ref(&fix)).contains("35.6892"));
    assert!(crate::gps::export_csv(&[fix]).contains("51.389"));
}

#[test]
fn test_doctor_scanner_netmode_at_and_privacy_report() {
    let m = ModemInfo {
        manufacturer: "Dell".into(),
        model: "DW5821e".into(),
        current_capabilities: (1 << 2) | (1 << 3),
        ..ModemInfo::default()
    };
    let doc = crate::doctor::run_doctor(std::slice::from_ref(&m));
    assert!(!doc.checks.is_empty());

    let scanned = crate::scanner::parse_mmcli_scan_output(
        "43235 - MTN Irancell (lte, current)\n43211 - IR-MCI (lte, available)",
        "432",
    );
    assert_eq!(scanned.len(), 2);
    assert_eq!(
        scanned[0].availability,
        crate::scanner::NetworkAvailability::Current
    );
    assert!(!scanned[0].roaming);

    let presets = crate::netmode::supported_presets(&m);
    assert!(presets.contains(&crate::netmode::NetworkModePreset::Only4G));
    assert!(!presets.contains(&crate::netmode::NetworkModePreset::Only5G));

    assert_eq!(
        crate::terminal::classify_at_command("AT+CSQ"),
        crate::terminal::AtSafetyClass::ReadOnly
    );
    assert_eq!(
        crate::terminal::classify_at_command("AT+NVW=1"),
        crate::terminal::AtSafetyClass::DangerousBlocked
    );

    let rep = crate::backup::build_privacy_safe_hw_report(&m, "Test note");
    assert_eq!(rep.modem_model, "DW5821e");

    // Antenna positioning session
    let mut pos = crate::antenna::PositioningSession::default();
    pos.record("Window", Some(-82.0), Some(-8.0), Some(18.0));
    pos.record("Desk", Some(-105.0), Some(-14.0), Some(4.0));
    assert_eq!(pos.best_position().unwrap().label, "Window");
    assert!(pos.fluctuation_range().unwrap() > 10.0);

    // Internet diagnostics domestic vs international classification
    let probes = vec![
        crate::netdiag::EndpointProbe {
            label: "Dom".into(),
            category: "domestic".into(),
            host: "a.ir".into(),
            port: 443,
            dns_ms: Some(12),
            tcp_ms: Some(25),
            reachable: true,
            error: String::new(),
        },
        crate::netdiag::EndpointProbe {
            label: "Intl".into(),
            category: "international".into(),
            host: "1.1.1.1".into(),
            port: 443,
            dns_ms: Some(12),
            tcp_ms: None,
            reachable: false,
            error: "timeout".into(),
        },
    ];
    let (diag, _) = crate::netdiag::classify_probes(&probes, 75);
    assert_eq!(
        diag,
        crate::netdiag::ProbableDiagnosis::PossibleRegionalDisruption
    );

    let loc = crate::gps::parse_mmcli_location(
        "  ------------------------\n  3GPP location\n  ------------------------\n  GPS\n  latitude: 35.6892\n  longitude: 51.3890\n  altitude: 1200.5 m\n$GPGGA,123519,3541.352,N,05123.340,E,1,08,0.9,1200.5,M",
    );
    assert!(loc.has_fix);
    assert_eq!(loc.latitude, Some(35.6892));
    assert!(loc.nmea_raw.contains("$GPGGA"));

    // RF Lab engines
    let _ = crate::rf_lab::inspect_thermal();
    let _ = crate::rf_lab::inspect_bus_link(&m);
    let anom = crate::rf_lab::detect_cell_anomalies(&m);
    assert!(!anom.disclaimer.is_empty());
    let eff = crate::rf_lab::evaluate_ca_efficiency(&m, Some(80.0));
    assert_eq!(eff.efficiency_label, "Unavailable");
    let auto_rec = crate::rf_lab::autopilot_recommend(&m, "Gaming");
    assert!(!auto_rec.best_band.is_empty());

    let html_tmp = std::env::temp_dir().join("modemdeck-test-health.html");
    crate::rf_lab::export_html_health_card(&m, &html_tmp).unwrap();
    let html_body = std::fs::read_to_string(&html_tmp).unwrap();
    assert!(html_body.contains("ModemDeck Health Card"));
    let _ = std::fs::remove_file(html_tmp);

    let _ = crate::usage::sample_active_app_sockets();

    // Multi-language i18n check
    for (code, expected_rtl) in [
        ("en", false),
        ("fa", true),
        ("ar", true),
        ("tr", false),
        ("de", false),
        ("fr", false),
        ("es", false),
        ("ru", false),
    ] {
        crate::i18n::init(code);
        assert_eq!(crate::i18n::is_rtl(), expected_rtl);
        assert!(!crate::i18n::translate_ui_text("Overview").is_empty());
    }
    crate::i18n::init("en");
}

#[test]
fn test_slint_migration_sqlite_tray_notifications_and_svg() {
    // SQLite insert + downsampled query
    let uid = "test-modem-uid-v2";
    let rec = crate::storage::history_db::HistoryRecord {
        timestamp: 1_700_000_001,
        modem_uid: uid.into(),
        sim_hash: "sim1".into(),
        operator: "Irancell".into(),
        band: "B3".into(),
        signal_percent: 78,
        rsrp: Some(-88.0),
        rsrq: Some(-9.0),
        snr: Some(14.5),
        ..Default::default()
    };
    crate::storage::sqlite_db::insert_history_record(&rec).unwrap();
    let queried = crate::storage::sqlite_db::query_recent_downsampled(uid, 10);
    assert!(!queried.is_empty());
    assert_eq!(queried.last().unwrap().band, "B3");

    // Tray 22x22 ARGB32 pixmap generation (Linux)
    #[cfg(target_os = "linux")]
    {
        let icon = crate::tray::render_signal_pixmap(85, true);
        assert_eq!(icon.width, 22);
        assert_eq!(icon.height, 22);
        assert_eq!(icon.data.len(), 22 * 22 * 4);
    }

    // Notifications cooldown + center
    crate::notifications::notify_with_cooldown(
        "test-alert-key",
        "ModemDeck Test",
        "Signal check",
        60,
        false,
    );
    let list = crate::notifications::list_notifications();
    assert!(list.iter().any(|n| n.title == "ModemDeck Test"));

    // Slint SVG signal path helper
    let svg = crate::ui::slint_app::build_signal_svg_path("nonexistent-uid");
    assert!(svg.starts_with("M "));

    // Localization 100% catalog coverage validation across all 8 languages
    let report = crate::i18n::validation::validate_catalog().expect("100% i18n catalog coverage");
    assert!(report.duplicate_keys.is_empty());
    assert!(report.placeholder_mismatches.is_empty());
    assert_eq!(report.languages.len(), 8);
    for lang in &report.languages {
        assert_eq!(lang.coverage_percent, 100.0);
        assert!(lang.empty_keys.is_empty());
    }

    // Hotspot uplink & VPN leak inspection sanity check
    let (_, uplinks) = crate::hotspot::uplink::detect_uplinks(None);
    assert!(uplinks.iter().any(|u| u.id == "auto"));
    assert!(uplinks.iter().any(|u| u.id == "cellular"));
    assert!(uplinks.iter().any(|u| u.id == "ethernet"));
    assert!(uplinks.iter().any(|u| u.id == "wifi"));
    assert!(uplinks.iter().any(|u| u.id == "vpn"));
    let vpn = crate::hotspot::uplink::inspect_vpn_path();
    assert_eq!(vpn.verification_label, "Unverified");
}
