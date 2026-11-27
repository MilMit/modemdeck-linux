#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-full}"
if [[ "$MODE" != full && "$MODE" != --metadata-only ]]; then
  echo "Usage: $0 [--metadata-only]" >&2
  exit 2
fi

VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["packages"][0]["version"])')"
[[ -n "$VERSION" ]] || { echo "Could not read the Cargo package version" >&2; exit 1; }
if command -v dpkg >/dev/null 2>&1; then
  DEB_VERSION="$(printf '%s' "$VERSION" | sed 's/-/~/')-1"
  dpkg --validate-version "$DEB_VERSION"
fi

echo "Validating ModemDeck $VERSION"
if [[ "$MODE" == full ]]; then
  cargo fmt --all -- --check
  cargo check --locked --all-targets
  cargo clippy --locked --all-targets --all-features -- -D warnings
  cargo test --locked --all-targets
  cargo build --release --locked
fi

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate data/net.milmit.ModemDeck.desktop
else
  echo "desktop-file-validate unavailable; skipped" >&2
fi
if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli validate --no-net --pedantic data/net.milmit.ModemDeck.metainfo.xml
else
  echo "appstreamcli unavailable; skipped" >&2
fi

grep -Fq "<release version=\"$VERSION\"" data/net.milmit.ModemDeck.metainfo.xml || {
  echo "AppStream release version does not match Cargo.toml ($VERSION)" >&2; exit 1;
}
PKG_VERSION="${VERSION//-/.}"
RPM_VERSION="$(printf '%s' "$VERSION" | sed 's/-/~/')"
grep -Fxq "version: '$VERSION'" snap/snapcraft.yaml || {
  echo "Snap version does not match Cargo.toml ($VERSION)" >&2; exit 1;
}
grep -Fxq "pkgver=$PKG_VERSION" packaging/PKGBUILD || {
  echo "Arch package version does not match Cargo.toml ($VERSION)" >&2; exit 1;
}
grep -Fxq "Version:        $RPM_VERSION" packaging/modemdeck.spec || {
  echo "RPM package version does not match Cargo.toml ($VERSION)" >&2; exit 1;
}
grep -Fq "tag=v$VERSION" packaging/.SRCINFO || {
  echo "AUR source tag does not match Cargo.toml ($VERSION)" >&2; exit 1;
}
CURRENT_TAG="$(git describe --tags --exact-match HEAD 2>/dev/null || true)"
if git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null && [[ "$CURRENT_TAG" != "v$VERSION" ]]; then
  echo "Release tag v$VERSION already exists on another commit" >&2
  exit 1
fi
if [[ -n "$(git remote get-url origin 2>/dev/null || true)" && "$CURRENT_TAG" != "v$VERSION" ]] && \
   git ls-remote --exit-code --tags origin "refs/tags/v$VERSION" >/dev/null 2>&1; then
  echo "Release tag v$VERSION already exists on origin" >&2
  exit 1
fi

if [[ "$MODE" == full ]]; then
  echo "Local release quality gates passed for $VERSION. Hardware and package-store validation are separate."
else
  echo "Release metadata checks passed for $VERSION."
fi
