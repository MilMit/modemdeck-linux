#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UUID="modemdeck@milmit.net"
SOURCE="$ROOT_DIR/gnome-extension/$UUID"
DEST="$HOME/.local/share/gnome-shell/extensions/$UUID"

mkdir -p "$(dirname "$DEST")"
rm -rf "$DEST"
cp -a "$SOURCE" "$DEST"

echo "Installed GNOME extension: $UUID"
if command -v gnome-extensions >/dev/null 2>&1; then
  if gnome-extensions enable "$UUID" 2>/dev/null; then
    echo "Extension enabled."
  else
    echo "GNOME may require you to log out and back in before enabling it."
    echo "Then run: gnome-extensions enable $UUID"
  fi
else
  echo "gnome-extensions command not found. Enable '$UUID' from Extensions after logging in again."
fi
