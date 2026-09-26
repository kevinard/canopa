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
mkdir -p "$dest/bin" "$dest/share/canopa" "$dest/share/niribar" \
    "$dest/share/canopa-notify" "$dest/share/systemd/user" "$dest/share/man/man1"
install -m755 "$build_dir/niribar" "$build_dir/niribar-wifi-provider" \
    "$build_dir/canopa-notify" "$build_dir/canopa" "$dest/bin/"
install -m644 docs/configuration.md config/canopa.kdl README.md LICENSES.md "$dest/share/canopa/"
install -m644 niribar/README.md niribar/LICENSE niribar/config/default.kdl "$dest/share/niribar/"
install -m644 notify/README.md notify/LICENSE-MIT notify/LICENSE-APACHE \
    notify/examples/config.kdl "$dest/share/canopa-notify/"
install -m644 niribar/contrib/systemd/niribar.service notify/packaging/systemd/canopa-notify.service "$dest/share/systemd/user/"
install -m644 notify/packaging/canopa-notify.1 "$dest/share/man/man1/"
python3 scripts/dependency-attributions.py \
    --output "$dest/share/canopa/dependency-attributions.md" \
    --notices "$dest/share/licenses/dependencies"
"$dest/bin/niribar" --version
"$dest/bin/canopa-notify" --help > /dev/null
tar -C "$stage" -czf "$output_dir/$bundle.tar.gz" "$bundle"
