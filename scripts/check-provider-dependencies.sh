#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
# Dev edges matter: provider transport fixtures must also remain headless.
tree=$(cargo tree -p canopa-shell --no-default-features --locked --edges normal,build,dev --prefix none --format '{p}')
if printf '%s\n' "$tree" | rg '^(canopa-assets|canopa-ui|image|iced|wayland|accesskit|wgpu|winit|resvg)([-_ ]|$)'; then
    echo 'Headless providers unexpectedly depend on host rendering.' >&2
    exit 1
fi
echo 'Provider dependency graph excludes host rendering, including test dependencies.'
