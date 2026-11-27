# Real Hardware Test Guide — Dell DW5821e (Snapdragon X20 / Foxconn T77W968)

1. **Verify Linux Detection**:
   ```bash
   lsusb | grep -i 413c
   mmcli -L
   modemdeck list-modems --json
   ```
   - `413c:81d7`: Standard DW5821e (physical SIM only).
   - `413c:81e0`: DW5821e-eSIM SKU (integrated eUICC).
2. **Check FCC Lock & Modem State**:
   ```bash
   modemdeck doctor
   ```
   If ModemManager reports `disabled` due to FCC lock, enable the official ModemManager `fcc-unlock.d` symlink for `413c:81d7` / `413c:81e0` and restart `ModemManager`.
3. **Verify Operator & Auto APN**:
   ```bash
   modemdeck operator info
   modemdeck apn list
   ```
4. **Verify Signal, CA & Band Lock (with Crash-Safe Rollback)**:
   ```bash
   modemdeck signal
   modemdeck bands list
   ```
   Test band lock in the GUI (`Network` -> `Band manager`), confirm connectivity within the rollback window, and verify `ModemDeck` restores previous bands automatically if unconfirmed.
5. **Verify Diagnostics & Safe AT Terminal**:
   - Run `modemdeck speedtest` to verify domestic vs international probes.
   - In `Pro Tools -> Safe AT Command Terminal`, test read-only commands (`ATI`, `AT+CSQ`, `AT^CELLINFO`, `AT^CA_INFO`).
