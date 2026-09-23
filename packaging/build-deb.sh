#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

command -v cargo >/dev/null 2>&1 || { echo "cargo is required to build the developer package" >&2; exit 1; }
command -v dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb is required" >&2; exit 1; }

VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
PKGROOT="$ROOT_DIR/target/deb-root"
mkdir -p "$ROOT_DIR/target/dist"
OUT="$ROOT_DIR/target/dist/MilMit-ModemDeck-${VERSION}-${ARCH}.deb"
rm -rf "$PKGROOT"
mkdir -p "$PKGROOT/DEBIAN" \
         "$PKGROOT/usr/bin" \
         "$PKGROOT/usr/share/applications" \
         "$PKGROOT/usr/share/icons/hicolor/scalable/apps" \
         "$PKGROOT/usr/lib/systemd/user" \
         "$PKGROOT/usr/share/gnome-shell/extensions/modemdeck@milmit.net"

cargo build --release
install -m 0755 target/release/modemdeck "$PKGROOT/usr/bin/modemdeck"
install -m 0644 data/net.milmit.ModemDeck.desktop "$PKGROOT/usr/share/applications/net.milmit.ModemDeck.desktop"
install -m 0644 assets/icons/net.milmit.ModemDeck.svg "$PKGROOT/usr/share/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg"
install -m 0644 data/net.milmit.ModemDeck-monitor.service "$PKGROOT/usr/lib/systemd/user/net.milmit.ModemDeck-monitor.service"
cp -a gnome-extension/modemdeck@milmit.net/. "$PKGROOT/usr/share/gnome-shell/extensions/modemdeck@milmit.net/"

cat > "$PKGROOT/DEBIAN/control" <<CONTROL
Package: modemdeck
Version: $VERSION
Section: net
Priority: optional
Architecture: $ARCH
Maintainer: MilMit
Depends: libgtk-4-1, modemmanager, network-manager, policykit-1
Homepage: https://milmit.net
Description: ModemDeck by MilMit — lightweight graphical cellular modem manager for Linux
 ModemDeck by MilMit provides live modem telemetry, safe band controls, APN and SIM
 management, SMS, calls/USSD when supported, data usage, profiles, multi-vendor
 diagnostics and GNOME integration.
CONTROL

cat > "$PKGROOT/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
command -v gtk4-update-icon-cache >/dev/null 2>&1 && gtk4-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
command -v systemctl >/dev/null 2>&1 && systemctl --user daemon-reload >/dev/null 2>&1 || true
exit 0
POSTINST
chmod 0755 "$PKGROOT/DEBIAN/postinst"

cat > "$PKGROOT/DEBIAN/postrm" <<'POSTRM'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
command -v gtk4-update-icon-cache >/dev/null 2>&1 && gtk4-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
exit 0
POSTRM
chmod 0755 "$PKGROOT/DEBIAN/postrm"

dpkg-deb --build --root-owner-group "$PKGROOT" "$OUT"
echo "$OUT"
