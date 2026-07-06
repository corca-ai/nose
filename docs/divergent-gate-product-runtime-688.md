# Divergent Gate Product Runtime 688

Issue #688 records product-output and runtime evidence for the #681 divergent
gate productization work through #687. It is not the final epic closeout and
does not claim default-on readiness.

## Checked Artifacts

- Product/runtime summary: [issue-688-product-output-runtime-summary-2026-07-06.v1.json](../bench/divergent_history/issue-688-product-output-runtime-summary-2026-07-06.v1.json)
  is the checked index artifact.
- Non-`base=` all-repos query regression: [issue-688-nonbase-allrepos-query-regression-2026-07-06.v1.json](../bench/divergent_history/issue-688-nonbase-allrepos-query-regression-2026-07-06.v1.json)
  compares `551758d5` with `08052e90` across all 120 pinned corpus repos.
- Non-`base=` same-binary control: [issue-688-nonbase-same-binary-control-2026-07-06.v1.json](../bench/divergent_history/issue-688-nonbase-same-binary-control-2026-07-06.v1.json)
  prices ordinary wall-time noise on the same 120 repos.
- Focused nose slice: [issue-688-nonbase-nose-slice-query-regression-2026-07-06.v1.json](../bench/divergent_history/issue-688-nonbase-nose-slice-query-regression-2026-07-06.v1.json)
  covers `crates/nose-cli/src` and `crates/nose-cli/tests/cli`.
- Bounded replay summaries: [baseline](../bench/divergent_history/issue-688-replay-summary-baseline-551758d5-r14-p3-2026-07-06.v1.json)
  and [current](../bench/divergent_history/issue-688-replay-summary-current-08052e90-r14-p3-2026-07-06.v1.json)
  replay the divergent gate over the default 14-repo set with `per-repo 3`.

Raw replay JSONL remains under `/tmp/nose-688/` and is not checked in. The
checked summaries record raw sha256 values.

## Non-Base Product Output

Command shape:

```sh
nose query <repo> all top=0 --mode semantic --format json
```

All-repos result:

| measure | value |
|---|---:|
| repos | 120 |
| output hash drift | 0 |
| byte-count drift | 0 |
| family-count drift | 0 |
| aggregate median | `56301.32ms -> 56835.43ms` |
| aggregate delta | `+0.95%` |

One small repo, `faraday`, crossed the repo-level runtime threshold in the
baseline/current run (`51.59ms -> 85.78ms`). The same-binary control did not
reproduce it (`61.70ms -> 53.05ms`) and measured aggregate `-0.40%`, so no
runtime triage or optimization was opened.

Focused nose slice result:

| path | output | families | median |
|---|---|---:|---:|
| `crates/nose-cli/src` | identical | 2 | `80.95ms -> 80.97ms` |
| `crates/nose-cli/tests/cli` | identical | 0 | `43.26ms -> 44.40ms` |

## Base Replay Runtime

The bounded replay used the default `eval/divergence_fire/replay.py` repo set,
`per-repo 3`, default and near arms, and raw records outside git.

| arm | count result | p50 | p90 |
|---|---|---:|---:|
| default | identical counts, tiers, lanes, errors 0 | `3.79s -> 4.23s` | `8.91s -> 9.20s` |
| near | identical counts, tiers, lanes, errors 0 | `4.09s -> 3.86s` | `10.07s -> 8.61s` |

The largest positive p50/p90 movement was default p50 `+11.6%`, below the 20%
replay timing threshold. No runtime triage or optimization work was opened.

## Validation

The docs gate validates this packet through
`scripts/check-divergent-history-artifacts.py`. The checker verifies referenced
sha256 values, non-`base=` hash/byte/family stability, deterministic per-repo
outputs, replay errors, replay count stability, replay timing thresholds, and
absence of source-bearing summary keys.
