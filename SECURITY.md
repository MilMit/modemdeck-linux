# Security

Please do not publish security vulnerabilities, private modem identifiers, SIM data or credentials in a public issue.

For security reports, use the support/contact route on https://milmit.net and include the ModemDeck version and enough technical detail to reproduce the issue without exposing personal identifiers.

## Data Privacy & Safety Guarantees
- IMEI, IMSI, ICCID, and phone numbers are masked or omitted from privacy-safe diagnostics exports.
- SQLite (`modemdeck.db`) and JSON backups are stored locally in `$XDG_DATA_HOME/modemdeck` / `$XDG_CONFIG_HOME/modemdeck` with no telemetry sent to external servers.
- Band/profile locks use a 45-second confirmation window with detached crash-recovery rollback guards.
