#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

usage() {
    cat <<'EOF'
Usage: bash scripts/check-files.sh [copy | --lib [FILTER] | --test CASE ... [FILTER] [-- TEST_ARGS...]]

Default (copy): focused fingerprint unit tests and the uutils_copy integration target.
Select --lib or named --test cases for other Files work. Examples:
  bash scripts/check.sh --files --lib operations::fingerprint::receipt_tests
  bash scripts/check.sh --files --test transfers --test undo_changed_copy
  bash scripts/check.sh --files --test uutils_copy an_old_sha

Runs formatting and selected tests on a private D-Bus session. Uses the normal
debug profile, default Files features and target/ cache with incremental builds.
Ordinary cases share the integration executable and are selected by module filter.
Use --test integration to run all ordinary integration cases.
Use scripts/check.sh --workspace only when broader validation is needed.
EOF
}

selection=()
cases=()
case_filter=
test_args=()
case "${1:-copy}" in
    copy)
        if [[ $# -gt 1 ]]; then usage >&2; exit 2; fi
        ;;
    --help|-h)
        usage
        exit 0
        ;;
    --lib|--test)
        # Keep broad Cargo/profile/feature switches out of focused checks.
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --lib)
                    if [[ ${#selection[@]} -ne 0 || ${#cases[@]} -ne 0 ]]; then usage >&2; exit 2; fi
                    selection+=(--lib)
                    shift
                    ;;
                --test)
                    if [[ $# -lt 2 || ! "$2" =~ ^[a-zA-Z0-9_-]+$ || "${selection[0]:-}" == --lib ]]; then
                        usage >&2; exit 2
                    fi
                    cases+=("$2")
                    shift 2
                    ;;
                *) break ;;
            esac
        done
        if [[ $# -gt 0 && "$1" != -- ]]; then
            if [[ "$1" == -* ]]; then usage >&2; exit 2; fi
            if [[ ${#cases[@]} -gt 0 ]]; then
                case_filter=$1
            else
                selection+=("$1")
            fi
            shift
        fi
        if [[ $# -gt 0 ]]; then
            if [[ "$1" != -- ]]; then usage >&2; exit 2; fi
            shift
            test_args=("$@")
        fi
        ;;
    *) usage >&2; exit 2 ;;
esac

suite_filters=()
suite_cases=()
whole_suite=false
standalone=()
for name in "${cases[@]}"; do
    case "$name" in
        device_runtime|file_clipboard_wayland|local_actions|local_operation_attention|uutils_copy)
            if [[ " ${standalone[*]} " != *" $name "* ]]; then standalone+=("$name"); fi
            ;;
        integration) suite_filters+=(""); whole_suite=true ;;
        *)
            if ! rg -q "^[[:space:]]*mod $name;\$" crates/canopa-files/tests/integration.rs; then
                echo "Unknown Files test case: $name" >&2
                exit 2
            fi
            suite_filters+=("cases::$name::")
            suite_cases+=("$name")
            ;;
    esac
done

run_integration() {
    exact=false
    for argument in "${test_args[@]}"; do
        if [[ "$argument" == --exact ]]; then exact=true; fi
    done
    if [[ -z "$case_filter" ]] && ! "$exact"; then
        dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --test integration -- "${suite_filters[@]}" "${test_args[@]}"
        return
    fi
    # libtest combines filters with OR. Select actual names first to preserve
    # case membership AND substring matching inside the original case's names.
    listing=$(dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --test integration -- --list --format=terse)
    selection_flags=()
    if "$whole_suite"; then selection_flags+=(--all-cases); fi
    if "$exact"; then selection_flags+=(--exact); fi
    selected=$(printf '%s\n' "$listing" | python3 scripts/select-files-tests.py --filter "$case_filter" "${selection_flags[@]}" -- "${suite_cases[@]}")
    if [[ -z "$selected" ]]; then
        printf 'No ordinary Files tests match the selected cases and filter %s.\n' "$case_filter"
        return
    fi
    mapfile -t names <<< "$selected"
    exact_argument=()
    if ! "$exact"; then exact_argument+=(--exact); fi
    dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --test integration -- "${names[@]}" "${exact_argument[@]}" "${test_args[@]}"
}

# Override stale measurement settings inherited from a terminal or agent wrapper.
export CARGO_INCREMENTAL=1
bash scripts/cargo.sh fmt --all -- --check
if [[ ${#selection[@]} -eq 0 && ${#cases[@]} -eq 0 ]]; then
    dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --lib operations::fingerprint::receipt_tests
    dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --test uutils_copy
elif [[ ${#selection[@]} -gt 0 ]]; then
    dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files "${selection[@]}" -- "${test_args[@]}"
else
    if [[ ${#suite_filters[@]} -gt 0 ]]; then
        run_integration
    fi
    for name in "${standalone[@]}"; do
        filter=()
        if [[ -n "$case_filter" ]]; then filter+=("$case_filter"); fi
        dbus-run-session -- bash scripts/cargo.sh test --locked -p canopa-files --test "$name" "${filter[@]}" -- "${test_args[@]}"
    done
fi
