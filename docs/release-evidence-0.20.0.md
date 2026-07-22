# 0.20.0 release evidence

Generated on 2026-07-22 for issue #943. The release decision is **GO**: every
blocking query-performance, cache, watch-session, and Soundness Lab condition
passed against the same immutable product candidate.

The compact machine-readable receipts are the
query [performance evidence](../bench/release/0.20.0/query-performance-943.v1.json),
the cache [performance evidence](../bench/cache/release-0.20.0-cache-943.v1.json),
the [watch-session report](../bench/cache/release-0.20.0-watch-943.v1.json), and the
final [Soundness Lab release gate](../bench/soundness/0.20.0/release-gate-943.v1.json).
CI validates their identities and conclusions with
`scripts/check-release-evidence-0.20.0.py`.

## Candidate and baseline

The measured candidate is source commit
`a544d03b6801871dbcd90bcb370825942d6851c8`, whose product `crates` tree is
`6d38b79884a44d1fe38a47cec19ca4d9a2ef7570`. The Darwin arm64 binary SHA-256
is `ad1f3fa3695168083be85ed81b199e059c91c45dcff769e1c9d99e6597081328` and
its machine-code SHA-256 is
`4dc1b4e18bf11777f1319d4ca89df35c582d1619ea56cd3f713f8f987f9613ac`.
The archived candidate SHA-256 is
`7187ac4d634ab64519827f7bd92604fb7d9c0a5f9f161ae0337904b39d6011c4`.
It was built with Rust 1.96.0, Xcode 26.3, and the macOS 26.2 SDK.

The comparison executable is the published v0.19.0 Darwin arm64 asset, not a
local rebuild. Its archive SHA-256 is
`097c7e766e9ab756a32cec715897067d1360e145074715168a653962be409981`,
its binary SHA-256 is
`0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3`,
and its machine-code SHA-256 is
`e55d0e989993ff1d1d6b4e933dbd3f5ade38203368b8321d3a7842799a95aca6`.
The release source is `0985e6963c58d5a97e523bc532b88aa5e34f2ef9`.

The 120-repository corpus manifest SHA-256 is
`87b3defc02c87e53f5ce20d10b68afdbc7190a6db5d5bfdb6b655b305bbc7ba8`,
the prune manifest SHA-256 is
`c22f34d3ab4da9b89b5938140bbfdf7664178b3b7b57e5ea3937ba0bb47c2980`,
and the frozen 17-repository base workload SHA-256 is
`ed76a6a2b5b2551dfd61f627998c6db50e0be70fb479067ccabf7b42f97b2ad6`.
These identities are part of the checked query receipt rather than being inferred
from the current checkout.

The versioned release commit and tag are created after these gates. Its `crates`
tree is `c1935323e5d046c9f8600b2d91c082e2cefbd3c7`: the only changes from the
measured tree extend semantic-pack compatibility ranges in two test fixture files
through the v0.20 release line. The binding pins both files' before/after Git blobs,
rejects every other `crates` change, and records that shipped product code is
unchanged.

## Query performance

All comparisons use the preregistered order-aware v3 paired-block estimator:
five primary blocks with five timed samples per observation, one warmup, and
both baseline/candidate orders. A repository is a material regression only when
the corrected delta is both greater than 5% and greater than 5 ms. A primary
signal receives exactly one six-block focused rerun; a remaining trigger or
inconclusive result fails the release.

| Workload | Repositories | Aggregate runtime delta | Focused rerun | Final unresolved |
| --- | ---: | ---: | --- | ---: |
| frozen base workload | 17 | -7.16% (-720.98 ms) | none | 0 |
| default | 120 | -3.28% (-1,234.65 ms) | swift-composable-architecture, sympy | 0 |
| semantic | 120 | -4.61% (-1,266.42 ms) | alamofire, sympy | 0 |
| near 0.8, no pack | 120 | -4.46% (-1,522.95 ms) | alamofire, h2database | 0 |

All expected output drift is declared in checked manifests and all four final
decisions have zero unexpected drift and zero unused declarations. There is no
query-performance release blocker.

The earlier [semantic-pack 0.20 closeout](../bench/semantic_pack/issue-870-epic-closeout-2026-07-19.v1.json)
remains the feature-contract artifact. The final candidate is independently
covered here by the all-120 semantic and near/no-pack workloads, the semantic-pack
cache mutation, and CI's checked example and pricing-artifact validation. Thus the
feature closeout is not being substituted for final-tree release measurement.

## Cache and watch performance

The paired SymPy leaf and no-op campaigns contain 30 alternating official and
candidate replays. Clean, empty-store, and history-bearing outputs are exact on
both binaries. The candidate has no material p50 or p95 regression in any phase.
The largest positive comparison is the SymPy leaf clean-run p95 at only
`+9.66 ms` (`+0.75%`); warm history is 61.7-64.5% faster.

Candidate-only 30-replay no-op campaigns on Prettier, Netty, and Fastlane also
require clean, empty-store, and history-store output equality. This is the
applicable correctness comparison for those repositories: the published v0.19.0
binary itself does not preserve clean-versus-empty-store output on Prettier, so
that old behavior is not used as a correctness oracle.

The complete 14-mutation matrix contains 2,100 rows (14 workloads × 30 replays ×
five phases). It covers no-op, leaf/provider/private/high-fanout edits,
add/delete/rename, analysis/view/baseline/ignore configuration, embedded regions,
semantic packs, Swift-global invalidation, and restored-mtime changes. Every
clean, empty-store, and history result is identical. Its raw local artifact is
sealed in the checked receipt by SHA-256, byte count, row count, provenance, and
summaries.

Resource gates also pass:

- SymPy store p95 is 150,786,981 bytes, 5.54× source size (limit 6×).
- The store is 39.67% of official v0.19.0 (required at most 50%).
- Warm-leaf RSS p50/p95 is 44.86%/44.82% of official (limit 60%).

The exact candidate watch report contains 30 revisions at each scale. Every
snapshot equals a fresh one-shot query, and both sessions pass a forced crash and
restart against the same transactional store.

| Files | Ready p50 / p95 | End-to-end p50 / p95 | Peak RSS | Store |
| ---: | ---: | ---: | ---: | ---: |
| 10,000 | 96.82 / 101.73 ms | 109.37 / 114.36 ms | 218.99 MB | 34.92 MB |
| 100,000 | 557.65 / 579.66 ms | 570.20 / 592.44 ms | 3.03 GB | 294.45 MB |

The report binds its one-shot equivalence evidence to the same source commit and
binary SHA-256, closing the earlier possibility of validating watch behavior with
a different build.

## Soundness Lab

The checked [release binding](../bench/soundness/0.20.0/release-binding-943.v1.json)
connects the exact candidate to the coverage
overlay [evidence](../bench/soundness/0.20.0/release-overlay-943.v1.json), the
focused [falsification evidence](../bench/soundness/0.20.0/release-falsification-943.v1.txt), the
Type-4 [blind-attack evidence](../bench/soundness/0.20.0/release-blind-attack-943.v1.json),
current exclusion attribution, and query-performance receipt.

- The complete [nightly replay](../bench/soundness/0.20.0/release-nightly-943.v1.json)
  passes all 120 pinned repositories: 0 failures, 0 false merges, and 0 canon
  violations. Its deterministic result is
  `a3db6ea806dd18b4abcbcda9811e7e4fe588211e775b23cf1e4ef5654d6bab24`.
- The [deep campaign](../bench/soundness/0.20.0/release-deep-943.v1.json) passes
  independent source/runtime calibration, all 430 metamorphic equivalence tests,
  and three fixed-seed falsification searches.
- Focused falsification checks 13,283 units and 25,766 executed cases with zero
  false merges, zero canon violations, and zero new distinguishers.
- The Type-4 blind attack checks 483 units and 54 fingerprint groups with zero
  false merges and zero canon violations.
- Frozen risk-weighted coverage is 45.76% against a 42.50% release target, with
  35/97 verified pairs and no language-floor regression.
- The current census attributes all 12,133 excluded units; generic or
  unattributed exclusions remain zero.

The nightly advisory count rises from 4,756 to 12,016. This is explicitly
non-blocking diagnostic breadth; all hard soundness conditions above remain zero.

## Validation

The compact release package can be validated without the large local raw timing
files:

```sh
python3 scripts/check-release-evidence-0.20.0.py --self-test
python3 scripts/check-release-evidence-0.20.0.py
python3 scripts/soundness-lab-gate.py self-test
python3 scripts/soundness-lab-gate.py check
python3 scripts/check-soundness-scorecard.py --release-commit "$(git rev-parse HEAD)"
./scripts/check-docs.sh
```

The first command checks the release policy boundaries; the second verifies the
checked query, cache, watch, nightly, deep, and final gate artifacts together.
The scorecard command additionally proves that the final release commit retains
the measured candidate's product tree.
