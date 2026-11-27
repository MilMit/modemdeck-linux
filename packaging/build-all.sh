#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
case "$(uname -m)" in
  x86_64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) echo "Unsupported build architecture: $(uname -m)" >&2; exit 1 ;;
esac
export MODEMDECK_DIST_DIR="$ROOT_DIR/target/dist/$VERSION/linux-$ARCH"
mkdir -p "$MODEMDECK_DIST_DIR"
./packaging/build-deb.sh
./packaging/build-portable.sh
./packaging/build-appimage.sh
./scripts/generate-checksums.sh "$MODEMDECK_DIST_DIR"
printf '\nBuilt release artifacts:\n'
find "$MODEMDECK_DIST_DIR" -maxdepth 1 -type f -printf '  %f\n' | sort
