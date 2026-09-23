#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

command -v cargo >/dev/null 2>&1 || { echo "cargo is required to build AppImage" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || { echo "curl or wget is required only on the build machine" >&2; exit 1; }

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64) LD_ARCH="x86_64"; APP_ARCH="x86_64" ;;
  aarch64|arm64) LD_ARCH="aarch64"; APP_ARCH="aarch64" ;;
  *) echo "Unsupported AppImage build architecture: $ARCH_RAW" >&2; exit 1 ;;
esac
VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
WORK="$ROOT_DIR/target/appimage"
APPDIR="$WORK/ModemDeck.AppDir"
TOOLS="$WORK/tools"
rm -rf "$APPDIR" "$TOOLS"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
         "$APPDIR/usr/lib/systemd/user" \
         "$APPDIR/usr/share/gnome-shell/extensions/modemdeck@milmit.net" \
         "$TOOLS"

cargo build --release
install -m 0755 target/release/modemdeck "$APPDIR/usr/bin/modemdeck"
install -m 0644 data/net.milmit.ModemDeck.desktop "$APPDIR/usr/share/applications/net.milmit.ModemDeck.desktop"
install -m 0644 assets/icons/net.milmit.ModemDeck.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg"
install -m 0644 data/net.milmit.ModemDeck-monitor.service "$APPDIR/usr/lib/systemd/user/net.milmit.ModemDeck-monitor.service"
cp -a gnome-extension/modemdeck@milmit.net/. "$APPDIR/usr/share/gnome-shell/extensions/modemdeck@milmit.net/"

# GTK/GIO may consult GSettings schemas at runtime. Bundle the build host's
# schemas into the AppImage so normal GUI startup does not depend on matching
# GTK data files being installed on the target machine.
if [[ -d /usr/share/glib-2.0/schemas ]]; then
  mkdir -p "$APPDIR/usr/share/glib-2.0/schemas"
  cp -a /usr/share/glib-2.0/schemas/*.xml "$APPDIR/usr/share/glib-2.0/schemas/" 2>/dev/null || true
  if command -v glib-compile-schemas >/dev/null 2>&1; then
    glib-compile-schemas "$APPDIR/usr/share/glib-2.0/schemas"
  fi
fi

LINUXDEPLOY="$TOOLS/linuxdeploy-${LD_ARCH}.AppImage"
URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${LD_ARCH}.AppImage"
if command -v curl >/dev/null 2>&1; then
  curl -fL "$URL" -o "$LINUXDEPLOY"
else
  wget -O "$LINUXDEPLOY" "$URL"
fi
chmod +x "$LINUXDEPLOY"

# linuxdeploy follows the executable's shared-library dependency chain and bundles it.
# ModemManager/NetworkManager/Polkit are intentionally NOT bundled: they are host
# system services and a second private copy would conflict with the running OS.
export OUTPUT="MilMit-ModemDeck-${VERSION}-${APP_ARCH}.AppImage"
"$LINUXDEPLOY" --appimage-extract-and-run \
  --appdir "$APPDIR" \
  --executable "$APPDIR/usr/bin/modemdeck" \
  --desktop-file "$APPDIR/usr/share/applications/net.milmit.ModemDeck.desktop" \
  --icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg" \
  --output appimage

FOUND="$(find . -maxdepth 1 -type f -name 'ModemDeck*.AppImage' -print -quit)"
if [[ -n "$FOUND" ]]; then
  mkdir -p target/dist
  mv "$FOUND" "target/dist/$OUTPUT"
  echo "$ROOT_DIR/target/dist/$OUTPUT"
else
  echo "AppImage output was not found." >&2
  exit 1
fi
