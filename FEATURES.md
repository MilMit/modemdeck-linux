# RC1 feature matrix

| Feature | RC1 status |
|---|---|
| APN create/edit/select/delete | Implemented via NetworkManager |
| APN optional password | Write-only to NetworkManager; not stored by ModemDeck |
| SIM PIN unlock | Implemented via ModemManager SIM D-Bus |
| SIM PIN enable/disable | Implemented via ModemManager SIM D-Bus |
| Daily/monthly data usage | Implemented locally from kernel interface counters |
| Live RX/TX rate | Implemented; no speed test |
| Built-in band profiles | Auto, B42 TD-LTE, B1+B3+B7 |
| User band profiles | Save/delete/apply |
| Restore profile after boot/resume | Optional background monitor |
| Suspend/resume | systemd-logind signal |
| WWAN hot-plug | ModemManager object events |
| Supported Modems page | Implemented |
| Privacy-safe diagnostics | IMEI/equipment IDs omitted |
| Check for updates | MilMit JSON manifest |
| Report problem / milmit.net | Implemented |
| English/Persian UI | Preference + translation layer |
| About / release notes | Implemented |
| 45-second rollback | Implemented |
| Crash-safe rollback | Persistent transaction + detached tokenized guard |
| Pre-change backups | Implemented and bounded |
| Exact CA components | PCC/SCC role, band and MHz |
| Cell-lock writes | Preview-only by design in RC1 |

## Communications (RC1.2)

- SMS: runtime detected; list/send/delete over ModemManager Messaging.
- Voice: runtime detected; dial/list/answer/hangup over ModemManager Voice.
- USSD: runtime detected; initiate/respond/cancel over ModemManager 3GPP USSD.
- Call audio: never assumed from call control alone; depends on the modem firmware and exposed audio path.
