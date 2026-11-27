#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
./scripts/validate-release.sh
./packaging/build-all.sh
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
echo "Local artifacts prepared for v${VERSION}. No commit, tag, push, or GitHub release was created."
echo "Review artifact checksums and the working tree, then obtain maintainer approval for publishing."
