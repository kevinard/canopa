#!/usr/bin/env bash
set -euo pipefail
# Keep Cargo's current directory, arguments, environment and incremental cache.
exec python3 "$(dirname -- "${BASH_SOURCE[0]}")/build-guard.py" -- cargo "$@"
