#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

SOCKET="${CANOPA_NOTIFY_SOCKET:-${XDG_RUNTIME_DIR:-/tmp}/canopa-notify/control.sock}"
DATABASE="${CANOPA_NOTIFY_DATABASE:-${XDG_RUNTIME_DIR:-/tmp}/canopa-notify/manual-test.db}"
BIN="${CANOPA_NOTIFY_BIN:-}"

usage() {
    cat <<EOF
Usage: $(basename "$0")

Starts canopa-notify in graphical service mode so normal notify-send calls go
through org.freedesktop.Notifications.

Environment:
  CANOPA_NOTIFY_BIN       Existing canopa-notify binary to run. Defaults to cargo run.
  CANOPA_NOTIFY_SOCKET    Control socket path. Default: $SOCKET
  CANOPA_NOTIFY_DATABASE  Test database path. Default: $DATABASE

The command stays in the foreground. Stop it with Ctrl-C.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
    echo "WAYLAND_DISPLAY is not set; graphical service mode needs a Wayland session." >&2
    exit 1
fi

mkdir -p "$(dirname "$SOCKET")" "$(dirname "$DATABASE")"

echo "Starting canopa-notify service mode"
echo "  socket:   $SOCKET"
echo "  database: $DATABASE"
echo
echo "In another terminal, run:"
echo "  scripts/manual/notify-send-scenarios.sh smoke"
echo "  scripts/manual/watch-notification-signals.sh"
echo

if [[ -n "$BIN" ]]; then
    exec "$BIN" daemon --service --socket "$SOCKET" --database "$DATABASE"
fi

cd "$ROOT_DIR"
exec bash scripts/cargo.sh run -p canopa-notify -- daemon --service --socket "$SOCKET" --database "$DATABASE"
