# Windows Porting Guide — ModemDeck

ModemDeck is architected with a clean platform abstraction (`src/platform/mod.rs`) so core domain logic (Hardware DB, Global Operator DB, Signal Scoring, Band Analyzer, Stability Analysis, Usage Tracking, Diagnostics, and UI models) has zero direct dependency on Linux D-Bus, ModemManager, or systemd.

## Shared Core Modules (OS-Independent)

- `src/model.rs` — Unified modem, signal, SIM, serving cell, and band models
- `src/hardware/` — Hardware compatibility DB (`resources/hardware/*.json`)
- `src/operators/` — Global operator DB & MVNO identification
- `src/analyzer/` — Signal scoring, band scan orchestration, CA & stability analysis
- `src/antenna/`, `src/cells/`, `src/netdiag/`, `src/failover/`, `src/scheduler/`, `src/backup/`

## Required Windows Backend Implementations

To ship a native Windows build, implement the `src/platform/mod.rs` traits under `#[cfg(target_os = "windows")]`:

1. **`ModemBackend`**:
   - Primary: WinRT `Windows.Networking.NetworkOperators.MobileBroadbandAccount` & `MobileBroadbandModem`
   - Low-level / Vendor extensions: Windows MBIM Device Services (`IMbnInterface` / `IMbnConnection` or Win32 `DeviceIoControl` on MBIM device path)
   - Device discovery: `SetupAPI` (`SetupDiGetClassDevs`) / `CfgMgr32` for USB/PCIe VID/PID & COM/AT ports
2. **`NetworkBackend`**:
   - Windows IP Helper API (`GetAdaptersAddresses`, `GetIfEntry2`) and `NetworkInformation` for metered state
3. **`SecretStoreBackend`**:
   - Windows Credential Manager (`CredWriteW` / `CredReadW` / `CredDeleteW`)
4. **`FirmwareBackend`**:
   - Windows Update / SetupAPI driver & firmware revision queries
5. **Background Service**:
   - Windows Service Control Manager (SCM) or Windows Background Task
6. **Packaging**:
   - MSI / MSIX installer with GTK4 Windows runtime bundle
