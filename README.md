# ModemDeck Linux by MilMit

**Website:** https://milmit.net

<p align="center">
  <img src="assets/icons/net.milmit.ModemDeck.svg" width="128" alt="ModemDeck icon">
</p>

<p align="center"><strong>Native Linux control for cellular modems — LTE, TD-LTE, 4G+, APN, SIM, messaging and diagnostics.</strong></p>


ModemDeck is a lightweight native GTK4 cellular modem manager for Ubuntu/Linux, developed by **MilMit**. It uses ModemManager/NetworkManager and small event-driven helpers instead of an Electron/Chromium runtime or a continuous `mmcli` polling loop.

## What RC1 includes

### Radio dashboard

- Multi-modem detection and hot-plug updates through ModemManager D-Bus.
- 2G/3G/4G/4G+/5G display when the available telemetry can prove that state.
- RSRP, RSRQ, SNR, RSSI and a separate radio-quality assessment.
- Signal history.
- Serving LTE band, FDD/TDD, EARFCN, PCI and channel bandwidth when available.
- Structured carrier aggregation: PCC/SCC components, 2CA/3CA topology and component channel bandwidth.
- GNOME top-bar integration plus optional low-overhead background monitor.

### Safe band controls

- Auto / LTE-FDD / TD-LTE presets.
- Custom supported-band selection using ModemManager `SetCurrentBands`.
- 45-second confirmation window.
- Local pre-change backup.
- Persistent pending transaction.
- Detached rollback guard that survives a GUI crash and restores the previous selection if it is still unconfirmed.
- Bounded safety-backup history.
- LTE cell-lock stays **preview-only** until model/firmware-specific write/recovery behavior is verified.

### Band profiles

Built-in profiles:

- `Auto`
- `B42 TD-LTE`
- `B1+B3+B7`

You can also save/delete your own profiles. Optionally, the active profile is restored after reboot and after suspend/resume by the lightweight user service.

### APN Manager

NetworkManager-backed GSM/APN profiles can be:

- listed
- created
- edited
- activated
- deleted

APN username and an optional password are supported. Password values are handed directly to NetworkManager and are **not** stored in ModemDeck's own config. Existing passwords are never read back into the UI.

### SIM security

Through the standard ModemManager SIM interface:

- unlock SIM with PIN
- enable PIN protection
- disable PIN protection
- show current SIM lock state/slot when reported

### Live traffic and data usage

- Live RX/TX rate from `/sys/class/net/<interface>/statistics/*`.
- No speed-test traffic is generated.
- Daily and monthly local byte accounting.
- Reset control.
- Local history is bounded to prevent unbounded config growth.

Accounting is active while the GUI or optional background monitor is running.

### Desktop/system integration

- Suspend/resume detection through `systemd-logind`.
- State resync and optional profile restore after resume.
- Modem USB/internal WWAN hot-plug handling.
- Optional notifications.
- `systemd --user` monitor.
- GNOME top-bar extension.
- Desktop launcher.
- `.deb` build script.

### Support, updates and privacy

- Supported Modems/capability page.
- In-app release notes.
- Update check against `https://milmit.net/modemdeck/update.json`.
- `server/update.json.example` for publishing the manifest on MilMit.
- Report-a-problem / MilMit website integration.
- Privacy-safe diagnostics export: IMEI/equipment IDs are intentionally omitted.
- English/Persian language preference and translation infrastructure.
- MilMit About/website/package metadata.

## Verified Dell/Foxconn DW5821e results

Testing on the target Ubuntu laptop has verified the `T77W968 / DW5821e Snapdragon X20 LTE` path:

- FDD LTE B1 serving carrier.
- `3CA • B1 + B7 + B3`, each component reported as 20 MHz.
- Real TD-LTE band lock to B42.
- B42 EARFCN 43292 / PCI 89.
- `2CA • B42 + B42`, each component reported as 20 MHz.
- Vendor radio details through a direct read that temporarily inhibits and then returns the modem to ModemManager.

A sum such as 40/60 MHz is **component channel bandwidth**, not an internet speed estimate.

## Multi-vendor architecture

Generic ModemManager support is used whenever possible. Advanced telemetry is capability-gated rather than guessed. Driver profiles exist for:

- Dell/Foxconn DW5821e / T77W968
- Quectel
- Fibocom
- Sierra Wireless / AirPrime
- Huawei
- Telit / Cinterion
- SIMCom
- Generic MBIM/QMI fallback

Support varies by exact modem and firmware. Controls that are unsupported or not verified stay disabled.


## GitHub releases

GitHub Actions builds packages on every push to `main`. The workflow also publishes/refreshes a `nightly` prerelease with the latest binaries. Version tags (`v*`) create versioned releases containing:

- `MilMit-ModemDeck-<version>-amd64.deb` — recommended Ubuntu package
- `MilMit-ModemDeck-<version>-x86_64.AppImage` — portable build
- `MilMit-ModemDeck-<version>-linux-source.tar.gz` — source snapshot
- `SHA256SUMS.txt` — checksums for release assets

See `docs/PUBLISHING.md` to publish the repository and create a release directly from Ubuntu.

## Build on Ubuntu

Install development/runtime dependencies:

```bash
./scripts/install-deps-ubuntu.sh
```

Build and run:

```bash
cargo run --release
```

Run the lightweight monitor alone:

```bash
cargo run --release -- --monitor
```

## Install for the current user

```bash
./scripts/install-user.sh
```

This installs the binary, desktop entry, user systemd unit and GNOME extension files when GNOME is present. Enable/disable the monitor from Preferences.

## Build a Debian package

```bash
./packaging/build-deb.sh
```

The `.deb` is written under `target/`.

## Self-check

```bash
./scripts/phase5-selfcheck.sh
```

It checks ModemManager, NetworkManager tools, PolicyKit helper availability, update-check dependency, user monitor/indicator installation and current process resource usage.

## Translation

Preferences support System / English / فارسی. The main UI is translated through stable translation keys plus a migration layer for legacy labels. Language changes are applied on restart.

## Update manifest on milmit.net

Copy `server/update.json.example` to the web location used by RC1:

`https://milmit.net/modemdeck/update.json`

Update `version`, `url` and `notes` for each release. ModemDeck sends no analytics in this check; it only downloads that small JSON manifest.

## Privacy and logs

- Diagnostics omit IMEI/equipment IDs.
- APN passwords are not stored in ModemDeck settings.
- No verbose persistent AT response logging by the monitor.
- Runtime status contains display-oriented state only.
- Data usage is stored locally in the user's config directory.

## Important safety behavior

Vendor deep reads may temporarily inhibit ModemManager and can briefly reconnect mobile data. They are therefore manual rather than high-frequency background operations.

Band writes are guarded by backup + persistent transaction + detached rollback process. Cell-lock writes remain preview-only in RC1 because persistence/recovery behavior is vendor/firmware-specific.

## Uninstall user installation

```bash
./scripts/uninstall-user.sh
```

### Communications in RC1.2

The main window is split into focused tabs instead of one long page. SMS, Voice and USSD controls are capability-gated at runtime. ModemDeck only enables a feature when ModemManager exports the matching standard D-Bus interface for the selected modem.

SMS supports on-demand inbox loading, sending and deleting. Voice supports dialing, answering and hangup when the Voice interface exists. Voice call control and voice audio routing are treated as separate capabilities; some WWAN modules can expose call control without a usable host audio route. USSD supports initiate/respond/cancel when available.

## End-user installation (RC1.3)
For normal users, do not run the developer dependency script. Distribute either the `.deb` or AppImage release artifact. The `.deb` lets Ubuntu install required OS runtime services automatically; the AppImage bundles ordinary application libraries and uses the host ModemManager/NetworkManager/Polkit services. See `PORTABLE.md`.


### Smooth live updates
Starting with RC1.4, routine ModemManager signal/property events patch the visible labels and progress bar in place instead of rebuilding the full GTK modem page. Structural refreshes are reserved for modem add/remove, manual refresh and explicit configuration operations.
