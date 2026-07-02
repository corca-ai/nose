# 0.17.0 post-release runtime triage

Generated on 2026-07-02 after the `nose 0.17.0` release evidence review.

For the reusable process and current harness, see [runtime triage](runtime-triage.md).
See the machine-readable [runtime-triage-0.17.0-post-release-2026-07-02.v1.json](../bench/recall_loss/runtime-triage-0.17.0-post-release-2026-07-02.v1.json) artifact for the focused measurements and repo classifications.
This page links back to the [0.17.0 release evidence](release-evidence-0.17.0.md),
which contains the all-120 release-candidate query regression run.
The follow-up [runtime-triage-arrow-minio-followup-2026-07-02.v1.json](../bench/recall_loss/runtime-triage-arrow-minio-followup-2026-07-02.v1.json) artifact records the subsequent `arrow` residual-cost and `minio` reclassification pass.

## Goal

The release-candidate all-corpus query regression was acceptable at the aggregate level
(`59,666.48ms -> 63,410.42ms`, `+6.27%` across 120 repos), but its largest repo-level
outliers still needed triage. The goal was to separate:

- expected cost from newly surfaced semantic families,
- small absolute or noisy regressions,
- real hot paths where runtime grew without a useful output-size explanation.

## Finding

The clearest real hot path was `arrow`: family count stayed `2 -> 2`, but
`arrow/arrow/locales.py` class value fingerprinting dominated `normalize+extract`.

Focused `NOSE_TIME=1 NOSE_TIME_UNIT_SUMMARY=1` evidence:

| Measurement | Before fix | After fix | Change |
| --- | ---: | ---: | ---: |
| `arrow` `normalize+extract` | `91.5ms` | `54.0ms` | `-41.0%` |
| top class unit total | `82.1ms` | `44.4ms` | `-45.9%` |
| top class unit value time | `80.7ms` | `43.0ms` | `-46.7%` |

The JSON output hash stayed identical:
`7ce920beb6bfa9161cd347d959feab2b83a689330f7e6a6f7cddd7e79c563e11`.

## Fix

`container_binding_value` now checks binding-domain evidence before asking whether the
module binding is mutated. This preserves the semantic policy: container binding values
were only admissible when binding-domain evidence existed before, and they still are.
The change only avoids repeated file-level mutation scans for assignments that cannot be
admitted anyway.

The attempted broader shared binding-domain proof cache was removed. It was safe, but
top-13 median timings did not justify the extra state.

## Top-13 Classification

The focused top-13 post-fix harness compared `v0.16.0` with the patched current binary
over 5 measured iterations and 1 warmup:

- aggregate focus-set runtime: `2764.9ms -> 3715.5ms` (`+34.4%`).
- `arrow`: real hot path, partially fixed. Remaining cost is class attribute value-graph
  construction, not the repeated mutation scan.
- `minio`: no family growth (`54 -> 54`) with a remaining Go hot path; this is the next
  highest-value performance investigation.
- `ripgrep`, `rustls`, `alacritty`, `regex`, `tokio`, `meilisearch`, `image`,
  `serde_json`: family count increased, so first classify cost per newly surfaced family
  before optimizing.
- `thor`: small absolute regression.
- `pry`, `fd`: not reproduced in the final focused harness.

## Next Work

Continue with the no-family-growth cases before broad micro-optimizing:

1. Investigate the remaining `arrow` data-like class value-graph cost without weakening
   class-data sensitivity.
2. Investigate the `minio` Go hot path.
3. For capability-growth repos, report runtime cost per additional surfaced family.

## Follow-Up Result

The follow-up did not leave a second code change. A prototype that shared the reachable
value-node set between value/literal fingerprint extraction and anchor extraction preserved
query JSON hashes, but its 5-run medians stayed within ordinary timing noise. It was removed.

Updated classification:

- `arrow`: residual cost is real but small. The representative hot unit is still
  `arrow/arrow/locales.py` class data (`85` class units seen, `79` kept, `12,723`
  tokens, `8,200` value atoms, representative value time `44.2ms`). Do not optimize
  this by weakening class-data sensitivity; a future fix needs a literal-table fast path
  that emits the same value/literal/anchor hashes.
- `minio`: not a single Go value-graph hot path. The representative top unit is
  `cmd/site-replication.go`, but the observed cost is split across lower/normalization
  passes and shared-line/render work. If this remains important, add lower-stage profiling
  before optimizing a specific Go unit.
