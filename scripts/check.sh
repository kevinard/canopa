#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

usage() {
    cat <<'EOF'
Usage: bash scripts/check.sh [--files [copy | --lib [FILTER] | --test CASE ... [FILTER] [-- TEST_ARGS...]] | --workspace | --core]

Default: focused Files Copy/fingerprint checks with incremental debug builds.
--files       Select Files tests; see scripts/check-files.sh --help.
--workspace   Full workspace/tooling gate, including all targets.
--core        Broad core checks without desktop dependencies.
EOF
}

case "${1:---files}" in
    --files)
        if [[ $# -gt 0 ]]; then shift; fi
        exec bash scripts/check-files.sh "$@"
        ;;
    --help|-h)
        usage
        exit 0
        ;;
    --workspace|--core)
        if [[ $# -ne 1 ]]; then usage >&2; exit 2; fi
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

# Ordinary debug checks use Cargo's incremental cache; CI may explicitly opt out.
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-1}"

case "$1" in
    --core)
        bash scripts/check-core-dependencies.sh
        bash scripts/check-provider-dependencies.sh
        bash scripts/cargo.sh check --locked -p canopa-shell --no-default-features --all-targets
        dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-shell --no-default-features --all-targets
        bash scripts/cargo.sh check --locked -p canopa-notify -p canopa-panel-protocol -p canopa-config -p canopa-file -p canopa-files -p canopa-cli -p canopa-notify-client -p canopa-portal --all-targets --no-default-features
        bash scripts/cargo.sh test --locked -p canopa-config -p canopa-file -p canopa-cli -p canopa-theme -p canopa-notify-client -p canopa-portal
        bash scripts/cargo.sh test --locked -p canopa-files --no-default-features
        bash scripts/test-notify.sh --no-default-features
        ;;
    --workspace)
        python3 scripts/check-vendor.py
        bash scripts/check-panel-protocol.sh
        python3 scripts/check-contrast.py
        python3 scripts/run-desktop-acceptance.py --list
        python3 scripts/test-desktop-runner.py
        python3 scripts/test-update.py
        python3 scripts/test-session-units.py
        python3 scripts/test-portal-config.py
        python3 scripts/test-install-system.py
        python3 scripts/test-check-scripts.py
        python3 scripts/test-test-layout.py
        python3 scripts/test-niri-source-bindings.py
        python3 scripts/test-niri-build-reuse.py
        python3 scripts/test-vendor-source-additions.py
        python3 scripts/test-remote-touch-core.py --output "${CARGO_TARGET_DIR:-target}/remote-touch-core-check.json"
        python3 scripts/test-vendor-test-workspace.py
        python3 scripts/test-cups-authentication.py --self-test
        python3 scripts/test-build-guard.py
        bash scripts/cargo.sh fmt --all -- --check
        bash scripts/cargo.sh clippy --workspace --all-targets --locked -- -D warnings
        python3 scripts/dependency-attributions.py --output docs/dependency-attributions.md --check
        dbus-run-session -- bash scripts/cargo.sh test --workspace --all-targets --locked --no-fail-fast
        python3 scripts/test-settings-portal.py --binary "${CARGO_TARGET_DIR:-target}/debug/xdg-desktop-portal-canopa"
        python3 scripts/test-remote-desktop-dispatch.py --binary "${CARGO_TARGET_DIR:-target}/debug/xdg-desktop-portal-canopa" --output "${CARGO_TARGET_DIR:-target}/remote-desktop-dispatch-check.json"
        bash scripts/cargo.sh build --locked -p canopa-cli -p canopa-session -p canopa-greeter -p canopa-polkit -p canopa-pinentry -p canopa-settings
        python3 scripts/test-configuration.py
        python3 scripts/test-vendor.py
        ;;
esac
