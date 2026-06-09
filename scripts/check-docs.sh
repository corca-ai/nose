#!/usr/bin/env bash
# Docs quality gate. The semantic-pack example check keeps checked-in v0
# manifests and fixture paths structurally honest; awiki checks the docs/ wiki is a single
# connected graph with no orphan pages or disconnected islands. Mirrors the
# `docs` job in .github/workflows/ci.yml.
#
#   ./scripts/check-docs.sh
#
# awiki is optional locally (this skips with a notice if absent); CI always runs
# it, so the gate is enforced there regardless. Install it with:
#   brew install corca-ai/tap/awiki
#   # or: go install github.com/corca-ai/awiki/cmd/awiki@latest
set -euo pipefail
cd "$(dirname "$0")/.."

if command -v python3 >/dev/null 2>&1; then
    python3 scripts/check-semantic-pack-examples.py
else
    echo "skipped semantic-pack example check — python3 not installed"
fi

if ! command -v awiki >/dev/null 2>&1; then
    echo "skipped — awiki not installed (brew install corca-ai/tap/awiki)"
    exit 0
fi

awiki lint --root docs
