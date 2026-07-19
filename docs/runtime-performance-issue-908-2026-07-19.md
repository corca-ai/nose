# Normalize-and-extract closeout for #908

Generated on 2026-07-19. The [durable #908 performance
artifact](../bench/recall_loss/issue-908-normalize-extract-performance-2026-07-19.v1.json)
binds the frozen reproduction, exact binaries, pre-edit profiles, r40 results, and output
hashes below.

## Outcome

#908 removes the inherited Guava and MinIO `normalize+extract` signals without changing
semantic output. A shared MinHash signing loop was the largest optimizable function in
both call-stack profiles. Transposing its loops keeps one minimum in a register and
writes each signature element once instead of revisiting the whole mutable signature
for every feature.

At the unchanged `5%` / `5 ms` contract, the final control-adjusted stage movement is:

| Repository | Frozen #892 residual | Fixed r40 |
| --- | ---: | ---: |
| Guava | +26.50 ms / +7.39% | -17.80 ms / -5.21% |
| MinIO | +7.05 ms / +6.39% | -6.05 ms / -5.73% |

The final Guava checker status is `pass`. The corrected two-repository r40 has no MinIO
trigger; its only requested follow-up was a Guava frontend signal, and the single
Guava-only focused primary/control run cleared it. No threshold changed.

## Pre-edit attribution

The frozen #892 candidate and official v0.19.0 binary were each profiled once per
repository, sequentially with `RAYON_NUM_THREADS=1`. Pass-level logging was diagnostic,
not a replacement performance verdict: emitting tens of thousands of per-unit lines
substantially increases elapsed time.

| Profile evidence | Guava v0.19 → candidate | MinIO v0.19 → candidate |
| --- | ---: | ---: |
| Instrumented `normalize+extract` | 5,980.0 → 6,056.0 ms | 1,426.3 → 1,447.3 ms |
| Summed value-graph build | 299.7 → 307.4 ms | 113.3 → 113.8 ms |
| `minhash::sign` top-stack samples | 322 → 302 | 112 → 114 |

There was no candidate-only soundness function dominating either repository. MinHash
signing was instead the largest common product function in both versions. This made it
the smallest shared optimization that could help both corpora without weakening #900 or
#859 semantics.

## Implementation and safety

MinHash still computes exactly `sig[i] = min_f h_i(f)` with the same SplitMix64 hash,
seed order, feature set, and result order. Only traversal changes:

- before: visit every feature, conditionally updating all 128 signature slots;
- after: visit every seed, retain its minimum locally, then write that slot once.

A unit test compares both algorithms over empty and representative feature sets with
`0`, `1`, `2`, `64`, and `128` seeds. Direct product replay is also byte-identical:

- Guava SHA-256: `cf37a623…fbcf5`;
- MinIO SHA-256: `3dd18607…159f`.

That equality holds both for current-main pre/post binaries and for the exact #892
candidate versus the isolated #908 fix. Family count, order, surfaces, metadata, output
bytes, and determinism are unchanged.

## Measurement discipline

The official v0.19.0 binary remains the product baseline. To isolate this fix from work
merged after #892, the shipping commit was cherry-picked onto the exact #892 candidate
source in a detached worktree and built once. The resulting fixed binary has file
SHA-256 `8a234bba…07194` and normalized code SHA-256 `e5c06c37…432b3`.

An initial r40 used current main and was excluded because post-#892 divergence and pack
work changed independent stages; it was not replaced to seek a better sample. The
corrected frozen two-repository r40 was followed by exactly one Guava primary/control
r40 because the unchanged checker requested that focused confirmation. No additional
performance run was taken.
