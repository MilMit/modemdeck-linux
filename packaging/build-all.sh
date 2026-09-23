#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
mkdir -p target/dist
DEB="$(./packaging/build-deb.sh)"
cp "$DEB" target/dist/
./packaging/build-appimage.sh
printf '\nBuilt release artifacts:\n'
find target/dist -maxdepth 1 -type f -printf '  %f\n' | sort
