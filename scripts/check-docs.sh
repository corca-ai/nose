#!/usr/bin/env bash
# Docs quality gate. awiki checks the docs/ wiki is a single connected graph —
# no orphan pages, no disconnected islands — so every doc stays reachable and the
# wiki navigable. Mirrors the `docs` job in .github/workflows/ci.yml.
#
#   ./scripts/check-docs.sh
#
# awiki is optional locally (this skips with a notice if absent); CI always runs
# it, so the gate is enforced there regardless. Install it with:
#   brew install corca-ai/tap/awiki
#   # or: go install github.com/corca-ai/awiki/cmd/awiki@latest
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v awiki >/dev/null 2>&1; then
    echo "skipped — awiki not installed (brew install corca-ai/tap/awiki)"
    exit 0
fi

awiki lint --root docs
