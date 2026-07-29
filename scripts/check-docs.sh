#!/usr/bin/env bash
# Docs quality gate. The semantic-pack example check keeps checked-in v0
# and v1 manifests and fixture paths structurally honest. Set NOSE_BIN to run the
# product CLI conformance check from this script; CI and `check-ci-local --full`
# run that check after a fresh release build. awiki checks the docs/ wiki is a
# single connected graph with no orphan pages or disconnected islands.
#
#   ./scripts/check-docs.sh
#
# awiki is optional locally (this skips with a notice if absent); CI always runs
# it, so the gate is enforced there regardless. Install it with:
#   brew install corca-ai/tap/awiki
#   # or: go install github.com/corca-ai/awiki/cmd/awiki@latest
set -euo pipefail
cd "$(dirname "$0")/.."

semantic_pack_examples=(docs/examples/semantic-packs/v0 docs/examples/semantic-packs/v1)

if command -v python3 >/dev/null 2>&1; then
    python3 scripts/check-semantic-pack-examples.py
    python3 scripts/check-ci-examples.py
    python3 scripts/check-divergent-history-artifacts.py
    python3 bench/type4/semantic_pattern_cards.py --check
    python3 bench/type4/open_surface_admission_audit.py --selftest
    python3 bench/type4/open_surface_admission_audit.py --check
    python3 bench/type4/python_loop_demorgan_proof_facts.py --selftest
    python3 bench/type4/python_loop_demorgan_proof_facts.py --check
    python3 bench/type4/proof_carrying_frontier.py --selftest
    python3 bench/type4/proof_carrying_frontier.py --check
    bench/type4/adversarial/scripts/type4-check
else
    echo "skipped semantic-pack example check — python3 not installed"
fi

if [ -n "${NOSE_BIN:-}" ]; then
    if [ ! -x "${NOSE_BIN}" ]; then
        echo "NOSE_BIN is set but not executable: ${NOSE_BIN}" >&2
        exit 127
    fi
    "${NOSE_BIN}" semantic-pack check "${semantic_pack_examples[@]}" --format json >/dev/null
    "${NOSE_BIN}" semantic-pack status docs/examples/semantic-pack-lock-v1.json --format json >/dev/null
else
    echo "skipped semantic-pack CLI conformance — set NOSE_BIN or run ./scripts/check-ci-local.sh --full"
fi

if ! command -v awiki >/dev/null 2>&1; then
    echo "skipped — awiki not installed (brew install corca-ai/tap/awiki)"
    exit 0
fi

if awiki lint --help 2>&1 | grep -q -- '--root'; then
    awiki lint --root docs
else
    (cd docs && awiki lint)
fi
