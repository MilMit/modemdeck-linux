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
DIST_DIR="${MODEMDECK_DIST_DIR:-$ROOT_DIR/target/dist}"
WORK="$ROOT_DIR/target/appimage"
APPDIR="$WORK/ModemDeck.AppDir"
TOOLS="$WORK/tools"
rm -rf "$APPDIR" "$TOOLS"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
         "$APPDIR/usr/share/metainfo" \
         "$APPDIR/usr/lib/systemd/user" \
         "$APPDIR/usr/share/gnome-shell/extensions/modemdeck@milmit.net" \
         "$TOOLS"

cargo build --release --locked
install -m 0755 target/release/modemdeck "$APPDIR/usr/bin/modemdeck"
install -m 0644 data/net.milmit.ModemDeck.desktop "$APPDIR/usr/share/applications/net.milmit.ModemDeck.desktop"
install -m 0644 assets/icons/net.milmit.ModemDeck.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg"
install -m 0644 data/net.milmit.ModemDeck.metainfo.xml "$APPDIR/usr/share/metainfo/net.milmit.ModemDeck.metainfo.xml"
install -m 0644 data/net.milmit.ModemDeck-monitor.service "$APPDIR/usr/lib/systemd/user/net.milmit.ModemDeck-monitor.service"
cp -a gnome-extension/modemdeck@milmit.net/. "$APPDIR/usr/share/gnome-shell/extensions/modemdeck@milmit.net/"

LINUXDEPLOY="$TOOLS/linuxdeploy-${LD_ARCH}.AppImage"
LINUXDEPLOY_VERSION="1-alpha-20251107-1"
URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-${LD_ARCH}.AppImage"
case "$LD_ARCH" in
  x86_64) LINUXDEPLOY_SHA256=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d ;;
  aarch64) LINUXDEPLOY_SHA256=620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff ;;
esac
if command -v curl >/dev/null 2>&1; then
  curl -fL "$URL" -o "$LINUXDEPLOY"
else
  wget -O "$LINUXDEPLOY" "$URL"
fi
echo "$LINUXDEPLOY_SHA256  $LINUXDEPLOY" | sha256sum --check --status || {
  echo "linuxdeploy download checksum did not match the pinned upstream release" >&2
  exit 1
}
chmod +x "$LINUXDEPLOY"

# linuxdeploy follows the executable's shared-library dependency chain and bundles it.
# ModemManager/NetworkManager/Polkit are intentionally NOT bundled: they are host
# system services and a second private copy would conflict with the running OS.
export OUTPUT="ModemDeck-${VERSION}-linux-${APP_ARCH}.AppImage"
export LDAI_OUTPUT="$OUTPUT"
"$LINUXDEPLOY" --appimage-extract-and-run \
  --appdir "$APPDIR" \
  --executable "$APPDIR/usr/bin/modemdeck" \
  --desktop-file "$APPDIR/usr/share/applications/net.milmit.ModemDeck.desktop" \
  --icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg" \
  --output appimage

FOUND="$ROOT_DIR/$OUTPUT"
if [[ ! -f "$FOUND" ]]; then
  FOUND="$(find "$ROOT_DIR" -maxdepth 1 -type f -name '*ModemDeck*.AppImage' -print -quit)"
fi
if [[ -n "$FOUND" && -f "$FOUND" ]]; then
  mkdir -p "$DIST_DIR"
  mv "$FOUND" "$DIST_DIR/$OUTPUT"
  echo "$DIST_DIR/$OUTPUT"
else
  echo "AppImage output was not found after appimagetool completed." >&2
  exit 1
fi
