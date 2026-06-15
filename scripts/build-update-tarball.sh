#!/usr/bin/env bash
# Build a tarball for in-app auto-update (Zed-style layout).
# Requires: cargo build already done (target/release/gitforge).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="dev.gitforge.GitForge"
BINARY="${ROOT}/target/release/gitforge"
STAGING="${ROOT}/target/update-tarball/gitforge.app"
VERSION="${1:-$(cargo metadata --manifest-path "${ROOT}/Cargo.toml" --format-version 1 2>/dev/null | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4 || echo "0.1.0")}"
ARCH="$(uname -m)"

if [[ ! -f "${BINARY}" ]]; then
  echo "error: build gitforge first (cargo build -p gitforge-app --release)" >&2
  exit 1
fi

echo "Building update tarball version=${VERSION} arch=${ARCH}..."

rm -rf "${STAGING}"
mkdir -p \
  "${STAGING}/usr/bin" \
  "${STAGING}/usr/share/applications" \
  "${STAGING}/usr/share/gitforge/themes" \
  "${STAGING}/usr/share/icons/hicolor"

install -Dm755 "${BINARY}" "${STAGING}/usr/bin/gitforge"
install -Dm644 "${ROOT}/packaging/${APP_ID}.desktop" "${STAGING}/usr/share/applications/${APP_ID}.desktop"

if [[ -d "${ROOT}/assets/icons/hicolor" ]]; then
  cp -a "${ROOT}/assets/icons/hicolor/." "${STAGING}/usr/share/icons/hicolor/"
fi

if [[ -d "${ROOT}/assets/themes" ]]; then
  cp -a "${ROOT}/assets/themes/." "${STAGING}/usr/share/gitforge/themes/"
fi

OUTPUT="${ROOT}/GitForge-${VERSION}-${ARCH}.tar.gz"
tar -C "$(dirname "${STAGING}")" -czf "${OUTPUT}" "$(basename "${STAGING}")"

cd "$(dirname "${OUTPUT}")"
sha256sum "$(basename "${OUTPUT}")" > "${OUTPUT}.sha256"

echo "Update tarball created: ${OUTPUT}"
echo "Checksum: ${OUTPUT}.sha256"
