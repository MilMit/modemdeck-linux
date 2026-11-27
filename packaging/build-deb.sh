#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

command -v cargo >/dev/null 2>&1 || { echo "cargo is required to build the developer package" >&2; exit 1; }
command -v dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb is required" >&2; exit 1; }

VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
DEB_VERSION="$(printf '%s' "$VERSION" | sed 's/-/~/')-1"
dpkg --validate-version "$DEB_VERSION"
DEB_ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
case "$DEB_ARCH" in
  amd64) ARTIFACT_ARCH=x86_64 ;;
  arm64) ARTIFACT_ARCH=aarch64 ;;
  *) echo "Unsupported Debian architecture: $DEB_ARCH" >&2; exit 1 ;;
esac
PKGROOT="$ROOT_DIR/target/deb-root-$DEB_ARCH"
DIST_DIR="${MODEMDECK_DIST_DIR:-$ROOT_DIR/target/dist}"
mkdir -p "$DIST_DIR"
OUT="$DIST_DIR/ModemDeck-${VERSION}-linux-${ARTIFACT_ARCH}.deb"
rm -rf "$PKGROOT"
mkdir -p "$PKGROOT/DEBIAN" "$PKGROOT/usr/bin" "$PKGROOT/usr/share/applications" \
  "$PKGROOT/usr/share/icons/hicolor/scalable/apps" "$PKGROOT/usr/share/metainfo" \
  "$PKGROOT/usr/share/doc/modemdeck" "$PKGROOT/usr/lib/systemd/user" \
  "$PKGROOT/usr/share/gnome-shell/extensions/modemdeck@milmit.net"

cargo build --release --locked
install -m 0755 target/release/modemdeck "$PKGROOT/usr/bin/modemdeck"
install -m 0644 data/net.milmit.ModemDeck.desktop "$PKGROOT/usr/share/applications/net.milmit.ModemDeck.desktop"
install -m 0644 assets/icons/net.milmit.ModemDeck.svg "$PKGROOT/usr/share/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg"
install -m 0644 data/net.milmit.ModemDeck.metainfo.xml "$PKGROOT/usr/share/metainfo/net.milmit.ModemDeck.metainfo.xml"
install -m 0644 LICENSE "$PKGROOT/usr/share/doc/modemdeck/copyright"
install -m 0644 CHANGELOG.md "$PKGROOT/usr/share/doc/modemdeck/changelog"
install -m 0644 data/net.milmit.ModemDeck-monitor.service "$PKGROOT/usr/lib/systemd/user/net.milmit.ModemDeck-monitor.service"
cp -a gnome-extension/modemdeck@milmit.net/. "$PKGROOT/usr/share/gnome-shell/extensions/modemdeck@milmit.net/"

cat > "$PKGROOT/DEBIAN/control" <<CONTROL
Package: modemdeck
Version: $DEB_VERSION
Section: net
Priority: optional
Architecture: $DEB_ARCH
Maintainer: MilMit
Depends: modemmanager, network-manager, libfontconfig1, libxkbcommon0, libudev1
Recommends: pkexec
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
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
exit 0
POSTINST
chmod 0755 "$PKGROOT/DEBIAN/postinst"

cat > "$PKGROOT/DEBIAN/postrm" <<'POSTRM'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
exit 0
POSTRM
chmod 0755 "$PKGROOT/DEBIAN/postrm"

dpkg-deb --build --root-owner-group "$PKGROOT" "$OUT"
echo "$OUT"
