#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

case "${1:-}" in
    --core)
        bash notify/scripts/check-core-dependencies.sh
        cargo check --locked -p canopa-notify -p canopa-panel-protocol --all-targets --no-default-features
        bash notify/scripts/test.sh --no-default-features
        ;;
    '')
        python3 scripts/check-vendor.py
        bash notify/scripts/check-panel-protocol.sh
        python3 niribar/scripts/check-contrast.py
        python3 niribar/scripts/test-update.py
        cargo fmt --all -- --check
        cargo clippy --workspace --all-targets --locked -- -D warnings
        dbus-run-session -- cargo test --workspace --all-targets --locked
        cargo test --locked -p iced_layershell --lib
        cargo test --locked -p layershellev --lib
        ;;
    *)
        echo 'Usage: bash scripts/check.sh [--core]' >&2
        exit 2
        ;;
esac
