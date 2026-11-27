#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
command -v git >/dev/null || { echo "git is required" >&2; exit 1; }

VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
DIST_DIR="${MODEMDECK_DIST_DIR:-$ROOT_DIR/target/dist}"
mkdir -p "$DIST_DIR"
git archive --format=tar --prefix="ModemDeck-${VERSION}/" HEAD | gzip -n > "$DIST_DIR/ModemDeck-${VERSION}-source.tar.gz"
tar -tzf "$DIST_DIR/ModemDeck-${VERSION}-source.tar.gz" >/dev/null
echo "$DIST_DIR/ModemDeck-${VERSION}-source.tar.gz"
