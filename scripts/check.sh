#!/usr/bin/env bash
# Backwards-compatible entrypoint for the full local CI mirror.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "$#" -eq 0 ]]; then
    set -- --full
fi

exec ./scripts/check-ci-local.sh "$@"
