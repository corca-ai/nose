#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${OUT_DIR:-/tmp/nose-type4-smoke}"
CROSS="${CROSS:-}"
NOSE="${NOSE:-target/release/nose}"
BASELINE_JSON="${BASELINE_JSON:-}"
SUITE="${SUITE:-full}"
GATE="${GATE:-}"
AXIS="${AXIS:-}"
PROPOSAL_PREFIX="${PROPOSAL_PREFIX:-}"

case "$GATE" in
  "")
    ;;
  focused)
    if [[ -z "$AXIS" && -z "$PROPOSAL_PREFIX" ]]; then
      echo "GATE=focused requires AXIS or PROPOSAL_PREFIX" >&2
      exit 2
    fi
    SUITE="${SUITE:-full}"
    CROSS="${CROSS:-none}"
    ;;
  core)
    SUITE="core"
    ;;
  full)
    SUITE="full"
    ;;
  *)
    echo "unknown GATE: $GATE (expected focused, core, full)" >&2
    exit 2
    ;;
esac
CROSS="${CROSS:-ring}"

generate_args=(bench/type4/generate.py --out-dir "$OUT_DIR" --cross "$CROSS")
if [[ -n "$AXIS" ]]; then
  generate_args+=(--axis "$AXIS")
fi
if [[ -n "$PROPOSAL_PREFIX" ]]; then
  generate_args+=(--proposal-prefix "$PROPOSAL_PREFIX")
fi

python3 "${generate_args[@]}"

MANIFEST="$OUT_DIR/manifest.json"
EVAL_DIR="$OUT_DIR"
if [[ "$SUITE" != "full" ]]; then
  EVAL_DIR="${COMPACT_DIR:-$OUT_DIR-$SUITE}"
  python3 bench/type4/select_cases.py "$MANIFEST" --suite "$SUITE" --out-dir "$EVAL_DIR"
  MANIFEST="$EVAL_DIR/manifest.json"
fi

python3 bench/type4/eval_manifest.py "$MANIFEST" --nose "$NOSE" --fail-on-false-merge
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
