#!/usr/bin/env bash
# Build a .deb package for GitForge using cargo-deb.
# Requires: cargo-deb (cargo install cargo-deb)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v cargo-deb >/dev/null 2>&1; then
  echo "Installing cargo-deb..."
  cargo install cargo-deb
fi

echo "Building .deb package..."
cargo deb -p gitforge-app --manifest-path "${ROOT}/Cargo.toml"

echo "Deb package:"
ls -lh "${ROOT}/target/debian/"*.deb 2>/dev/null || echo "No .deb found"
