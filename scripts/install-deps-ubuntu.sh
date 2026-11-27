#!/usr/bin/env bash
set -euo pipefail

sudo apt update
sudo apt install -y \
  build-essential \
  pkg-config \
  libudev-dev \
  libfontconfig1-dev \
  libxkbcommon-dev \
  modemmanager \
  network-manager \
  pkexec \
  curl

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

rustup toolchain install 1.98.1 --profile minimal --component rustfmt --component clippy

echo "Dependencies installed. Run: cargo run --release"
