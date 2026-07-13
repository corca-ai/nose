# 0.19.0 release evidence

Generated on 2026-07-13 for the `nose 0.19.0` release candidate.

The machine-readable [release evidence artifact](../bench/recall_loss/release-0.19.0-evidence-2026-07-13.v1.json)
records the exact commands, artifact hashes, and measurements.

## Summary

- The baseline is the published `v0.18.0` Darwin arm64 binary, verified against
  its release checksum, rather than a local rebuild of the old source.
- The final `nose 0.19.0` binary's established semantic-only query surface
  measured `28,389.31ms -> 28,964.56ms` (`+2.03%`) across all `120` pinned
  repositories, below the `5%` release threshold without adjustment.
- The expanded default surface measured `34,322.88ms -> 36,159.03ms` (`+5.35%`)
  while returning `92,229 -> 100,966` families (`+9.47%`). Every repository
  gained families and none lost them, so this is retained as the measured price
  of deliberate capability growth rather than a same-output regression.
- Profiling found and removed a separate ranking defect introduced by accepted-pair
  endpoint coverage. On the focused Alamofire/Raylib slice, indexing collapsed
  sites by file reduced runtime `6,787.93ms -> 3,591.61ms` (`-47.09%`) with
  identical output; Alamofire `rank_map` fell from `3,203.7ms` to `22.0ms`.
- The tag smoke exposed a second, front-end-only Linux outlier. Replacing the
  per-file 256 KiB byte-window scan used to recognize ANSI-highlight artifacts
  with an escape-byte prefilter preserved exact output. On the focused
  Fastlane/Prettier control-paired slice, Prettier `lower` improved by an adjusted
  `26.6ms` (about `8.0%`) and `parse+lower` by `24.55ms` (about `13.5%`); the
  aggregate adjusted difference was a neutral `+2.21ms`.
- The clean candidate exactly reproduced the checked v6 product-quality result:
  worthy recall is `95.33%` on dev and `95.89%` on held-out, with labeled P@10
  of `59.46%` and `55.91%` respectively.
- The `crates` recall-loss gate passed with `0` false merges and `0`
  canon-preservation violations.

## Performance diagnosis

The first release-candidate run was `8.08%` slower than the official 0.18.0
binary. Most of the apparent delta tracked the larger result surface, but
Alamofire was a clear outlier: its `rank_map` stage alone took roughly `3.2`
seconds in both the candidate run and an immediate same-binary control.

The cause was an avoidable cross-file scan in accepted coverage reconstruction.
For each family member, reporting scanned every collapsed accepted site and only
then filtered by file. The retained patch builds a same-file index once and keeps
the exact collapse, overlap, deduplication, and edge semantics. A behavior-level
test covers overlapping sites in one file plus a linked site in another file.

The final candidate's all-repository same-binary control measured
`37,979.24ms -> 38,092.01ms` (`+0.30%`) with `120/120` identical hashes,
family counts, and byte counts. The remaining default-surface time is concentrated
in intended scoring and front-end work, while aggregate `rank_map` contributes
only `24.0ms` more than 0.18.0 across the entire corpus.

## Release comparison

The official-release comparison uses the final versioned `nose 0.19.0` binary:

| Surface | Families | Runtime | Raw delta | Approx. control-adjusted |
| --- | ---: | ---: | ---: | ---: |
| established semantic | `14,884 -> 15,815` | `28,389.31ms -> 28,964.56ms` | `+2.03%` | `+1.73%` |
| expanded default | `92,229 -> 100,966` | `34,322.88ms -> 36,159.03ms` | `+5.35%` | `+5.05%` |

The approximate control adjustment directionally subtracts the `+0.30%`
same-binary default-surface result; it is context, not a claim of deterministic
timing. The semantic surface remains inside the release gate under either view.
The default surface is `0.05` percentage points above the adjusted threshold but
returns `8,737` additional families (`+9.47%`) across all `120` repos; this is
explicitly accepted capability-growth cost. Both confirmed implementation hot
paths were fixed before release.

## Product quality

The release pass reran the split-safe v6 label evaluation from a clean tree. It
did not inspect held-out source.

| Split | Worthy recall | Labeled P@10 | Top-10 label coverage |
| --- | ---: | ---: | ---: |
| dev | `2716/2849 = 95.33%` | `264/444 = 59.46%` | `444/660 = 67.27%` |
| held-out | `2005/2091 = 95.89%` | `213/381 = 55.91%` | `381/540 = 70.56%` |

These values exactly match the checked #832 closeout, so the performance patch
did not change the evaluated product-quality result.

## Recall-loss gate

The release candidate passed:

```sh
target/release-0.19.0/frontend-perf-target/release/nose verify crates \
  --max-violations 0 \
  --recall-loss-report target/release-0.19.0/recall-loss.final.crates.json
```

| Metric | Value |
| --- | ---: |
| total units | `7,835` |
| interpretable units | `1,122` |
| canon checked | `117` |
| false merges | `0` |
| canon-preservation violations | `0` |
| completeness | `63/180 = 35.00%` |

The six trace disagreements are advisory and do not enter the soundness gate.
