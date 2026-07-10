# Runtime triage

Runtime triage turns a query-regression report into a reproducible performance decision:
which repos are expected capability cost, which are noisy, and which need a focused fix.
Use it before optimizing a slow repo by hand.

## When Required

Run this process when a PR, release candidate, or post-release follow-up changes semantic
admission, lowering, normalization, query ranking/filtering, or corpus-scale analysis
behavior and the broad query-regression run reports a meaningful runtime increase.

Start with the PR-sized [semantic regression smoke](semantic-regression-smoke.md).
It owns the automatic 5%/5 ms base/head decision and focused rerun; this runbook is
the deeper classification step after that bounded gate identifies a material signal.

The broad gate answers whether product output or runtime moved. Run
`scripts/check-query-regression.py` on that artifact to make the no-behavior-change
decision mechanical: hashes, byte counts, and family counts must stay identical, while
runtime is compared against a configured percentage threshold after subtracting any
same-binary control. Runtime triage answers why a remaining measured slowdown moved.
Do not skip the classification step just because a repo looks slow: capability-growth
cost, measurement noise, lower/front-end cost, and value-graph hot paths call for
different actions.

## Harness

Use `scripts/runtime-triage-harness.py` when comparing two binaries on one or more corpus
repos. It runs `nose query <repo> all top=0 --mode semantic --format json` with
`NOSE_TIME=1` and `NOSE_TIME_UNIT_SUMMARY=1`, stores raw runs, aggregates median stage
timings, records representative top unit summaries, and classifies each repo.

Example:

```sh
python3 scripts/runtime-triage-harness.py \
  --baseline-binary /tmp/nose-v0.16.0-target/release/nose \
  --current-binary target/release/nose \
  --baseline-source-ref v0.16.0 \
  --current-source-ref HEAD \
  --all-repos \
  --iterations 5 \
  --warmups 1 \
  --output target/runtime-triage-all-repos.json
```

Run the built-in parser/classifier self-test after changing the script:

```sh
python3 scripts/runtime-triage-harness.py --self-test
```

For no-behavior-change work, run the query-regression checker on the broad artifact:

```sh
python3 scripts/check-query-regression.py \
  target/query-regression.json \
  --same-binary-control target/same-binary-control.json \
  --max-runtime-delta-pct 5
```

## Classifications

The harness writes `schema: nose.runtime_triage_harness.v1` and classifies each repo:

- `not-reproduced`: current median is not slower than baseline.
- `small-or-noisy`: runtime delta is below the configured percentage or absolute threshold.
- `capability-growth`: family count increased; first report runtime cost per additional
  surfaced family before optimizing.
- `no-family-growth-value-hot-path`: family count did not grow and representative unit
  value time dominates.
- `no-family-growth-lower-or-frontend`: family count did not grow and lower/front-end
  stage delta dominates.
- `no-family-growth-mixed-hot-path`: family count did not grow, but timing is split across
  stages and needs narrower instrumentation before a fix.

The default thresholds are intentionally conservative:

- `--regression-pct 20`
- `--small-absolute-ms 25`
- `--hot-unit-ms 20`

## Timing Knobs

Use the timing knobs in increasing order of verbosity:

- `NOSE_TIME=1`: top-level discover/lower/normalize/query stage timings.
- `NOSE_TIME_UNIT_SUMMARY=1`: per-file/per-unit-kind aggregate extraction timings.
- `NOSE_TIME_UNITS=1`: individual unit timings for units taking at least 10ms.
- `NOSE_TIME_NORMALIZE=1`: per-normalization-pass timings by file.
- `NOSE_TIME_VALUE_GRAPH=1`: per-value-graph build timings split into seed,
  immutable-binding seeding, inline-candidate setup, process, and finish phases.

`NOSE_TIME_VALUE_GRAPH=1` is intentionally verbose. Use it only after
`NOSE_TIME_UNIT_SUMMARY=1` has identified a repo/file/unit-kind worth inspecting.

## Process

1. Run `scripts/query-regression-harness.py` for broad product-level elapsed timing.
2. Run `scripts/check-query-regression.py` to fail any unexpected product-output drift.
3. Run `scripts/runtime-triage-harness.py` on the largest remaining runtime regressions.
4. Optimize only `no-family-growth-*` cases with clear stage attribution.
5. For `capability-growth`, report cost per newly surfaced family before changing code.
6. Record the artifact under `bench/recall_loss/` or `target/` and link the summary doc.

The triage is complete when every selected repo has a recorded classification, any
code change names the stage it is intended to improve, and the follow-up document
links both the broad query-regression artifact and the focused runtime-triage artifact.

The [0.17.0 post-release runtime triage](runtime-triage-0.17.0.md) is the first
documented use of this process.

The [20-optimization runtime pass](runtime-performance-20-optimizations-2026-07-02.md)
records the first longer optimization sequence using this process, including the
same-binary noise control, all-120-repo before/after artifact, and focused recheck of
the largest apparent regressions.
