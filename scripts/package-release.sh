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
mkdir -p "$dest/bin" "$dest/share/canopa" "$dest/share/canopa-shell" "$dest/share/canopa-files" \
    "$dest/share/canopa-notify" "$dest/share/systemd/user" "$dest/share/man/man1" \
    "$dest/share/dbus-1/services" "$dest/share/xdg-desktop-portal/portals" "$dest/share/applications"
install -m755 "$build_dir/canopa-shell" "$build_dir/canopa-wifi-provider" \
    "$build_dir/canopa-audio-provider" "$build_dir/canopa-media-provider" \
    "$build_dir/canopa-notify" "$build_dir/canopa-files" "$build_dir/canopa" \
    "$build_dir/xdg-desktop-portal-canopa" "$build_dir/canopa-wallpaper" "$build_dir/canopa-lock" "$build_dir/canopa-idle" "$build_dir/canopa-greeter" "$build_dir/canopa-polkit" "$build_dir/canopa-pinentry" "$dest/bin/"
install -m644 packaging/portal/*.portal "$dest/share/xdg-desktop-portal/portals/"
install -m644 packaging/portal/*.service "$dest/share/dbus-1/services/"
install -m644 packaging/file-manager/org.freedesktop.FileManager1.service "$dest/share/dbus-1/services/"
install -m644 packaging/systemd/xdg-desktop-portal-canopa.service "$dest/share/systemd/user/"
install -m644 packaging/applications/org.canopa.Files.desktop "$dest/share/applications/"
mkdir -p "$dest/share/canopa/pam" "$dest/share/canopa/stasis"
install -m644 packaging/stasis/stasis.rune "$dest/share/canopa/stasis/"
mkdir -p "$dest/share/canopa/greetd" "$dest/share/wayland-sessions"
install -m644 packaging/greetd/* "$dest/share/canopa/greetd/"
install -m644 packaging/wayland-sessions/canopa.desktop "$dest/share/wayland-sessions/"
install -m644 packaging/pam/canopa-lock.* "$dest/share/canopa/pam/"
python3 scripts/package-documentation.py "$dest"
install -m644 packaging/systemd/canopa-session.target packaging/systemd/canopa-shell.service packaging/systemd/canopa-notify.service packaging/systemd/canopa-wallpaper.service packaging/systemd/canopa-lock.service packaging/systemd/canopa-polkit.service packaging/systemd/canopa-idle.service "$dest/share/systemd/user/"
install -m644 packaging/canopa-notify.1 "$dest/share/man/man1/"
python3 scripts/dependency-attributions.py \
    --output "$dest/share/canopa/dependency-attributions.md" \
    --notices "$dest/share/licenses/dependencies"
"$dest/bin/canopa-shell" --version
"$dest/bin/canopa-notify" --help > /dev/null
"$dest/bin/canopa-files" --help > /dev/null
"$dest/bin/canopa-files" --internal-check-copy
"$dest/bin/canopa-wallpaper" check-config --defaults > /dev/null
"$dest/bin/canopa-lock" --defaults check-config > /dev/null
"$dest/bin/canopa-greeter" --defaults check-config > /dev/null
"$dest/bin/canopa-pinentry" --defaults check-config > /dev/null
"$dest/bin/canopa-polkit" --defaults check-config > /dev/null
tar -C "$stage" -czf "$output_dir/$bundle.tar.gz" "$bundle"
