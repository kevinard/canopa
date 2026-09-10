#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 1 ]]; then
    echo 'Usage: bash scripts/check-panel-protocol.sh /path/to/niribar' >&2
    exit 2
fi
notify_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
niribar_root="$1"
for protocol_file in Cargo.toml src/lib.rs; do
    diff -u -- "$notify_root/crates/canopa-panel-protocol/$protocol_file" \
        "$niribar_root/vendor/canopa-panel-protocol/$protocol_file"
done
echo 'Canopa panel protocol source matches the Niribar snapshot.'
