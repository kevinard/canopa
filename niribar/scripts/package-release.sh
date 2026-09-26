#!/usr/bin/env bash
set -euo pipefail
build_dir=${1:?usage: package-release.sh BUILD_DIR OUTPUT_DIR}
output_dir=${2:?usage: package-release.sh BUILD_DIR OUTPUT_DIR}
bundle=niribar-x86_64-unknown-linux-gnu
mkdir -p "$output_dir"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/$bundle/bin" "$stage/$bundle/share/niribar" "$stage/$bundle/share/systemd/user"
install -m 755 "$build_dir/niribar" "$build_dir/niribar-wifi-provider" "$stage/$bundle/bin/"
install -m 644 config/default.kdl README.md LICENSE "$stage/$bundle/share/niribar/"
install -m 644 contrib/systemd/niribar.service "$stage/$bundle/share/systemd/user/"
tar -C "$stage" -czf "$output_dir/$bundle.tar.gz" "$bundle"
