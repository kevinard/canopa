#!/usr/bin/env bash
set -euo pipefail
# Use a private bus even when the caller has an existing graphical session.
# Package/test filters and libtest arguments are forwarded to cargo unchanged.
exec dbus-run-session -- cargo test --workspace --locked "$@"
