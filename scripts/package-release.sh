#!/usr/bin/env bash
set -euo pipefail
build_dir=$(realpath -- "${1:?usage: package-release.sh BUILD_DIR OUTPUT_DIR}")
mkdir -p -- "${2:?usage: package-release.sh BUILD_DIR OUTPUT_DIR}"
output_dir=$(realpath -- "$2")
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
bundle=canopa-x86_64-unknown-linux-gnu
dest="$stage/$bundle"
mkdir -p "$dest/bin" "$dest/share/canopa" "$dest/share/canopa-shell" \
    "$dest/share/canopa-notify" "$dest/share/systemd/user" "$dest/share/man/man1" \
    "$dest/share/dbus-1/services" "$dest/share/xdg-desktop-portal/portals"
install -m755 "$build_dir/canopa-shell" "$build_dir/canopa-wifi-provider" \
    "$build_dir/canopa-notify" "$build_dir/canopa" \
    "$build_dir/xdg-desktop-portal-canopa" "$dest/bin/"
install -m644 packaging/portal/*.portal "$dest/share/xdg-desktop-portal/portals/"
install -m644 packaging/portal/*.service "$dest/share/dbus-1/services/"
install -m644 packaging/systemd/xdg-desktop-portal-canopa.service "$dest/share/systemd/user/"
install -m644 docs/settings-portal.md docs/shell-rename.md "$dest/share/canopa/"
install -m644 docs/notifications.md docs/configuration.md config/canopa.kdl README.md LICENSES.md "$dest/share/canopa/"
install -m644 crates/canopa-shell/README.md crates/canopa-shell/LICENSE crates/canopa-shell/config/default.kdl "$dest/share/canopa-shell/"
install -m644 crates/canopa-notify/README.md crates/canopa-notify/LICENSE-MIT crates/canopa-notify/LICENSE-APACHE \
    crates/canopa-notify/examples/config.kdl "$dest/share/canopa-notify/"
install -m644 packaging/systemd/canopa-session.target packaging/systemd/canopa-shell.service packaging/systemd/canopa-notify.service "$dest/share/systemd/user/"
install -m644 packaging/canopa-notify.1 "$dest/share/man/man1/"
python3 scripts/dependency-attributions.py \
    --output "$dest/share/canopa/dependency-attributions.md" \
    --notices "$dest/share/licenses/dependencies"
"$dest/bin/canopa-shell" --version
"$dest/bin/canopa-notify" --help > /dev/null
tar -C "$stage" -czf "$output_dir/$bundle.tar.gz" "$bundle"
