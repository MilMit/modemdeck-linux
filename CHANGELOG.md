## 2.0.0-beta.1 — Rust + Slint Migration Candidate (pre-release)

- **Complete GTK4 Removal & Slint 1.19.1 UI**:
  - Replaced all GTK4/GDK4/Cairo/GIO/GLib UI code with a centralized Slint Design System (`ui/tokens.slint`, `ui/theme.slint`, `ui/components.slint`, `ui/main.slint`) and `src/ui/slint_app.rs`.
  - Added Dark, Light, and High-Contrast themes, Basic/Advanced/Expert modes, full RTL/Persian + 8-language support, collapsible sidebar, and reduced-motion toggle.
- **Pure-Rust D-Bus & Storage**:
  - Migrated `modem/helper.rs`, `modem/manager.rs`, `services/communications.rs`, `core/power.rs`, and `services/monitor.rs` from `gio::DBusProxy`/`glib::MainLoop` to async `zbus 6.5` + `tokio` (`crate::core::block_on`).
  - Added `rusqlite` SQLite WAL storage (`src/storage/sqlite_db.rs`) alongside JSON history.
- **System Tray, Desktop Notifications, Close-to-Tray & Single-Instance IPC**:
  - Added Linux StatusNotifierItem system tray (`src/tray/mod.rs` via `ksni`) with dynamic 22x22 ARGB32 signal-bar icon, live tooltip, quick actions, and XDG autostart support.
  - Added Freedesktop D-Bus notifications (`src/notifications/mod.rs`) with cooldowns and notification center.
  - Added Unix domain socket single-instance activation (`$XDG_RUNTIME_DIR/modemdeck-ui.sock`) and `--minimized` / `--tray` startup flags.
- **Packaging & Docs**:
  - Updated DEB (`packaging/build-deb.sh`) and Snap (`snap/snapcraft.yaml`) to drop `libgtk-4-1` / `gnome` extension; added Flatpak, RPM, Arch `PKGBUILD`, AppStream metainfo, and `docs/SLINT_UI_GUIDE.md`.
- **Beta release preparation**:
  - Made version metadata channel-aware, added a GitHub Releases fallback for the unavailable hosted update manifest, and prevented stable builds from offering prereleases.
  - Removed four unused direct Rust dependencies after static dependency analysis.

## 1.1.0 — Professional Network Analysis & Modem Management Upgrade

- **Live Signal & Radio Quality Fixes**:
  - Fixed `Extended signal metrics are not currently reported by this modem/driver.` remaining visible after live RSRP/RSRQ/RSSI/SNR values arrive.
  - Fixed Radio Quality panel not updating live when extended signal metrics arrive after initial render.
  - Added signal timestamps, Fresh/Cached/Stale/Unavailable state tracking, and context-aware cache invalidation on SIM, Band, or Operator changes.
- **LTE & 5G Signal Scoring System**:
  - Replaced equal-weight averaging with separate LTE and 5G NR scoring models (0–100 score, 7 grades, Confidence level, explanatory reasons).
  - Enforced interference guardrails so strong RSRP with poor SNR/RSRQ cannot be misclassified as high quality.
- **Profile Rollback Fix**:
  - Active band profiles are no longer persisted before confirmation; rejected/timed-out/crashed profile changes roll back cleanly without re-applying on restart.
- **Band Analyzer**:
  - Automated multi-band frequency test engine (Quick, Standard, Deep, Custom, Radio-only, Internet Performance, Stability, Carrier Aggregation modes; 30s–30m durations).
  - Serving band verification, data cap enforcement, exclusive operation lock, crash-safe `ScanTransaction`, and best-for-use-case winners.
- **Persistent Band Lock & Recovery**:
  - Added Automatic, Temporary Lock, Persistent Lock, and Smart Automatic modes with Safe vs Strict policies, lock verification states, and anti-reconnect-loop protection.
  - Scoped persistent locks to stable Modem UID + SIM hash + Operator across reboots, suspend/resume, and hot-plug events.
- **Carrier Aggregation, Speed Test, Stability, History, Hotspot**:
  - Added CA Analyzer, interface-bound Internet Speed Test (with VPN vs Direct route detection), multi-target Connection Stability Monitor, Operator Comparison, persistent History DB with Cairo charts + CSV/JSON export, and NetworkManager Mobile Hotspot.
- **Architecture & UI**:
  - Migrated from `.rs.part` fragments to clean standard Rust modules (`core`, `modem`, `drivers`, `analyzer`, `locking`, `optimization`, `hotspot`, `storage`, `services`, `diagnostics`, `i18n`, `ui`).
  - Added Dark / Light / System themes, improved Dark Theme contrast, and Persian (RTL) support.

## 1.0.0-rc1.4.1 — Tested Ubuntu package

- GitHub Actions build passes for Rust, Debian package, AppImage, source archive and checksums.
- Debian package no longer hard-depends on legacy `policykit-1`; `pkexec` is recommended for privileged vendor reads.
- The `.deb` package was installed successfully on the target Ubuntu system and the ModemDeck GUI launched successfully.
- Published as the first tested RC release on GitHub.

## 1.0.0-rc1.4 — Smooth live UI

- ModemManager D-Bus signal changes no longer rebuild the whole modem page.
- Signal percentage, progress bar, operator/network state and LTE/5G radio metrics update in place.
- D-Bus update bursts are coalesced for 250 ms to avoid redraw storms.
- Scroll position, selected section, expanded panels and partially entered form data now stay stable during live updates.

# Changelog

## 1.0.0-rc1.1 — compile fix

- Fixed the i18n fallback lifetime so unknown translation keys can safely return the caller-provided string instead of incorrectly requiring a `\'static` lifetime.
- Removed duplicate Persian translation match arms.
- Removed an unused GIO prelude import in the suspend/resume watcher.
- Kept the current GIO signal subscription implementation for runtime compatibility and scoped its deprecation allowance to that watcher only.

## 1.0.0-rc1 — MilMit release candidate

- Added APN Manager backed by NetworkManager: list, create, edit, activate and delete GSM/APN profiles. Optional APN passwords are written directly to NetworkManager and are never copied into ModemDeck configuration.
- Added SIM PIN tools for unlock and enabling/disabling PIN protection through the standard ModemManager SIM interface.
- Added live RX/TX throughput using Linux network-interface byte counters; no speed-test traffic is generated.
- Added local daily and monthly mobile-data accounting with bounded history and reset controls.
- Added band profiles with built-ins for `Auto`, `B42 TD-LTE`, and `B1+B3+B7`, plus user-created profiles.
- Added optional restore of the active band profile after boot and resume through the lightweight background monitor.
- Added systemd-logind suspend/resume awareness and modem state resynchronization after resume.
- Kept event-driven ModemManager hot-plug support for USB/internal WWAN devices.
- Added an in-app Supported Modems/capability page.
- Expanded privacy-safe diagnostics while continuing to omit IMEI/equipment IDs.
- Added MilMit update checks via `https://milmit.net/modemdeck/update.json` and included a server manifest example.
- Added Report a problem / `milmit.net` integration.
- Added English/Persian language selection and translation scaffolding for the main UI, tools and preferences.
- Added release notes inside the application and improved MilMit About/metadata branding.
- Upgraded band rollback to a persistent crash-safe transaction plus a detached rollback guard: if the GUI crashes before the 45-second confirmation expires, the guard can still restore the last known-good bands.
- Added automatic local backups before band changes and cell-lock preview generation, with old backups pruned to a bounded history.
- Added exact structured CA component display such as `PCC B42 20 MHz + SCC1 B42 20 MHz`, topology (`2CA`, `3CA`) and total component channel bandwidth.
- Added RC1 styling for Data Usage, SIM, APN, Profiles and CA component cards.
- Updated Debian dependencies for NetworkManager, PolicyKit and curl integration.

### Hardware validation carried forward

The target Dell/Foxconn DW5821e / T77W968 on Ubuntu has already validated:

- FDD LTE B1 with 3CA `B1 + B7 + B3` (20 MHz each).
- TD-LTE B42 with EARFCN 43292, PCI 89 and 2CA `B42 + B42` (20 MHz each).
- Direct vendor telemetry using the safe temporary ModemManager inhibit/read/return path.

### RC note

The RC1 source has not been compiled in the artifact-generation container because Rust/GTK development tooling is unavailable there. Build/runtime verification continues on the target Ubuntu system before a final 1.0 stable tag.



## 0.5.0-alpha.5

- Added MilMit branding throughout the desktop application.
- Added `milmit.net` as the project website in Cargo and Debian metadata.
- Added `ModemDeck by MilMit • milmit.net` identity to the GTK header and footer.
- Updated About dialog, desktop launcher, user service, GNOME indicator metadata and package maintainer branding.
- Kept modem logic and the Alpha 4 DW5821e/CA behavior unchanged to avoid destabilizing the tested radio path.

## 0.5.0-alpha.4

- Parse DW5821e PCC/SCC carrier aggregation into structured components.
- Show CA topology such as 3CA B1+B7+B3 and total component channel bandwidth.
- Disable custom Band Manager apply until at least one LTE band is selected.
- Improve active preset highlighting across GTK themes.
- Constrain Preferences switches to a compact layout.
- Keep deep vendor reads manual to avoid repeatedly inhibiting ModemManager.

## 0.5.0-alpha.3 — Phase 5 radio UX / DW5821e validation

- Promoted the verified DW5821e/T77W968 vendor radio snapshot to the authoritative serving-radio path for that modem family.
- Hid the generic `GetCellInfo` serving-cell panel on DW5821e because the tested ModemManager stack reports `base stations info is not supported` even through the privileged fallback.
- Added a serving-radio summary showing verified LTE band, FDD/TDD duplex, EARFCN, PCI, bandwidth and carrier-aggregation state after a vendor read.
- Renamed vendor actions to `Read radio details` / `Refresh radio details`.
- Kept vendor deep-read manual rather than periodically inhibiting ModemManager; this avoids needless mobile-data interruptions.
- Added a radio-quality assessment derived from RSRP, RSRQ and SNR. The ModemManager percentage is still shown separately and is no longer presented as the whole connection-quality story.
- Fixed Band Manager preset highlighting. `Auto`, `LTE FDD` and `TD-LTE` are highlighted only when the current allowed-band set actually matches that mode.
- Added a detected band-mode label (`Auto`, `LTE FDD only`, `TD-LTE only`, or `Custom`).
- Clearing or restoring band selections now invalidates cached serving-radio snapshots so stale band/CA data is not shown after a band change.
- The network badge continues to show `4G+` only when carrier aggregation is actually observed.

### Verified target result

The Dell/Foxconn DW5821e (T77W968) direct read was validated on the target Ubuntu system and returned a real serving snapshot including LTE B1, EARFCN 100, PCI 294, 20 MHz bandwidth and CA inactive.

### Build note

This source snapshot has not been compiled in the generation environment because Rust/GTK development tooling is unavailable there. Compile testing remains on the target Ubuntu machine.

## 0.5.0-alpha.2 — Phase 5 compile fix

- Fixed Linux signal handling for gtk-rs/glib 0.22 by using the dedicated `glib-unix` crate.
- Removed unused imports reported by Rust compiler.

## 0.5.0-alpha.1 — Phase 5 desktop integration

- Added a lightweight `--monitor` mode for always-on D-Bus monitoring without opening the GTK window.
- Added runtime status publishing for desktop integration.
- Added GNOME Shell top-bar extension showing cellular signal and `2G / 3G / 4G / 4G+ / 5G` when the underlying modem data can prove that state.
- Added signal history, Preferences, notifications, privacy-safe diagnostics, user systemd integration and `.deb` packaging.
- Kept Phase 3 safe band rollback and Phase 4 multi-vendor capability gating.
- LTE cell-lock remains preview-only; unverified vendor write commands are never guessed.

## 1.0.0-rc1.2

- Reworked the modem view into compact tabs: Overview, Network, SIM & APN, Messages, Calls & USSD, and Advanced.
- Added runtime feature chips for SMS, Voice calls, and USSD based on interfaces actually exported by ModemManager.
- Added on-demand SMS listing, sending, and deletion through the standard ModemManager Messaging/Sms D-Bus interfaces.
- Added voice call dial/list/answer/hangup controls when the modem exports the standard ModemManager Voice interface.
- Added USSD initiate/respond/cancel support when the modem exports the 3GPP USSD interface.
- Messaging and call lists load only on demand so live signal updates do not create continuous extra modem traffic.
- Call audio is deliberately reported separately from call control. A modem exposing Voice does not automatically guarantee a usable host audio path.

## 1.0.0-rc1.3 — Portable packaging
- Removed runtime dependency on `curl`; update checks now use built-in HTTPS via Rustls.
- Removed runtime dependency on `notify-send`; notifications use the desktop D-Bus API directly.
- Added MilMit application SVG icon and package integration.
- Debian package now depends only on GTK4 runtime plus the host modem/network/policy services.
- Added AppImage build pipeline.
- Added one-command `packaging/build-all.sh`.
- Added experimental strict Snap manifest for Ubuntu App Center testing.
- Added GitHub Actions workflow to build Linux release artifacts automatically.
