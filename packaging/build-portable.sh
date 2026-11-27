#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }

VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
case "$(uname -m)" in
  x86_64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) echo "Unsupported portable build architecture: $(uname -m)" >&2; exit 1 ;;
esac

cargo build --release --locked
STAGE="$ROOT_DIR/target/portable/ModemDeck"
DIST_DIR="${MODEMDECK_DIST_DIR:-$ROOT_DIR/target/dist}"
OUT="$DIST_DIR/ModemDeck-${VERSION}-linux-${ARCH}.tar.gz"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/share/applications" "$STAGE/share/icons/hicolor/scalable/apps" "$STAGE/share/metainfo"
install -m 0755 target/release/modemdeck "$STAGE/bin/modemdeck"
install -m 0644 data/net.milmit.ModemDeck.desktop "$STAGE/share/applications/net.milmit.ModemDeck.desktop"
install -m 0644 assets/icons/net.milmit.ModemDeck.svg "$STAGE/share/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg"
install -m 0644 data/net.milmit.ModemDeck.metainfo.xml "$STAGE/share/metainfo/net.milmit.ModemDeck.metainfo.xml"
install -m 0644 LICENSE README.md CHANGELOG.md "$STAGE/"
cat > "$STAGE/INSTALL.txt" <<'EOF'
Run ./bin/modemdeck from this directory after extracting the archive.
This archive does not install desktop integration. ModemManager, NetworkManager,
D-Bus, a supported graphical session, and the native runtime libraries are still
provided by the host operating system. Use a distribution package for managed
desktop integration and package-manager updates.
EOF
mkdir -p "$DIST_DIR"
tar -C "$ROOT_DIR/target/portable" -czf "$OUT" ModemDeck
echo "$OUT"
