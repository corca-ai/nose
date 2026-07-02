# 20-optimization runtime pass

Generated on 2026-07-02 after the post-0.17.0 runtime-triage setup.

For the reusable process and harnesses, see [runtime triage](runtime-triage.md).
The durable machine-readable artifacts are:

- Same-binary control: [current-performance-baseline-2026-07-02.v1.json](../bench/recall_loss/current-performance-baseline-2026-07-02.v1.json).
- Final all-120 before/after run: [performance-after-20-optimizations-2026-07-02.v1.json](../bench/recall_loss/performance-after-20-optimizations-2026-07-02.v1.json).
- Focused apparent-regression recheck: [performance-after-20-optimizations-top-regression-recheck-2026-07-02.v1.json](../bench/recall_loss/performance-after-20-optimizations-top-regression-recheck-2026-07-02.v1.json).

## Scope

This pass started from `ca48518d` and ended at `a0bfc252`. It first hardened the
runtime-triage tooling and recorded a same-binary control baseline, then landed 20
profile-guided optimization commits:

1. `c915cbe1` compacted long value-graph path guards.
2. `7401378e` indexed strict exact function target spans.
3. `2116f3aa` skipped declaration-only callable units early.
4. `94c8bbc4` shared value-graph reachable analysis for anchors.
5. `4ce92397` used sorted vectors for query-opportunity candidates.
6. `e62b7c83` reused family ids during query JSON rendering.
7. `90321db0` avoided serde conversion in baseline member keys.
8. `de4a918d` shared line diffs for shared-line display counts.
9. `308463a3` deduplicated shared-line votes without per-pair sets.
10. `dd69fcea` used fast hash maps for shared-line weighting.
11. `a39c4b22` used fast hash buckets for structural candidates.
12. `ef7f34d7` borrowed exact value bucket keys.
13. `203a6255` skipped empty module binding dependency walks.
14. `626d189f` returned early for empty module seed bindings.
15. `f1de6dd9` prefiltered overlap opportunity candidates.
16. `faf471a2` avoided allocations while rendering baseline ids.
17. `a04d4e51` used flat line-diff dynamic-programming storage.
18. `3350b1dd` borrowed family anchors during query sorting.
19. `696e7654` used a fast hash map for the file-line cache.
20. `a0bfc252` used fast hash sets for surface overrides.

Rejected prototypes were removed when the measurements did not justify the extra
state or when they changed product output. Examples include a parameter-domain scope
cache, broader query-family id precomputation, count-only anti-unification, extraction
shape sharing, and generated-source index rewrites.

## Method

All broad runs used:

```sh
scripts/query-regression-harness.py \
  --baseline-binary target/nose-perf-before-ca48518d \
  --current-binary target/release/nose \
  --all-repos \
  --iterations 3 \
  --warmups 1
```

The query was `nose query <repo> all top=0 --mode semantic --format json`.

The same-binary control compared `target/nose-perf-before-ca48518d` with itself at
`ca48518d`. The final all-corpus run compared that baseline binary with
`target/release/nose` built from `a0bfc252`.

Because the final broad run showed several large repo-level regressions, the largest
apparent regressions were rechecked with 7 measured iterations and 1 warmup on
`libgdx`, `libsodium`, `guava`, `netty`, `raylib`, `sympy`, and `esbuild`.

## Results

The same-binary all-corpus control measured:

| Run | Aggregate median | Delta | Product hashes |
| --- | ---: | ---: | --- |
| `ca48518d` vs `ca48518d` | `52,814.25ms -> 56,709.78ms` | `+7.38%` | 120/120 identical |

The final all-corpus before/after run measured:

| Run | Aggregate median | Delta | Product hashes | Family counts |
| --- | ---: | ---: | --- | --- |
| `ca48518d` vs `a0bfc252` | `66,498.30ms -> 70,626.21ms` | `+6.21%` | 120/120 identical | 120/120 unchanged |

The largest full-run improvements by absolute time were:

| Repo | Median change | Delta |
| --- | ---: | ---: |
| `alamofire` | `-1,545.70ms` | `-45.59%` |
| `swift-nio` | `-1,001.25ms` | `-59.69%` |
| `cmark` | `-553.91ms` | `-83.09%` |
| `sqlite` | `-445.16ms` | `-24.66%` |
| `nushell` | `-412.27ms` | `-18.86%` |
| `h2database` | `-362.40ms` | `-20.04%` |
| `fzf` | `-261.38ms` | `-36.12%` |

The largest apparent full-run regressions by absolute time were:

| Repo | Full-run median change | Full-run delta |
| --- | ---: | ---: |
| `libgdx` | `+1,386.25ms` | `+53.48%` |
| `libsodium` | `+799.11ms` | `+143.27%` |
| `guava` | `+583.54ms` | `+19.93%` |
| `netty` | `+360.54ms` | `+16.37%` |
| `raylib` | `+346.22ms` | `+14.04%` |
| `sympy` | `+263.00ms` | `+7.53%` |
| `esbuild` | `+259.77ms` | `+11.41%` |

The focused 7-iteration recheck of those apparent regressions measured:

| Run | Aggregate median | Delta | Product hashes |
| --- | ---: | ---: | --- |
| top apparent regressions | `14,874.05ms -> 14,551.12ms` | `-2.17%` | 7/7 identical |

Repo-level focused recheck:

| Repo | Recheck median change | Recheck delta |
| --- | ---: | ---: |
| `libgdx` | `-446.00ms` | `-18.17%` |
| `raylib` | `-27.27ms` | `-1.23%` |
| `guava` | `-9.74ms` | `-0.35%` |
| `esbuild` | `+14.30ms` | `+0.73%` |
| `netty` | `+19.74ms` | `+0.98%` |
| `libsodium` | `+61.37ms` | `+12.43%` |
| `sympy` | `+64.66ms` | `+2.19%` |

## Interpretation

The final all-corpus aggregate is not a clean aggregate speedup. It measured `+6.21%`
slower, so this pass should not be described as a broad corpus-wide runtime win.

The output safety signal is clean: every final broad-run product hash stayed identical,
and every repo kept the same family count.

The broad elapsed-time signal is noisy at this iteration count. The same-binary control
already measured `+7.38%`, and the focused recheck of the largest apparent regressions
measured `-2.17%` aggregate with identical hashes. The clearest conclusion is therefore:

- no product-output regression was introduced;
- no large broad performance degradation was reproduced;
- several targeted hot paths improved materially;
- the all-corpus aggregate gate needs focused rechecks before treating a single
  3-iteration apparent regression as actionable.

## Next Work

Keep using runtime triage before optimizing:

1. For any future broad all-corpus regression, recheck the top apparent regressions with
   at least 7 iterations before changing code.
2. Continue preferring stage-attributed hot paths over global micro-optimizations.
3. Treat `libsodium` as the only remaining focused recheck item from this pass if it
   appears again, because it was still positive in the 7-iteration recheck, though only
   by `61.37ms`.
