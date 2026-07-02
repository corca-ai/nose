# Runtime triage

Runtime triage turns a query-regression report into a reproducible performance decision:
which repos are expected capability cost, which are noisy, and which need a focused fix.
Use it before optimizing a slow repo by hand.

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
  --repo arrow \
  --repo minio \
  --iterations 5 \
  --warmups 1 \
  --output target/runtime-triage-arrow-minio.json
```

Run the built-in parser/classifier self-test after changing the script:

```sh
python3 scripts/runtime-triage-harness.py --self-test
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
2. Run `scripts/runtime-triage-harness.py` on the largest regressions.
3. Optimize only `no-family-growth-*` cases with clear stage attribution.
4. For `capability-growth`, report cost per newly surfaced family before changing code.
5. Record the artifact under `bench/recall_loss/` or `target/` and link the summary doc.

The [0.17.0 post-release runtime triage](runtime-triage-0.17.0.md) is the first
documented use of this process.
