#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
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

systemctl --user try-restart niribar.service
printf 'Niribar installed in %s.\n' "$bin_dir"
