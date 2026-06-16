#!/usr/bin/env bash
# Exercise "Restart to update" locally without publishing a GitHub release.
#
# 1. Builds a release binary
# 2. Wraps it in a script that logs when GPUI relaunches the app
# 3. Launches GitForge with GITFORGE_DEV_SIMULATE_UPDATE_READY so the title bar
#    shows the restart button immediately
#
# Click "Restart to update" in the title bar, then run:
#   cat /tmp/gitforge-restart-test.log
# You should see a RELAUNCHED line with a timestamp.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${ROOT}/target/release/gitforge"
LOG="/tmp/gitforge-restart-test.log"
WRAPPER="$(mktemp /tmp/gitforge-restart-wrapper.XXXXXX)"

cleanup() {
  rm -f "${WRAPPER}"
}
trap cleanup EXIT

echo "Building release binary..."
cd "$ROOT"
cargo build -p gitforge-app --release --features dev-simulate-update

cat >"${WRAPPER}" <<EOF
#!/usr/bin/env bash
echo "RELAUNCHED \$(date -Is)" >> "${LOG}"
exec "${BINARY}" "\$@"
EOF
chmod +x "${WRAPPER}"

rm -f "${LOG}"
echo "Launching GitForge with a simulated pending update."
echo "Wrapper log: ${LOG}"
echo "Click the title bar button, then inspect the log."
echo

RUST_LOG="${RUST_LOG:-info}" \
  GITFORGE_DEV_SIMULATE_UPDATE_READY=1 \
  GITFORGE_DEV_RESTART_PATH="${WRAPPER}" \
  "${BINARY}"
