#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

case "${1:-}" in
    --core)
        bash notify/scripts/check-core-dependencies.sh
        cargo check --locked -p canopa-notify -p canopa-panel-protocol -p canopa-config -p canopa-file -p canopa-cli -p canopa-notify-client -p canopa-portal --all-targets --no-default-features
        cargo test --locked -p canopa-config -p canopa-file -p canopa-cli -p canopa-theme -p canopa-notify-client -p canopa-portal
        bash notify/scripts/test.sh --no-default-features
        ;;
    '')
        python3 scripts/check-vendor.py
        bash notify/scripts/check-panel-protocol.sh
        python3 niribar/scripts/check-contrast.py
        python3 niribar/scripts/test-update.py
        python3 scripts/test-session-units.py
        cargo fmt --all -- --check
        cargo clippy --workspace --all-targets --locked -- -D warnings
        python3 scripts/dependency-attributions.py --output docs/dependency-attributions.md --check
        dbus-run-session -- cargo test --workspace --all-targets --locked
        python3 scripts/test-settings-portal.py --binary "${CARGO_TARGET_DIR:-target}/debug/xdg-desktop-portal-canopa"
        cargo build --locked -p canopa-cli
        python3 scripts/test-configuration.py
        python3 scripts/test-vendor.py
        ;;
    *)
        echo 'Usage: bash scripts/check.sh [--core]' >&2
        exit 2
        ;;
esac
