#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
command -v cargo >/dev/null 2>&1 || { echo "cargo is required" >&2; exit 1; }
command -v rpmbuild >/dev/null 2>&1 || { echo "rpmbuild is required" >&2; exit 1; }
command -v git >/dev/null 2>&1 || { echo "git is required" >&2; exit 1; }

VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|aarch64) ;;
  *) echo "Unsupported RPM architecture: $ARCH" >&2; exit 1 ;;
esac

TOPDIR="$ROOT_DIR/target/rpmbuild"
DIST_DIR="${MODEMDECK_DIST_DIR:-$ROOT_DIR/target/dist}"
mkdir -p "$TOPDIR/SOURCES" "$DIST_DIR"
SOURCE="$TOPDIR/SOURCES/v${VERSION}.tar.gz"
git archive --format=tar --prefix="modemdeck-linux-${VERSION}/" HEAD | gzip -n > "$SOURCE"
tar -tzf "$SOURCE" >/dev/null

rpmbuild -bb \
  --define "_topdir $TOPDIR" \
  --define "_rpmdir $DIST_DIR" \
  --define "_srcrpmdir $DIST_DIR" \
  "$ROOT_DIR/packaging/modemdeck.spec"

find "$DIST_DIR" -type f -name '*.rpm' -print
