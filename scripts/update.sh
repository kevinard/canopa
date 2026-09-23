#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

fail() {
    printf 'Update failed: %s\n' "$*" >&2
    exit 1
}

load_state=$(systemctl --user show niribar.service --property=LoadState --value)
[[ "$load_state" == loaded ]] || fail 'niribar.service is not loaded.'

# Cargo can reuse local-package fingerprints from another checkout sharing this
# target directory. Rebuild our package and path dependencies (see Cargo.toml),
# while retaining the much larger registry dependency cache and debug builds.
cargo clean --locked --release --target-dir target \
    --package niribar --package canopa-panel-protocol --package accesskit_unix \
    --package iced_layershell --package layershellev
cargo build --locked --release --bins --jobs 2 --target-dir target

bin_dir="$HOME/.local/bin"
mkdir -p "$bin_dir"
stage=$(mktemp -d "$bin_dir/.niribar-update.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT

install -m 755 target/release/niribar target/release/niribar-wifi-provider "$stage/"
"$stage/niribar" --version
# Rename staged files so a running executable is never overwritten in place.
mv -fT -- "$stage/niribar-wifi-provider" "$bin_dir/niribar-wifi-provider"
mv -fT -- "$stage/niribar" "$bin_dir/niribar"

service_state=$(systemctl --user show niribar.service --property=ActiveState --value)
case "$service_state" in
    active|activating|reloading|failed)
        # try-restart silently does nothing after a startup failure. Clear the
        # restart limit too, so an old binary rejecting new config can recover.
        if [[ "$service_state" == failed ]]; then
            systemctl --user reset-failed niribar.service
        fi
        systemctl --user restart niribar.service
        pid=$(systemctl --user show niribar.service --property=MainPID --value)
        [[ "$pid" =~ ^[1-9][0-9]*$ ]] || fail 'niribar.service did not start; check journalctl --user -u niribar.service.'
        # Type=simple reports success before the application finishes startup.
        # Require the same process to survive and run the executable we installed.
        sleep 2
        systemctl --user is-active --quiet niribar.service \
            || fail 'niribar.service failed during startup; check journalctl --user -u niribar.service.'
        current_pid=$(systemctl --user show niribar.service --property=MainPID --value)
        [[ "$current_pid" == "$pid" ]] \
            || fail 'niribar.service restarted during startup; check journalctl --user -u niribar.service.'
        [[ "/proc/$pid/exe" -ef "$bin_dir/niribar" ]] \
            || fail "niribar.service is running a different executable; set ExecStart to $bin_dir/niribar."
        printf 'niribar.service is running the installed executable (PID %s).\n' "$pid"
        ;;
    inactive)
        printf 'niribar.service is inactive; leaving it stopped.\n'
        ;;
    *)
        fail "niribar.service is $service_state; retry when it has settled."
        ;;
esac
printf 'Niribar installed in %s. SHA-256:\n' "$bin_dir"
sha256sum "$bin_dir/niribar"
