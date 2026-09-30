#!/usr/bin/env bash
set -euo pipefail
# Use a private bus even when the caller has an existing graphical session.
# Keep core checks independent of Canopa Shell's GUI feature graph.
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
exec dbus-run-session -- cargo test -p canopa-notify -p canopa-panel-protocol --locked "$@"
