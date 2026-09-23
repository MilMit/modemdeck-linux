#!/usr/bin/env bash
set -euo pipefail

UUID="modemdeck@milmit.net"
if command -v systemctl >/dev/null 2>&1; then
  systemctl --user disable --now net.milmit.ModemDeck-monitor.service >/dev/null 2>&1 || true
fi
if command -v gnome-extensions >/dev/null 2>&1; then
  gnome-extensions disable "$UUID" >/dev/null 2>&1 || true
fi
rm -f "$HOME/.local/bin/modemdeck"
rm -f "$HOME/.local/share/applications/net.milmit.ModemDeck.desktop"
rm -f "$HOME/.config/systemd/user/net.milmit.ModemDeck-monitor.service"
rm -rf "$HOME/.local/share/gnome-shell/extensions/$UUID"
rm -f "${XDG_RUNTIME_DIR:-/tmp}/modemdeck-status.json" "${XDG_RUNTIME_DIR:-/tmp}/modemdeck-history.json"
if command -v systemctl >/dev/null 2>&1; then
  systemctl --user daemon-reload >/dev/null 2>&1 || true
fi
echo "ModemDeck user installation removed. Preferences under ~/.config/modemdeck were kept."
