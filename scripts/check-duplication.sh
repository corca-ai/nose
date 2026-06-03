#!/usr/bin/env bash
# Copy-paste gate — nose dogfooding itself (the project's own "jscpd").
#
# Fails when the number of *substantial* duplicate families on nose's own source
# (refactoring value >= MIN_VALUE) exceeds BUDGET. This is a ratchet: the current
# accepted families are all reviewed and recorded in docs/Dogfooding.md (e.g. the
# borrow-blocked `generic` node-copy). To accept a genuinely new one, either dedupe
# it or raise BUDGET in this file with a one-line justification in the PR.
#
# Runs with --no-contiguous: this gate is about *design-level* duplication (families
# worth extracting), not the contiguous copy-paste floor — which always surfaces the
# reviewed-and-accepted per-grammar frontend parallelism (see docs/Dogfooding.md).
set -euo pipefail

MIN_VALUE=40   # ignore small/incidental similarity; gate only on substantial families
BUDGET=4       # accepted substantial families today (see docs/Dogfooding.md)
BIN="${NOSE_BIN:-./target/release/nose}"
GATE_ARGS=(scan crates --exclude tests --no-contiguous --min-value "$MIN_VALUE")

if [ ! -x "$BIN" ]; then
    echo "error: nose binary not found at '$BIN' (build with: cargo build --release)" >&2
    exit 2
fi

header="$("$BIN" "${GATE_ARGS[@]}" --top 0 2>/dev/null | head -1)"
count="$(printf '%s' "$header" | grep -oE '^[0-9]+' || echo 0)"

echo "copy-paste gate: $count substantial duplicate families (value >= $MIN_VALUE), budget $BUDGET"

if [ "$count" -gt "$BUDGET" ]; then
    echo >&2
    echo "FAILED: $count > $BUDGET — new substantial duplication was introduced." >&2
    echo "Dedupe it, or (with justification) bump BUDGET in scripts/check-duplication.sh." >&2
    echo >&2
    "$BIN" "${GATE_ARGS[@]}"
    exit 1
fi

echo "OK"
