# ModemDeck Linux v1.0.0-rc1.4.1

First tested release candidate of **ModemDeck Linux by MilMit**.

This build has passed the GitHub Actions Linux pipeline and the Debian package has been installed successfully on the target Ubuntu system.

## Included

- Native GTK4 Linux interface
- ModemManager / NetworkManager integration
- LTE / TD-LTE / 4G+ telemetry
- Safe band controls with rollback
- APN and SIM controls
- SMS, calls and USSD when exposed by ModemManager
- Live RX/TX and usage accounting
- Multi-vendor diagnostics
- GNOME integration
- Debian package and AppImage outputs

## Install on Ubuntu

Download `MilMit-ModemDeck-1.0.0-rc1.4.1-amd64.deb` and run:

```bash
sudo apt install ./MilMit-ModemDeck-1.0.0-rc1.4.1-amd64.deb
```

The package requires GTK4, ModemManager and NetworkManager. `pkexec` is recommended for privileged vendor-specific reads but is not a hard install dependency.

## Status

This is an RC release, not the final stable 1.0 tag. Advanced modem behavior still varies by modem model and firmware, and cell-lock writes remain preview-only by design.
