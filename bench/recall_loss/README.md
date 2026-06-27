# Recall-loss baselines

This directory keeps compact, reproducible summaries for
`nose verify --recall-loss-report`. The full JSON reports are local artifacts;
the checked-in files record the command, selected surface, hard gate, reason
rollups, and representative fixtures needed to reproduce or review a semantic
kernel PR.

## Regenerate

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.crates.json

cargo run -q -p nose-cli -- verify \
  bench/repos/chi/middleware/content_type.go \
  bench/repos/boltons/boltons/iterutils.py \
  bench/repos/thor/lib/thor/actions.rb \
  bench/repos/radash/src/array.ts \
  bench/repos/hyperfine/src/util/number.rs \
  bench/repos/swift-metrics/Sources/CoreMetrics/Metrics.swift \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.corpus-slice.json
```

Compare two full reports with:

```sh
python3 scripts/recall-loss-diff.py before.json after.json
```

Build the #574 callee-identity census from a full report with:

```sh
python3 scripts/recall-loss-callee-census.py \
  target/recall-loss.crates.json \
  > bench/recall_loss/issue-574-callee-census.v1.json
```

## Files

- [crates.baseline.v1.json](crates.baseline.v1.json) records the current
  `crates` baseline and the #570 attribution improvement from the first coarse
  #569 baseline.
- [corpus-slice.baseline.v1.json](corpus-slice.baseline.v1.json) records a
  small mixed-language corpus slice across Go, Python, Ruby, TypeScript, Rust,
  and Swift.
- [issue-570-cycles.v1.json](issue-570-cycles.v1.json) records the five focused
  top-bucket cycles and the explicit unsupported/fail-closed boundary decision.
- [issue-572-cycle.v1.json](issue-572-cycle.v1.json) records the first
  post-#570 refinement cycle: expression-statement effect boundaries and Rust
  macro surfaces are split out of the callee-identity bucket while preserving
  the hard gate.
- [issue-574-callee-census.v1.json](issue-574-callee-census.v1.json) records
  the #567/#574 census of the remaining callee-identity bucket by language and
  call-target surface.
