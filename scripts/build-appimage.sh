#!/usr/bin/env bash
# Build a portable AppImage for GitForge.
# Requires: cargo build already done, and linuxdeploy + appimagetool on PATH
#           (or they will be downloaded automatically).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="dev.gitforge.GitForge"
BINARY="${ROOT}/target/release/gitforge"
APPDIR="${ROOT}/target/appimage/${APP_ID}.AppDir"
VERSION="${1:-$(cargo metadata --manifest-path "${ROOT}/Cargo.toml" --format-version 1 2>/dev/null | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4 || echo "0.1.0")}"

if [[ ! -f "${BINARY}" ]]; then
  echo "error: build gitforge first (cargo build -p gitforge-app --release)" >&2
  exit 1
fi

echo "Building AppImage version=${VERSION}..."

rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin" "${APPDIR}/usr/share/applications" "${APPDIR}/usr/share/icons/hicolor"

install -Dm755 "${BINARY}" "${APPDIR}/usr/bin/gitforge"

install -Dm644 "${ROOT}/packaging/${APP_ID}.desktop" "${APPDIR}/${APP_ID}.desktop"

if [[ -d "${ROOT}/assets/icons/hicolor" ]]; then
  cp -a "${ROOT}/assets/icons/hicolor/." "${APPDIR}/usr/share/icons/hicolor/"
fi

mkdir -p "${APPDIR}/usr/share/gitforge/themes" "${APPDIR}/usr/share/gitforge/icons"
if [[ -d "${ROOT}/assets/themes" ]]; then
  cp -a "${ROOT}/assets/themes/." "${APPDIR}/usr/share/gitforge/themes/"
fi
if [[ -d "${ROOT}/assets/icons" ]]; then
  for f in "${ROOT}/assets/icons/"*.svg; do
    [[ -f "$f" ]] && install -Dm644 "$f" "${APPDIR}/usr/share/gitforge/icons/"
  done
fi

cat > "${APPDIR}/AppRun" << 'APPRUN'
#!/usr/bin/env bash
APPDIR="$(dirname "$(readlink -f "$0")")"
export PATH="${APPDIR}/usr/bin:${PATH}"
exec "${APPDIR}/usr/bin/gitforge" "$@"
APPRUN
chmod +x "${APPDIR}/AppRun"

ARCH=$(uname -m)
LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${ARCH}.AppImage"
APPIMAGETOOL_URL="https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-${ARCH}.AppImage"

TMPDIR="${ROOT}/target/appimage/tools"
mkdir -p "${TMPDIR}"

if ! command -v linuxdeploy >/dev/null 2>&1; then
  if [[ ! -f "${TMPDIR}/linuxdeploy" ]]; then
    echo "Downloading linuxdeploy..."
    curl -fSL -o "${TMPDIR}/linuxdeploy" "${LINUXDEPLOY_URL}"
    chmod +x "${TMPDIR}/linuxdeploy"
  fi
  export PATH="${TMPDIR}:${PATH}"
fi

if ! command -v appimagetool >/dev/null 2>&1; then
  if [[ ! -f "${TMPDIR}/appimagetool" ]]; then
    echo "Downloading appimagetool..."
    curl -fSL -o "${TMPDIR}/appimagetool" "${APPIMAGETOOL_URL}"
    chmod +x "${TMPDIR}/appimagetool"
  fi
  export PATH="${TMPDIR}:${PATH}"
fi

OUTPUT="${ROOT}/GitForge-${VERSION}-${ARCH}.AppImage"

echo "Packaging AppImage..."
cd "$(dirname "${APPDIR}")"
appimagetool "${APPDIR}" "${OUTPUT}"

echo "AppImage created: ${OUTPUT}"
