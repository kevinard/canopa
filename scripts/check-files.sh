#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

usage() {
    cat <<'EOF'
Usage: bash scripts/check-files.sh [copy | --lib [FILTER] | --test TARGET ... [FILTER] [-- TEST_ARGS...]]

Default (copy): focused fingerprint unit tests and the uutils_copy integration target.
Select --lib or named --test targets for other Files work. Examples:
  bash scripts/check.sh --files --lib operations::fingerprint::receipt_tests
  bash scripts/check.sh --files --test transfers --test undo_changed_copy
  bash scripts/check.sh --files --test uutils_copy an_old_sha

Runs formatting and selected tests on a private D-Bus session. Uses the normal
debug profile, default Files features and target/ cache with incremental builds.
Use scripts/check.sh --workspace only when broader validation is needed.
EOF
}

selection=()
case "${1:-copy}" in
    copy)
        if [[ $# -gt 1 ]]; then usage >&2; exit 2; fi
        ;;
    --help|-h)
        usage
        exit 0
        ;;
    --lib|--test)
        # Require target selection: a bare Cargo filter still builds every test
        # executable. Keep broad Cargo/profile/feature switches out of this path.
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --lib)
                    if [[ ${#selection[@]} -ne 0 ]]; then usage >&2; exit 2; fi
                    selection+=(--lib)
                    shift
                    ;;
                --test)
                    if [[ $# -lt 2 || ! "$2" =~ ^[a-zA-Z0-9_-]+$ || "${selection[0]:-}" == --lib ]]; then
                        usage >&2; exit 2
                    fi
                    selection+=(--test "$2")
                    shift 2
                    ;;
                *) break ;;
            esac
        done
        if [[ $# -gt 0 && "$1" != -- ]]; then
            if [[ "$1" == -* ]]; then usage >&2; exit 2; fi
            selection+=("$1")
            shift
        fi
        if [[ $# -gt 0 ]]; then
            if [[ "$1" != -- ]]; then usage >&2; exit 2; fi
            selection+=("$@")
        fi
        ;;
    *) usage >&2; exit 2 ;;
esac

# Override stale measurement settings inherited from a terminal or agent wrapper.
export CARGO_INCREMENTAL=1
bash scripts/cargo.sh fmt --all -- --check
if [[ ${#selection[@]} -eq 0 ]]; then
    dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --lib operations::fingerprint::receipt_tests
    dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --test uutils_copy
else
    dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files "${selection[@]}"
fi
