#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<EOF
Usage: $(basename "$0")

Watches sender-visible freedesktop notification signals while you interact with
real banners. Run this beside:

  scripts/manual/notify-send-scenarios.sh actions

Expected click behavior:
  1. ActivationToken is emitted first when a token is available.
  2. ActionInvoked follows with the same notification id.
  3. NotificationClosed is emitted when the notification is closed.

This script prefers dbus-monitor because it can filter the session bus directly.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

if command -v dbus-monitor >/dev/null 2>&1; then
    exec dbus-monitor --session \
        "type='signal',interface='org.freedesktop.Notifications'"
fi

if command -v busctl >/dev/null 2>&1; then
    exec busctl --user monitor org.freedesktop.Notifications
fi

if command -v gdbus >/dev/null 2>&1; then
    exec gdbus monitor \
        --session \
        --dest org.freedesktop.Notifications \
        --object-path /org/freedesktop/Notifications
fi

echo "Missing dbus-monitor, busctl, or gdbus; cannot watch D-Bus signals." >&2
exit 1
