#!/usr/bin/env bash
set -u

ok() { printf 'OK   %s\n' "$*"; }
warn() { printf 'WARN %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*"; }

if command -v modemdeck >/dev/null 2>&1; then ok "modemdeck binary: $(command -v modemdeck)"; else warn "modemdeck is not installed in PATH"; fi
if systemctl is-active --quiet ModemManager 2>/dev/null; then ok "ModemManager is active"; else fail "ModemManager is not active"; fi
if command -v mmcli >/dev/null 2>&1; then
  COUNT="$(mmcli -L 2>/dev/null | grep -c '/Modem/' || true)"
  ok "ModemManager modem objects: $COUNT"
else
  warn "mmcli is unavailable"
fi

if command -v nmcli >/dev/null 2>&1; then ok "NetworkManager CLI available"; else fail "nmcli is unavailable (APN Manager needs NetworkManager)"; fi
if command -v pkexec >/dev/null 2>&1; then ok "PolicyKit pkexec available"; else warn "pkexec unavailable (privileged vendor fallback cannot run)"; fi
ok "MilMit update checks are built into the Rust binary (no curl runtime dependency)"

if systemctl --user status net.milmit.ModemDeck-monitor.service >/dev/null 2>&1; then
  STATE="$(systemctl --user is-active net.milmit.ModemDeck-monitor.service 2>/dev/null || true)"
  ok "background monitor service: ${STATE:-unknown}"
else
  warn "background monitor user service is not installed/enabled"
fi

RUNTIME="${XDG_RUNTIME_DIR:-/tmp}"
if [[ -f "$RUNTIME/modemdeck-status.json" ]]; then ok "runtime status file exists"; else warn "runtime status file not found"; fi
if [[ -f "$RUNTIME/modemdeck-history.json" ]]; then ok "signal history file exists"; else warn "signal history file not found yet"; fi

UUID="modemdeck@milmit.net"
if [[ -d "$HOME/.local/share/gnome-shell/extensions/$UUID" || -d "/usr/share/gnome-shell/extensions/$UUID" ]]; then
  ok "GNOME top-bar extension files installed"
  if command -v gnome-extensions >/dev/null 2>&1; then
    if gnome-extensions list --enabled 2>/dev/null | grep -qx "$UUID"; then ok "GNOME indicator enabled"; else warn "GNOME indicator installed but not enabled"; fi
  fi
else
  warn "GNOME top-bar extension not installed"
fi


CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}/modemdeck"
if [[ -f "$CONFIG_HOME/profiles.json" ]]; then
  ok "band profiles database exists"
else
  warn "band profiles database not created yet"
fi
if [[ -f "$CONFIG_HOME/data-usage.json" ]]; then
  ok "data usage database exists"
else
  warn "data usage database not created yet"
fi
if [[ -f "$CONFIG_HOME/pending-band-change.json" ]]; then
  warn "an unconfirmed band transaction is pending"
else
  ok "no pending band rollback transaction"
fi

if pgrep -x modemdeck >/dev/null 2>&1; then
  ps -C modemdeck -o pid=,pcpu=,pmem=,rss=,cmd=
else
  warn "no ModemDeck process is currently running"
fi
