#!/usr/bin/env bash
# Install GitForge Freedesktop launcher and icons for the current user.
# Run once after building; re-run after icon or .desktop template changes.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="dev.gitforge.GitForge"
DESKTOP_SRC="${ROOT}/packaging/${APP_ID}.desktop"
ICONS_SRC="${ROOT}/assets/icons/hicolor"
DESKTOP_DIR="${HOME}/.local/share/applications"
ICONS_DIR="${HOME}/.local/share/icons"
HICOLOR_DEST="${ICONS_DIR}/hicolor"
DESKTOP_DEST="${DESKTOP_DIR}/${APP_ID}.desktop"

if [[ ! -f "${DESKTOP_SRC}" ]]; then
  echo "error: missing desktop template at ${DESKTOP_SRC}" >&2
  exit 1
fi

if [[ ! -d "${ICONS_SRC}" ]]; then
  echo "Generating icons..."
  "${ROOT}/scripts/generate-app-icons.sh"
fi

if [[ -x "${ROOT}/target/debug/gitforge" ]]; then
  EXEC_PATH="${ROOT}/target/debug/gitforge"
elif command -v gitforge >/dev/null 2>&1; then
  EXEC_PATH="$(command -v gitforge)"
else
  echo "error: build gitforge first (cargo build -p gitforge-app)" >&2
  exit 1
fi

mkdir -p "${DESKTOP_DIR}" "${HICOLOR_DEST}"
cp -a "${ICONS_SRC}/." "${HICOLOR_DEST}/"

tmp="$(mktemp)"
trap 'rm -f "${tmp}"' EXIT
sed "s|^Exec=.*|Exec=${EXEC_PATH}|" "${DESKTOP_SRC}" > "${tmp}"
install -m 644 "${tmp}" "${DESKTOP_DEST}"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${DESKTOP_DIR}" 2>/dev/null || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "${ICONS_DIR}/hicolor" 2>/dev/null || true
fi

# KDE caches icons and .desktop entries separately from GTK.
if command -v kbuildsycoca6 >/dev/null 2>&1; then
  kbuildsycoca6 --noincremental 2>/dev/null || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
  kbuildsycoca5 --noincremental 2>/dev/null || true
fi

echo "Installed:"
echo "  ${DESKTOP_DEST}"
echo "  icons -> ${HICOLOR_DEST}"
echo "  Exec=${EXEC_PATH}"
echo ""
case "${XDG_CURRENT_DESKTOP:-}" in
  *KDE*|*Plasma*)
    echo "KDE: open from KRunner (Meta) or the app menu, or run: gtk-launch ${APP_ID}"
    echo "If the taskbar icon looks stale, unpin GitForge and pin it again after reinstalling."
    ;;
  *)
    echo "Launch: gtk-launch ${APP_ID}  (or find GitForge in your app menu)"
    ;;
esac
