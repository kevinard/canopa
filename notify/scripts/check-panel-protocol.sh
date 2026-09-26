#!/usr/bin/env bash
set -euo pipefail
# One crate now supplies both applications. An optional old host path is accepted
# only for the host within this workspace; independent snapshots are no longer used.
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
if [[ $# -gt 1 || ( $# -eq 1 && "$(realpath -- "$1")" != "$PWD/niribar" ) ]]; then
    echo 'Use the Niribar member of this Canopa workspace.' >&2
    exit 2
fi
cargo metadata --locked --offline --format-version 1 --no-deps | python3 -c '
import json, pathlib, sys
metadata = json.load(sys.stdin)
expected = pathlib.Path(metadata["workspace_root"]) / "crates/canopa-panel-protocol"
packages = {p["name"]: p for p in metadata["packages"]}
for name in ["niribar", "canopa-notify"]:
    dependencies = [d for d in packages[name]["dependencies"] if d["name"] == "canopa-panel-protocol"]
    assert len(dependencies) == 1 and pathlib.Path(dependencies[0]["path"]).resolve() == expected.resolve(), name
print("Both applications use the single workspace panel protocol crate.")
'
