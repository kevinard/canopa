#!/usr/bin/env bash
set -euo pipefail
# Compatibility entry point; Canopa owns coordinated release packaging.
exec bash "$(dirname -- "${BASH_SOURCE[0]}")/../../scripts/smoke-release.sh" "$@"
