#!/usr/bin/env bash
set -euo pipefail
# One crate now supplies both applications. An optional old host path is accepted
# only for the host within this workspace; independent snapshots are no longer used.
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
if [[ $# -gt 1 || ( $# -eq 1 && "$(realpath -- "$1")" != "$PWD/crates/niribar" ) ]]; then
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

# Inspect normal/build edges only: Tokio may be used by the codec test harness,
# but a default provider dependency must not select a runtime or desktop stack.
protocol_tree=$(cargo tree --locked -p canopa-panel-protocol --no-default-features --edges normal,build --prefix none --format '{p}')
if printf '%s\n' "$protocol_tree" | rg '^(niribar|canopa-notify|tokio|iced|wayland|accesskit|wgpu|winit)([-_ ]|$)'; then
    echo 'Default panel protocol unexpectedly depends on an application or runtime.' >&2
    exit 1
fi
echo 'Default panel protocol is independent of applications, desktop and runtime.'
