# ModemDeck desktop UI redesign

This proposal follows the existing Slint window, page model, `Tokens`, `Theme`, and reusable components. The first implementation pass is visible in `ui/main.slint`: it promotes cellular scanning into Operators, makes the data interruption explicit, removes the prefilled hotspot password, puts internet path and sharing on the dashboard, and labels the limits of hotspot uplink selection and failover. The screen continues to use the existing three theme modes and `Theme.is-rtl` alignment rules.

## Product navigation

Keep one persistent device context in the top bar (selected modem, state, signal, refresh, theme and complexity). Make selection a named selector with the modem model and connection state; all modem actions use this selected context. Group the sidebar as:

1. **Overview** — modem summary, live path, hotspot state, quick actions.
2. **Internet** — active links, route, priorities and failover.
3. **Hotspot** — sharing source, Wi-Fi AP settings and VPN forwarding.
4. **Mobile network** — current operator, live cellular scan, network mode and bands.
5. **SIM & plans** — SIM, eSIM, APN.
6. **Messages & calls** — SMS, USSD, calls.
7. **Diagnostics** — doctor, connectivity, history and privacy-safe export.
8. **Advanced** — RF analyzer, GPS/cells, hardware/firmware, safe AT console.
9. **Settings**.

The current page IDs can stay stable during migration; this is primarily a grouping and naming change. Do not add a Wi-Fi scan. A Wi-Fi uplink is selected from active NetworkManager connections only.

## Wireframes

### Overview

```text
┌ Selected modem ▾  Connected · LTE · 4/5 bars             Refresh ┐
│ Operator, country, confidence · active band · cell / CA status     │
│ RSRP / RSRQ / SINR       ↓ receive / ↑ send       Today / month   │
├───────────────────────────────────────────────────────────────────┤
│ INTERNET PATH: Ethernet → VPN (healthy)   [route details]          │
│ Sharing: Hotspot ON · 2 clients · uplink: Mobile broadband         │
│ [Start/stop hotspot] [Cellular scan] [Run diagnostics] [Modem ▾]  │
└───────────────────────────────────────────────────────────────────┘
```

Render route and uplink as **Unknown / not monitored** when NetworkManager data is unavailable. Modem registered/connected state is not proof of internet reachability. Keep sensitive SIM identifiers off this page.

### Mobile network scan

```text
Selected modem: Quectel … · registered on Operator A
Scanning searches nearby 3GPP networks, not Wi-Fi. Active data may pause.
[Scan cellular networks]  Last scan: time / never
┌ Operator | MCC/MNC | Country | Radio | Availability | Roaming ┐
│ Operator A | 432/35 | Iran | LTE | Current | No                │
│ Operator B | 432/11 | Iran | LTE | Available | Yes              │
└───────────────────────────────────────────────────────────────┘
Choose available row → impact confirmation → Register → result/error
[Return to automatic selection]
Separate card: operator database lookup by name, country, or MCC/MNC.
“Database lookup does not detect nearby available networks.”
```

States: idle, scan confirmation, scanning with progress, results, empty results, timeout/error with retry, registration confirmation, registering, registered, failed, automatic selection restored. Keep last scan time and distinguish stale results from live results.

### Hotspot and VPN sharing

```text
Hotspot  Off                                                    [On]
Uplink: [Automatic ▾]  Active sources: Mobile / Ethernet / Wi-Fi / VPN
Wi-Fi AP: adapter + AP capability · concurrent STA/AP warning
SSID [____________]  Password [••••••••] [Show temporarily]
Band [Auto ▾] Channel [Auto ▾] Security [WPA2 ▾]
Clients: 2 connected · per-device usage when supported
VPN source selected:
  VPN tunnel: Connected · route/DNS verified / not verified
  Client traffic through tunnel: On / blocked / unsupported
  VPN lost → Stop client internet (default, fail closed)
  [Confirm protected sharing]
```

Source choices must show active interface name, health, VPN badge, and whether the source is usable. Never imply VPN forwarding is enabled merely because a VPN is connected on the host. If route policy or kill switch cannot be installed, explain the missing privilege/backend and keep protected sharing disabled. Wi-Fi uplink is a list of active connections; do not offer scan networks.

### Internet and failover

```text
Current default route: Ethernet (healthy) → VPN tunnel (healthy)
Mode: Manual / Automatic failover (distinct state)
Priority 1 Ethernet   healthy · checked 14:32 · [↑] [↓]
Priority 2 Cellular   healthy · checked 14:31 · [↑] [↓]
Priority 3 Wi-Fi       disconnected · “not connected”
VPN overlay             connected · endpoint omitted from exports
Automatic switch: Off / On · threshold · cooldown · failback
Preview: switching may interrupt calls and active data sessions.
[Apply] [Restore previous priorities]
```

Show transition states: connection lost → health check delay → route switching → verifying → recovered/failback or failed with reason. Route-metric priority editing alone must be labeled as priority configuration, never as active automatic failover.

### Operator and APN

Operator section shows current name, country, MCC/MNC, detection confidence/source and a suggested APN action. APN rows show profile name, APN, active/saved, and safe actions. Create/edit form: profile name, APN, username, password (write-only; blank means keep unchanged), authentication and IP type where supported. Recommend profile by detected operator. Confirm before activation or deletion; activation previews that the data session may drop. Never read back or display saved credentials.

### Band control and rollback

```text
Selected device · Firmware capability: LTE band control supported / unknown
Mode: (•) Automatic  ( ) Manual
Supported bands: [B1] [B3] [B7] [B20]      Current selection: Auto
Change preview: connection may drop while the modem re-registers.
[Review change]
Confirm panel: target bands · selected modem · saved previous state
Rollback protection: 45 s remaining  [Keep this selection] [Rollback now]
Cell lock: Not confirmed for this modem/firmware · [Learn why] [Preview disabled]
```

Use the existing transaction and rollback behavior: show the real deadline from the operation, keep the selection button disabled until a pending transaction exists, and show success/failure after re-reading the modem. A cell lock must use per-device capability evidence; absent evidence means unavailable, not a guessed enabled control.

### Band Analyzer

```text
Selected modem · data route: Cellular / unknown · data cap: no limit / limit
Test: Quick / Standard / Deep / Radio-only / Stability / Custom
Duration and stages · estimated data use · [Start]
Running: Stage 2/5 · LTE B3 · 01:24 · samples 18 valid / 2 missing [Cancel]
Results: Band | RF quality | DL/UL | latency/jitter | stability | samples | confidence
Outcome badges: Passed / Band not verified / No coverage / Failed / Cancelled / Data cap
Recommendation: “B3 · best balance” · confidence Medium · reason: stability + throughput
[Compare saved] [Export CSV] [Export JSON]
```

Separate measured internet speed from radio scores; a radio-only run must not imply internet performance. Label skipped/missing measurements, show serving-band verification and sample confidence, and avoid presenting carrier aggregation bandwidth as throughput. The smart recommendation stays a suggestion with its reason and confidence.

### APN editor states

```text
Current operator: Operator A · MCC/MNC · confidence/source
Saved profiles: profile | APN | active | last changed
[Recommended APN] [New APN]
Editor: Profile name · APN · Username · New password (optional)
        Auth · IP type (only supported controls)
Password is write-only; blank keeps the stored credential.
Activation preview: may disconnect mobile data · [Activate] [Cancel]
Delete preview: profile name · active profile? data may drop · [Delete] [Cancel]
```

Do not put username or APN password in the profile list, debug console, history, or exports. On unsupported auth/IP fields, omit or explain; don't accept values the backend ignores.

## Other product areas

### SIM, messages, calls, USSD and eSIM

SIM card: slot, lock state and remaining attempts only where the modem reports them. PIN unlock and PIN protection changes use a confirmation step, clear consequences and sanitized errors. SMS list supports refresh, read, send and delete confirmation. Calls distinguish signaling control from voice/audio path: show “call control available; audio path unverified” unless voice capability is confirmed. USSD has sending, response, interactive continuation and cancel states. Unsupported actions remain visible as disabled with a device-specific reason.

The eSIM panel shows hardware/tool support, installed profiles with privacy-safe labels, active state, download/add, enable/disable, primary profile and delete. Activation code entry is masked, cleared after submission, excluded from logs and exports. Deletion always names the profile and requires confirmation. `lpac` absence or eUICC absence gets a clear setup explanation.

### GPS and antenna positioning

GPS card: GNSS enabled/disabled, fix/no fix, accuracy, last-fix age and an explicit location privacy control. Current backend reads a point on request and exports GPX/CSV; continuous track recording is not implemented. Only show coordinates inside an explicit location view, warn that they are sensitive, and provide clear/delete controls if persisted. Antenna-position comparison uses labeled repeated samples (baseline, best position, delta and sample count); it reports aggregate modem metrics only and must never invent independent per-antenna readings.

### Hardware, firmware, diagnostics, history and backup

- Hardware card separates **detected**, **database-confirmed**, **user-tested**, and **unknown** compatibility. Show model, driver, bus and firmware only in the device details view. Redact identifiers in generic reports; require a deliberate detailed export for identifying data.
- Firmware card reports current version and LVFS/fwupd availability. Installation is shown only if an official supported update path exists; otherwise provide status/instructions without an install button.
- Diagnostics group service health (ModemManager/NetworkManager), modem/SIM, RFKill, default route, gateway, DNS, VPN and internet probes. Each result includes evidence, time, severity, next action and whether a fix is safe or requires confirmation. Include disconnect, VPN-loss and rollback notifications.
- History filters signal, usage, analyzer runs, speed tests, cellular scans and route changes by time/device. Usage shows the interface and counter source. The current history database mainly records modem snapshots; route/scan/analyzer history needs wiring.
- Backup export is available. Restore needs a file preview, schema/version validation, diff, confirmation and rollback plan. State explicitly that secrets and activation codes are excluded. The restore backend exists but no restore flow is wired into the UI.
- Scheduler entries only appear if the task runner executes them. The current scheduler module stores configuration; avoid suggesting saved tasks are running without a scheduler execution loop.

## High-risk flow outcomes

| Action | Before action | Running | Success | Failure / recovery |
|---|---|---|---|---|
| Cellular scan | Selected modem + possible data interruption | Stage + elapsed time + cancel availability | Structured results + last-scan time | Timeout/empty result, retry or return to automatic |
| Manual operator registration | Operator, roaming and data-loss effect | Registering; keep selected modem visible | New operator and data path rechecked | Reason + automatic registration action |
| Band change | Target bands, expected outage, rollback window | Countdown with Keep/Rollback | Re-read actual bands | Automatic rollback state and recovery hint |
| Hotspot VPN share | Verify selected source is VPN default route; DNS and kill switch support | Monitor tunnel and client route | Explicit “client traffic verified through VPN” only after probe | VPN loss blocks client egress; otherwise protected share remains unavailable |
| Route/failover apply | Current routes, target order, calls/data impact | Applying + health-check stage | Route re-probed and previous policy snapshot retained | Restore prior metrics/policy |
| SIM PIN change | Target SIM/slot, retry consequences | No secret echoed in status | New lock state re-read | Attempts remaining only if modem confirms; never log PIN |
| APN activate/delete | Profile and possible data interruption | NetworkManager operation | Connection re-read | Restore previous active profile where possible |
| eSIM add/delete | Profile identity without activation secret, connectivity impact | Download/delete stage | Profile list re-read | Keep activation code out of result/log |
| Restore backup | File/version, diff, excluded secrets | Applying with progress | Re-read settings and confirm | Restore previous settings snapshot |

Any interrupted route or VPN-dependent operation returns to **state unknown** until the backend re-reads the OS. Do not display stale “connected” as current success.

## Shared components and state rules

- `DeviceContext`: selected modem, device state, no-device state, and scope label on every modem-specific page.
- `ConnectionBadge`: state plus text; use color as a secondary signal only.
- `PathRow`: source → VPN overlay → destination and freshness timestamp.
- `CapabilityNotice`: unsupported vs temporarily unavailable vs permission required, each with a next action.
- `ImpactConfirm`: operation, expected interruption, affected modem/route, Confirm and Cancel.
- `OperationResult`: progress, success, actionable error, timeout and retry.
- `EmptyState`: no devices, no scan results, no APNs, no messages, or no history; never use a blank card.
- Use existing `ModernCard`, `KpiTile`, `StatusPill`, `ModernButton`, `ModernTextField`, and `EmptyState`. Keep status text readable in high contrast; don't encode status using color alone.

Every live value carries a timestamp or a clear “not available” label. On disconnect/VPN loss during an operation, stop or roll back where safe and report the outcome. Sensitive values (IMEI, IMSI, ICCID, SIM PIN, APN password, activation codes, VPN endpoint/credentials) are masked and excluded from diagnostic exports. Exports should say when values were redacted.

## Localization and themes

Use `Tr` strings for every user-visible label and action; add Persian translations alongside English. Set `Theme.is-rtl` from language, mirror navigation and directional arrows, and keep technical tokens (LTE, MCC/MNC, RSRP, interface names, IP addresses) in LTR runs. Test long Persian labels by allowing wrap/elision only for secondary text. Layout direction must not change data order in tables. Existing `Theme.mode` covers dark, light, and high-contrast; check focus, disabled, warning and error states in all three. Increase minimum width only if the app cannot maintain a usable single-column fallback.

## Existing implementation and backend gaps

| Area | Available now | Needed to complete the experience |
|---|---|---|
| Modem dashboard | ModemManager state, operator, radio metrics, serving band/cell and signal | Active default route, internet probes, country confidence and selected-device context (several current actions still use `first()` rather than selected modem) |
| Cellular scan | `scanner::run_scan`, automatic registration; basic output text | Structured result model in Slint, progress/last-scan timestamp, manual registration for selected available network and confirmation/rollback result |
| Operator database | Local lookup by name/country/MCC-MNC | Structured rows and explicit distinction from live scan (now labeled in UI) |
| Hotspot | NetworkManager starts/stops AP, password validation, interface/client MAC count and byte counters, hardware capability detection in module | Select active source, AP capability UI, band/channel/security controls, named client usage, interface list and hotspot internet health |
| VPN sharing | No forwarding or kill-switch implementation found | Route/DNS verification, policy routing/NAT, VPN disconnect watcher, fail-closed enforcement, clear privilege/error reporting; must be implemented before enabling “VPN uplink” |
| Failover | `apply_failover_policy` updates route metrics on active Wi-Fi/Ethernet/GSM profiles | Persistent ordered priorities, route health probes, automatic state machine, VPN-aware route tracking, rollback and truthful enabled state |
| APN | Create recommended/manual profile, activate/delete, saved profile list | Full edit/create fields including username/password/auth; write-only credential update and impact confirmation |
| SIM/SMS/USSD | PIN unlock, slot switching, SMS operations, USSD, call hooks; eSIM capability detection/profile load hook | PIN protection toggle, deletion/edit/send confirmations, interactive USSD continuation/cancel, full eSIM add/remove lifecycle; feature-level unsupported reasons |
| Diagnostics | Doctor, service/network checks and exports exist | Redaction audit across every export, live VPN/DNS/route and hotspot health, searchable event history for routes and scans |

Additional page-by-page capability audit from the current Rust modules and Slint callbacks:

| Page / capability | Backend currently present | UI status and remaining work |
|---|---|---|
| Multi-modem selection | ModemManager enumerates several devices | Top-bar cycling only changes a local display index; Rust summary/actions still use the first modem. Selection is intentionally marked unavailable in UI until a selected modem ID is threaded through sync and every action. This is a P0 correctness task. |
| Band control | Supported-band discovery, band apply, safe transaction, pending-state recovery, countdown guard and rollback exist | Band selection UI and confirm/keep/rollback callbacks exist. Ensure the page scopes to selected modem and binds countdown/result state; cell-lock capability must remain evidence-based. |
| Band Analyzer | Modes, cancellation, RF sampling, performance probes, per-band results and confidence/outcome model exist | Analyzer start/progress/results are wired. Persisted result store, history comparison, robust sample confidence view, recommendation explanation and exports need UI integration. Smart selection exists in `optimization` but is not wired into the page. |
| Operator scan | `scanner::run_scan`, scan history type and return-to-auto exist | Scan trigger currently renders formatted text, not structured rows; elapsed time, latest timestamp, manual operator registration and result confirmation are missing. |
| APN | Backend list/create/update/activate/delete, recommended profiles, password secret-store helper and profile backup/restore exist | UI supports list, basic name/APN creation, activation, deletion and recommendation. Add editor for supported fields, password-safe update, confirmation, and import/export review. |
| SIM, SMS and calls | PIN unlock, slot switch, SMS list/send/delete, dial/answer/hangup and USSD initiate/cancel are in services | Several are surfaced, but answer/delete and USSD continuation/cancel need explicit UI. PIN protection enable/disable is not present. Voice bearer/audio availability must be separately detected. |
| eSIM | Capability detection and `lpac` list/download/state/delete operations exist | UI currently shows capability and load action only. Add privacy-safe lifecycle UI, selected-modem scoping and tool/device reason states. |
| GPS | Enable/disable, read one fix, GPX/CSV export functions exist | UI handles manual read and exports from stored fixes. Continuous tracking, retention policy and user-facing location deletion/privacy controls need backend/UI work. |
| Antenna positioning | In-memory labeled point samples, aggregate signal score, best point and fluctuation exist | Basic record/summary is wired. Add baseline comparison and chart/sample counts. Backend intentionally has aggregate metrics, not independent physical antenna values. |
| Hardware/firmware | Hardware database/sysfs detection and firmware/fwupd inspection exist; safe hardware report builder redacts identifiers | Device and firmware summaries are surfaced. Separate confirmed vs unknown capabilities and offer install only through a verified official path. |
| Diagnostics | Doctor, safe fixes, endpoint probes, HTML/health and privacy-safe report functions exist | Main checks run, but route/DNS/VPN and hotspot egress require new probes; review each export for location and identifier leakage. |
| History/usage | SQLite modem snapshots, traffic counters, query/export and stored analyzer results exist | History and usage charts/exports are surfaced. UI does not yet connect analyzer comparison, scan history or route-change events. Counter interface/source and freshness need display. |
| Backup/restore | Configuration backup export and restore functions exist; hardware compatibility report exists | Export is surfaced. Restore preview/diff/version validation/confirmation/rollback flow is absent. |
| Scheduler | Task/config models can load/save enabled task settings | No active execution loop was found. Present schedule settings as unavailable until execution is implemented, not as running automation. |
| Platform support | Linux path is implemented; Windows is a stub and macOS has no feature adapter | Keep Windows/macOS destinations marked planned until their adapters pass capability checks. |

## Implementation priorities

1. **P0 — trustworthy scope and routes:** thread selected modem identity through dashboard sync and every modem action; add NetworkManager route/interface/VPN/DNS health model; never infer internet from modem registration.
2. **P1 — sharing and cellular network choice:** choose active hotspot uplink, validate actual client egress, implement VPN policy routing and fail-closed loss behavior, then expose the control; return structured scan results and manual registration with confirmation.
3. **P2 — safeguards and capability evidence:** connect confirmations to band/APN/SIM/eSIM operations, improve capability status per modem/firmware, complete diagnostics redaction.
4. **P3 — finish existing modules:** analyzer saved comparisons/recommendations, GPS privacy/track policy, eSIM lifecycle, restore preview, route/scan history, and scheduler execution if it remains a product feature.
5. **P4 — polish:** complete all `Tr` keys, verify Persian RTL and other long translations, and review dark/light/high-contrast at minimum window size.

## Slint implementation notes

- Keep global design primitives in `ui/tokens.slint`, semantic palette and direction in `ui/theme.slint`, and reusable `ConnectionBadge`, `PathRow`, `CapabilityNotice`, `ImpactConfirm`, and `OperationResult` patterns in `ui/components.slint`.
- Use page-local structs and `ModelRc` data for scan results, connection rows, analyzer samples, SIM/eSIM profiles and history. Avoid newline-joined strings for anything the user must select or sort.
- Add callbacks that return operation IDs or structured results; progress should update through the event loop and include a cancellable token. Do not block the Slint event loop on `nmcli`, `mmcli`, `lpac`, or network probes.
- Keep optional controls behind explicit capability states (`supported`, `unsupported(reason)`, `unknown`, `permission-required`). Don't use a disabled button without the reason nearby.
- At narrow widths use a one-column card flow and horizontally scrollable tables; avoid fixed widths for translated labels. Use `Theme.is-rtl` for alignment and mirror only directional navigation affordances, not protocol values.
- Keep identifiers and secrets out of generic `Text` debug widgets. Add an explicit reveal action with auto-hide only for hotspot password entry; never offer reveal for stored APN or eSIM secrets.

## Cross-platform design: Linux, Windows, macOS

The shared product uses one domain model and one Slint information architecture. Platform adapters supply capability evidence; the UI renders the same flows and vocabulary everywhere, with a small **Platform details** disclosure for service names, permission steps, and troubleshooting specific to the host OS. Never expose a control based only on the OS name. Modem model, firmware, driver, connection manager and permission state can all change the result.

### Shared capability state contract

Each feature returns a state, reason, source and checked time:

```text
Supported(evidence) | Unsupported(reason) | NeedsPermission(action)
NeedsInstall(component) | Unknown(reason) | TemporarilyUnavailable(reason)
```

Use one shared `CapabilityNotice` presentation for all states. `Unknown` is not `Supported`; an unimplemented platform is `Unsupported / backend not available` with a platform-specific next step. Keep data models and actions shared; implementations are injected by target-specific Rust backends. `PlatformOs` now identifies `Linux | Windows | MacOS` (plus a genuinely unknown fallback); replace its coarse platform booleans with typed feature status for modem discovery/control, route inspection, hotspot, VPN forwarding, credentials, background work, notifications and updates.

### Platform adapter and trust matrix

| Concern | Linux | Windows | macOS |
|---|---|---|---|
| Modem discovery/control | Current production path uses ModemManager/D-Bus; direct MBIM/QMI tools may help diagnostics. Requires the modem daemon, compatible driver/plugin, device permissions and possibly polkit authorization. | Implement against Windows Mobile Broadband APIs/MBIM and SetupAPI device discovery; advanced operations depend on driver/device services and API entitlement/packaging model. Treat vendor-only band/eSIM/GNSS behavior as unsupported unless verified. | No generic ModemDeck modem-management adapter exists today. Requires a supported modem vendor/driver interface (often vendor SDK or serial/MBIM device service) and device access; report per-device capability. Do not promise general native WWAN control. |
| Active network, default route, health | NetworkManager D-Bus/libnm where installed; otherwise detect an alternate manager or show route visibility unavailable. Network control may require polkit. | Windows Network List/IP Helper APIs for active interfaces, default routes and counters. Route mutation and sharing need a separately designed privileged adapter. | SystemConfiguration for service/interface/route state; NetworkExtension for supported VPN integration. System routing changes require an appropriately authorized helper. |
| Hotspot/source forwarding | Current backend starts an AP through the network service; source selection, client egress check and VPN fail-closed are not implemented. Network changes can require authorization. | Use OS-supported mobile hotspot/connection-sharing capabilities only after validating source, adapter combination and permissions. If the app cannot bind a chosen source or verify client traffic, expose source as unavailable. | Use supported system sharing behavior or an approved helper where available. The app must prove which uplink is shared; if the platform API cannot select/verify an uplink, show system-managed/unknown and do not offer a guaranteed VPN kill switch. |
| Credentials | Secret Service/libsecret or another desktop keyring; keyring may be locked/unavailable. Current code calls `secret-tool` and lacks a cross-OS implementation. | Windows Credential Manager or a DPAPI-backed credential adapter. Protect per-user secrets and avoid logging/deployment plaintext. | Keychain Services; handle access prompts and app signing/access-group constraints. |
| Background work / startup | User-level systemd service/timer when available, XDG autostart fallback; no assumption that systemd exists. Avoid privileged daemon unless needed. | Prefer user-level startup/task scheduling for monitoring; install a Windows service only for necessary privileged/system-wide work and explain elevation/consent. | Login item/launch agent for user work; launch daemon only for justified privileged work. macOS 13+ Service Management registration is user-visible and can require approval in System Settings. |
| Tray / menu surface | StatusNotifier/AppIndicator host availability varies by desktop; app remains usable without it. | Notification area icon with OS-managed overflow and standard context menu. | Menu bar extra with clear pause/quit/open controls; also respect Dock app lifecycle. |
| Notifications | Desktop notification service availability depends on desktop/session. | Native toast notifications; background/mobile broadband events may require package capabilities/metadata. | User-approved notification authorization; focus modes and notification settings remain OS controlled. |
| Secure update/install | Signed repository/package, Flatpak/AppImage or distribution package; updates depend on chosen channel and package manager. | Signed MSIX/MSI or official managed distribution; updater checks signature/channel and avoids changing modem drivers silently. | Signed and notarized app package/installer; official update channel validates signature and preserves user consent for privileged helpers. |

**Readiness today:** Linux is the only implemented OS backend, and even there route truth, selected-modem scope, source-bound hotspot forwarding and VPN fail-closed are gaps. Windows code explicitly contains a planned stub. macOS currently resolves to `Other`; it is not a partially working backend. The app should show a platform support card with “Core UI available; device/network controls unavailable on this platform” until a real adapter reports otherwise.

### Cross-platform screen and flow map

| Screen / flow | Shared UI and behavior | Linux variation | Windows variation | macOS variation |
|---|---|---|---|---|
| Dashboard and modem chooser | Same modem cards, signal/usage/path widgets. Zero / one / many modem states. Every command carries selected device ID. | Show current daemon/device access state only in Platform details. If device scope isn't selected, clearly state actions target the first detected modem; current selector is not wired. | Show API/driver account readiness and device access state. Do not expose advanced modem fields when Windows driver does not report them. | Show “cellular backend unavailable” until a vendor/OS adapter discovers device; never label a USB serial port as fully supported without capability probes. |
| Cellular operator scan/register | Same scan warning, structured operator rows, current/available/forbidden/unknown badges, manual register confirmation and auto-return. This is a cellular scan only; no Wi-Fi scanning anywhere. | Use current live scanner when selected modem exists; structured rows/manual register still need UI/backend work. | Use supported MBN provider/registration APIs and callbacks; scan/register availability depends on driver and device services. | Unknown/unsupported by default; enable only via verified modem-specific implementation. |
| Hotspot and uplink | Same source selector (mobile/Ethernet/active Wi-Fi/VPN/auto), health and actual client egress status. VPN choice remains unavailable unless protected forwarding is verified. | Existing AP start/stop; no uplink selection or VPN forwarding yet. Show adapter/concurrent mode and permission result. | Use OS hotspot/Internet Connection Sharing backend; source choices depend on API and network adapter combinations. Elevation or policy may block sharing. | Prefer system sharing integration only if source and client route can be inspected; NetworkExtension VPN access may require entitlement. Do not equate a host VPN with client VPN egress. |
| Connections and failover | Shared route graph, priority list, manual/automatic state and transition/rollback flow. | NetworkManager route metrics currently change priority only; no automatic health-driven failover. | Query system routes; route mutation needs a privileged and reversible implementation. Distinguish OS auto-connect from ModemDeck failover. | Inspect routes and VPN overlays; custom routing requires an authorized helper. Show failover unsupported until verified. |
| Bands and Analyzer | Shared band models, mode selection, time/progress/cancel, signal/performance/stability, sample confidence, recommendation reason and transaction rollback. | Existing bands and analyzer implementations; some cell-lock support is modem/firmware-specific. | Use MBN or vendor device service. Unsupported/unknown band controls stay disabled with reason; analyzer can still run only metrics available. | Requires modem vendor adapter. Analyzer should display missing metrics and not fabricate cellular band/cell data. |
| Operator/APN profiles | Shared APN list/editor, write-only secret change, backup and impact confirmation. | Current profile operations use the Linux connection manager and secret helper. | Windows mobile broadband profiles/provisioning APIs; API access may be restricted by app identity/privileges. | Vendor modem profile adapter; if OS/vendor does not expose profile edit, offer read-only state or unsupported reason. |
| SIM, SMS, voice, USSD, eSIM | Same sections and privacy rules; each control gated independently. Call control and voice/audio path are separate capabilities. | Current ModemManager-backed services cover some PIN/SMS/call/USSD operations; eSIM depends on eUICC and helper availability. | MBN includes SIM PIN, SMS, USSD and device service surfaces, but access/driver/metadata restrictions apply; call audio and eSIM can be unavailable. | Per-device/vendor support only; do not assume public desktop APIs for SIM/SMS/eSIM. Enable only what the detected backend verifies. |
| GPS/location and antenna position | Same explicit location privacy controls, fix freshness, labeled position samples, baseline and aggregate-only antenna metrics. | GNSS may be exposed by modem; current read is point-based, not continuous tracking. | GNSS data may come from modem device services or Windows location services; ask for applicable location access and distinguish system location from modem GNSS. | GNSS requires device/vendor support; location permission and storage policy must be explicit. Aggregate RF only unless distinct antenna chains are verified. |
| Hardware/firmware | Same detected/confirmed/unknown compatibility labels and official update instructions. | Sysfs, hardware database and fwupd/LVFS inspection; flashing not provided by ModemDeck. | SetupAPI/driver inventory and official vendor/Windows update information; do not promise firmware updates through Windows Update universally. | System information and vendor support metadata; no generic modem firmware flasher. |
| Diagnostics/history/reports | Same checks and history schema with source, timestamp, evidence, freshness and redacted export. | Current doctor has OS-specific checks; present readable generic results, and put technical service names under Platform details. Extend route/VPN/hotspot checks. | Implement service/device/network probes per APIs; report denied permissions distinctly. | Implement SystemConfiguration/NetworkExtension/vendor probes; unsupported checks are omitted or shown unavailable, not failed. |
| Backup/settings/scheduler | Shared settings backup, schema preview, diff and rollback. Shared UI language, density, theme, accessibility and complexity. Scheduler shows “configured” vs “running.” | Current backup export/restore logic and persisted scheduler config; no active scheduler loop found. | Per-user app data and credential vault; schedule runner is not implemented. | App container/keychain and Service Management consent; scheduler runner is not implemented. |

### Permissions, background work, tray and updates UX

- A common **Platform & access** settings page lists Device access, Network management, Location (only if requested), Background monitoring, Notifications, Secret storage and Update channel. Each row says `Ready`, `Needs permission`, `Not installed`, `Unsupported`, or `Unknown`, with a user action and last check.
- Ask permission at the point of use, explain the concrete action first, and return to the same operation after approval. A denied permission leaves safe read-only functions available.
- Background monitoring has a separate state from “app starts at login.” Show which helper is installed, its scope (user/system), whether it is approved, and an action to open the OS settings. A disabled helper degrades to foreground-only operation.
- Tray/menu-bar icon is optional. The same menu items are Open, current connection summary, pause/resume monitoring, refresh, diagnostics and Quit; platform integration follows the native host and is omitted if unavailable.
- Updates always identify source/channel, version, signature state and restart impact. Modem firmware is a separate flow from ModemDeck application updates.

### Architecture evolution in Slint and Rust

Use one `PlatformCapabilitySnapshot` passed into Slint; don't branch page construction on OS. For each capability include `status`, `reason_key`, `action_key`, `source`, `checked_at`, and optional technical detail. Slint maps the reason/action keys through `Tr` and offers a disclosure for localized platform-specific instructions. Add adapters behind traits such as `ModemBackend`, `NetworkBackend`, `HotspotBackend`, `RoutePolicyBackend`, `SecretStoreBackend`, `BackgroundTaskBackend`, `NotificationBackend`, and `UpdateBackend`. Keep platform crates in target-specific Cargo dependencies and domain models/tests OS-independent.

`PlatformOs` now identifies Linux, Windows, macOS and `Other`, but that only identifies the host. `PlatformCapabilities` still uses coarse Linux/Windows-oriented booleans, while non-Linux results are all false. That is insufficient to distinguish unsupported hardware, denied permission and missing optional software. The feature-status contract above is required before the shared UI can make truthful per-platform decisions.

## Platform API references

- Linux [ModemManager overview](https://modemmanager.org/) and [NetworkManager developer API](https://www.networkmanager.dev/docs/developers/).
- Windows [Mobile Broadband API overview](https://learn.microsoft.com/en-us/windows/win32/api/_mbn/) and [Mobile Broadband Runtime APIs and privilege requirements](https://learn.microsoft.com/en-us/windows-hardware/drivers/mobilebroadband/list-of-mobile-broadband-windows-runtime-apis).
- macOS [CoreWLAN interface](https://developer.apple.com/documentation/corewlan/cwinterface), [NetworkExtension](https://developer.apple.com/documentation/NetworkExtension), [Keychain Services](https://developer.apple.com/documentation/security/keychain-services), and [Service Management](https://developer.apple.com/documentation/servicemanagement/smappservice).

These references describe possible integration surfaces, not guaranteed feature availability. Actual support remains conditional on the platform version, app packaging/entitlements, active OS services, modem hardware, driver and vendor implementation.

The current Rust UI sync selects `modems.first()` for the main summary, and multiple handlers resolve `mm_idx` from the first modem. A reliable multi-modem redesign needs a selected modem ID stored in the view model and used by each modem-specific action. Do not suggest all pages honor selection until that routing is complete.
