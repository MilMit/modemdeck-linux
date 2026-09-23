# ModemDeck by MilMit — RC1 target checklist

Run these on the target Ubuntu machine before tagging v1.0 stable.

## Build

```bash
cargo run --release
./scripts/phase5-selfcheck.sh
```

## Core modem

- DW5821e is detected after cold boot.
- USB/internal WWAN removal/addition updates the UI without restarting ModemDeck.
- Suspend then resume: signal telemetry returns and no duplicate process/timer appears.

## Safe band controls

1. Start on a known-working band/profile.
2. Apply a different working band and do **not** confirm.
3. Verify previous bands return after about 45 seconds.
4. Repeat, kill the GUI during the countdown, and verify the detached rollback guard still restores the previous bands.
5. Apply/confirm B42 and verify the pending transaction file disappears.

## Verified radio telemetry

- B1: FDD-LTE read works.
- B42: TDD-LTE read works.
- 3CA B1+B7+B3 is parsed as PCC/SCC components.
- 2CA B42+B42 is parsed as PCC/SCC components.
- `4G+` is shown only when CA is actually observed.

## APN

- List current NetworkManager GSM profiles.
- Create a test APN.
- Edit APN/username/autoconnect.
- Optional password can be written but is never shown back in ModemDeck.
- Activate profile.
- Delete test profile.

## SIM

Use only a SIM whose PIN is known.

- Current SIM lock state is displayed.
- Unlock works when SIM is actually PIN-locked.
- Enable/disable PIN protection works and errors are surfaced cleanly.

## Traffic

- RX/TX changes while downloading/uploading.
- Counters do not generate traffic themselves.
- Daily/monthly totals increase.
- Reset works.
- Background monitor continues accounting when GUI is closed (if enabled).

## Profiles / boot restore

- Auto, B42 TD-LTE and B1+B3+B7 exist.
- User profile save/delete works.
- Active profile can restore after log in/reboot when background monitor is enabled.
- Resume does not overwrite an in-progress unconfirmed band transaction.

## Desktop integration

- GNOME indicator loads after installation/login.
- Notifications are optional.
- English/Persian selection works after restart.
- Supported Modems, Release Notes, About, diagnostics and update-check pages open.
- Diagnostics contain no IMEI/equipment ID.

## Packaging

```bash
./packaging/build-deb.sh
sudo apt install ./target/modemdeck_1.0.0-rc1.1_*.deb
```

Check install, upgrade and uninstall. Final stable release should be cut only after this checklist passes.
