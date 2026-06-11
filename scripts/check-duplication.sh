#!/usr/bin/env bash
# Duplication gate — nose dogfooding itself.
#
# Fails when the number of *substantial* duplicate families on nose's own source
# (refactoring value >= MIN_VALUE) exceeds BUDGET. This is a ratchet: the current
# accepted families are all reviewed and recorded in docs/dogfooding.md (mostly
# intentional per-grammar frontend parallelism). To accept a genuinely new one,
# either dedupe it or raise BUDGET in this file with a one-line justification in the PR.
#
# Runs only the `near` channel: this gate is about *design-level* Type-3 duplication
# (families worth extracting), not the syntax copy-paste floor — which always surfaces
# the reviewed-and-accepted per-grammar frontend parallelism (see docs/dogfooding.md).
#
# DETERMINISM: the count is reproducible run-to-run AND across platforms — nose hashes with
# FxHash (no random seed) and ranks with IEEE correctly-rounded ops only (+ - * / sqrt), and the
# family dedup sorts by a TOTAL order (span, value, then min source location). So CI and a local
# run report the SAME number; a count change is a real detection change (new duplication or a
# grammar/parse difference), never platform jitter. If they ever disagree, suspect a stale binary
# or a tree-sitter grammar version skew — not nondeterminism.
set -euo pipefail

MIN_VALUE=40   # ignore small/incidental similarity; gate only on substantial families
# Re-baselined 6 → 20 in PR #82: that PR STRENGTHENS the `near` channel (value-fingerprint
# candidates + high-vj acceptance for impure code, and sub-DAG anchor pairing), so nose now
# detects 14 additional PRE-EXISTING near-duplicate families in its own source — the cross-grammar
# frontend helpers and the `proven_*` value-graph factories — not new code introduced here. They
# are dedup candidates (see docs/dogfooding.md); the gate stays a ratchet against NEW duplication
# on top of this stronger detector.
# 20 → 21: weight-grading the sub-DAG score (a larger shared computation now scores higher, up to
# 0.90) lifts one PRE-EXISTING partial-clone family in nose's own source past the substantial
# (value ≥ 40) line — finer ranking surfacing real debt, not new code. Still a dedup candidate.
# 21 → 22: receiver-method LibraryApi occurrence evidence makes the near channel admit one
# PRE-EXISTING param-domain/binding helper family; new occurrence-producer duplication was deduped.
# 22 → 23: adding the Java empty-collection constructor recognizer to the `strict_exact_safe_call`
# dispatch chain (one `if recognizer { return true }` line) lifts the PRE-EXISTING
# `strict_exact_safe_call` ↔ `strict_exact_in_membership_safe` similarity (a ~4-line incidental
# overlap between a recognizer dispatch and a membership checker, not extractable duplication) past
# the value ≥ 40 line — not new avoidable duplication. See docs/dogfooding.md.
# Re-baselined 23 -> 24 in the #210 campaign: stronger fingerprint fidelity (deref
# stores, loop-effect keying) made one PRE-EXISTING cross-crate near-family visible —
# the assignment-name counting loops in value_graph/context.rs::seed_module_value_bindings
# and module_imports.rs::collect_statement_exports (small, cross-crate; reviewed, kept).
# Re-baselined 24 -> 25 in the #283-A fix: the effect-free-reorder guard shifts a few
# self-source value-graph fingerprints, nudging one PRE-EXISTING large-span dispatch
# near-family (interp.rs / value_graph/eval.rs / control.rs, sharing ~12 of ~1082 lines)
# past the value >= 40 line — a spurious whole-function span, not new duplication.
BUDGET=25      # accepted substantial families today (see docs/dogfooding.md)
BIN="${NOSE_BIN:-./target/release/nose}"
GATE_ARGS=(scan crates --exclude tests --mode near --min-value "$MIN_VALUE")

if [ ! -x "$BIN" ]; then
    echo "error: nose binary not found at '$BIN' (build with: cargo build --release)" >&2
    exit 2
fi

count="$(
    "$BIN" "${GATE_ARGS[@]}" --top 0 2>/dev/null \
        | sed -nE 's/^([0-9]+) .*/\1/p' \
        | head -1
)"
count="${count:-0}"

echo "duplication gate: $count substantial near-duplicate families (value >= $MIN_VALUE), budget $BUDGET"

if [ "$count" -gt "$BUDGET" ]; then
    echo >&2
    echo "FAILED: $count > $BUDGET — new substantial duplication was introduced." >&2
    echo "Dedupe it, or (with justification) bump BUDGET in scripts/check-duplication.sh." >&2
    echo >&2
    "$BIN" "${GATE_ARGS[@]}"
    exit 1
fi

echo "OK"
