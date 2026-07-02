# 0.17.0 release evidence

Generated on 2026-07-02 for the `nose 0.17.0` release candidate.

See the machine-readable [release-0.17.0-evidence-2026-07-02.v1.json](../bench/recall_loss/release-0.17.0-evidence-2026-07-02.v1.json) artifact for exact commands and measurements.

## Summary

- Hazard refresh stayed stable: `432,609` events, `14,784` families, G1 AUC `v5=0.695`,
  `v7=0.698`, logistic mean `0.662`; no hazard formula recalibration was needed.
- Product query regression compared `v0.16.0` with the 0.17.0 release candidate across
  all `120` pinned corpus repos with `3` measured iterations and `1` warmup.
- Aggregate query runtime moved `59,666.48ms -> 63,410.42ms` (`+6.27%`). The run
  recorded expected JSON hash drift in all repos and family-count drift in `30` repos.
- Recall-loss gate on `crates` passed with `0` false merges and `0` canon-preservation
  violations. `nose 0.16.0` did not support `--recall-loss-report`, so deterministic
  recall-loss diffs start with 0.17.0 artifacts.
- Full local CI must pass before tagging; see [continuous integration](continuous-integration.md)
  and the release steps in [CONTRIBUTING](../CONTRIBUTING.md#releasing).

## Performance fixes

Three release-prep profiling fixes turned abnormal corpus regressions into an
acceptable release-candidate runtime delta:

- Swift-heavy imported call-target evidence now builds one imported-binding index per file.
  The `fastlane` Swift call-target outlier dropped from `854.1ms` to `1.9ms`.
- C-heavy assignment domain propagation now indexes domain evidence by anchor during
  lowering instead of scanning every evidence record for every assignment. `raylib`
  parse+lower dropped from `1533.0ms` to `474.3ms` on the absolute-path release check.
- Rust import snapshot resolution now uses compact module identities instead of
  absolute-path suffix hashes, and builds mutation/use indexes lazily. Absolute-path
  `rustls` import-resolve dropped from `152.2ms` to `10.9ms`.

The remaining repo-level runtime triggers are investigation prompts rather than release
blockers. They are dominated by small absolute timings, expected family-count increases,
or already-profiled surfaces.

Post-release follow-up is tracked in
[0.17.0 post-release runtime triage](runtime-triage-0.17.0.md). That triage partially
fixed the clearest no-family-growth hot path (`arrow` class value fingerprinting) without
changing query JSON output, and leaves `arrow` residual class-data cost plus `minio` as
the next no-family-growth performance investigations.
