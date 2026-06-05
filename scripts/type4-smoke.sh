#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${OUT_DIR:-/tmp/nose-type4-smoke}"
CROSS="${CROSS:-ring}"
NOSE="${NOSE:-target/release/nose}"
BASELINE_JSON="${BASELINE_JSON:-}"
SUITE="${SUITE:-full}"

python3 bench/type4/generate.py --out-dir "$OUT_DIR" --cross "$CROSS"

MANIFEST="$OUT_DIR/manifest.json"
EVAL_DIR="$OUT_DIR"
if [[ "$SUITE" != "full" ]]; then
  EVAL_DIR="${COMPACT_DIR:-$OUT_DIR-$SUITE}"
  python3 bench/type4/select_cases.py "$MANIFEST" --suite "$SUITE" --out-dir "$EVAL_DIR"
  MANIFEST="$EVAL_DIR/manifest.json"
fi

python3 bench/type4/eval_manifest.py "$MANIFEST" --nose "$NOSE" --fail-on-false-merge

# Independent soundness gate: the interpreter oracle checks EVERY fingerprint-equal pair on
# an input battery (eval_manifest only checks the generator's labeled pairs), so it catches a
# coincidental cross-case false merge the manifest cannot. The residual on the synthetic corpus
# is oracle-fidelity artifacts (e.g. C `1`/`0` ≡ bool; see experiments §A2), so the gate is a
# BUDGET, not zero: set VERIFY_MAX to the characterized baseline and a new real false merge from
# a future canon pushes the count over it. Unset ⇒ informational (reports, never fails).
verify_args=("$EVAL_DIR/sources" --leads "$EVAL_DIR/leads.json")
if [[ -n "${VERIFY_MAX:-}" ]]; then
  verify_args+=(--max-violations "$VERIFY_MAX")
fi
# The oracle also EXPORTS its under-merged groups (behavior-equal, fingerprint-split) to
# leads.json — oracle-discovered missed-convergence candidates that feed the next generator
# round (the third actor closing the loop; vj ≥ 0.7 leads are the strongest).
"$NOSE" verify "${verify_args[@]}"

"$NOSE" stats "$EVAL_DIR/sources"

frontier_args=(
  "$MANIFEST"
  --nose "$NOSE"
  --json-out "$EVAL_DIR/frontier.json"
)

if [[ -n "$BASELINE_JSON" ]]; then
  frontier_args+=(
    --compare-to "$BASELINE_JSON"
    --compare-out "$EVAL_DIR/frontier-compare.json"
    --fail-on-regression
  )
fi

python3 bench/type4/frontier.py "${frontier_args[@]}"
