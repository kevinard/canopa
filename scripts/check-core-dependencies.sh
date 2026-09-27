#!/usr/bin/env bash
set -euo pipefail
# Include dev/build edges: a test helper must not silently restore desktop dependencies.
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
tree=$(cargo tree -p canopa-notify -p canopa-panel-protocol --no-default-features --locked --edges normal,build,dev --prefix none --format '{p}')
if printf '%s\n' "$tree" | rg '^(canopa-assets|image|iced|wayland|accesskit|rodio|cpal|alsa|wgpu|winit)([-_ ]|$)'; then
    echo 'Core profile unexpectedly depends on a desktop adapter.' >&2
    exit 1
fi
echo 'Core dependency graph excludes GUI and audio adapters.'
