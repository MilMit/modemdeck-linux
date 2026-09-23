#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
SYSTEMD_DIR="${HOME}/.config/systemd/user"

command -v cargo >/dev/null 2>&1 || {
  echo "cargo not found. Run ./scripts/install-deps-ubuntu.sh first." >&2
  exit 1
}

cd "$ROOT_DIR"
cargo build --release

mkdir -p "$BIN_DIR" "$APP_DIR" "$SYSTEMD_DIR"
install -m 0755 target/release/modemdeck "$BIN_DIR/modemdeck"
install -m 0644 data/net.milmit.ModemDeck.desktop "$APP_DIR/net.milmit.ModemDeck.desktop"
install -m 0644 data/net.milmit.ModemDeck-monitor.service "$SYSTEMD_DIR/net.milmit.ModemDeck-monitor.service"

if command -v systemctl >/dev/null 2>&1; then
  systemctl --user daemon-reload >/dev/null 2>&1 || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi

# Install the GNOME indicator files when GNOME is available. Enabling may need
# one logout/login depending on the Shell version.
if command -v gnome-shell >/dev/null 2>&1 || command -v gnome-extensions >/dev/null 2>&1; then
  "$ROOT_DIR/scripts/install-gnome-extension.sh" || true
fi

echo "ModemDeck installed for ${USER}."
echo "Launch it from the application menu or run: modemdeck"
echo "Enable the low-overhead monitor from ModemDeck Preferences when desired."
