# Incremental cache benchmark

This page is the measurement contract for the [Instant Monorepo engine](https://github.com/corca-ai/nose/issues/871).
It is deliberately fixed before the cache architecture changes. A faster result is not accepted if
it cannot prove that every history-bearing result is the same result a clean scan would produce.

## Correctness invariant

For each revision, same binary, command, checkout-relative root, and environment:

```text
no cache stdout == empty-store stdout == history-bearing-store stdout
```

The comparison is byte-for-byte over complete `all top=0 --format json` output. It therefore
covers family membership, ids, order, witnesses, ranking, surfaces, and metadata together.
The seed revision is also compared with a clean scan before a mutation is applied. A missing
fixture, failed mutation, non-zero query, malformed JSON, absent measurement, or unequal output
makes the replay fail; it never becomes a skipped success.

Cross-version performance comparisons are different: version/schema fields may intentionally
change, so stdout is not compared between releases. Instead, each binary must independently
prove clean/empty-store/history-store equality on the same pinned checkout. The paired runner
alternates candidate-first and official-first replays to price machine noise fairly. The
mutation manifest fixes the source and output identity algorithms.

## Published v0.19.0 baseline

The checked [`official-v0.19.0-binaries.v1.json`](../bench/cache/official-v0.19.0-binaries.v1.json)
pins every published platform archive, its extracted executable, the annotated tag object and
peeled commit, and the release distribution manifest. A source rebuild is not an accepted
baseline. The downloaded archive and extracted executable must both match before measurement.

The same manifest binds the already checked 120-repository release evidence: established
semantic `28,964.56 ms` and expanded default `36,159.03 ms`, each with one warmup and three
alternating iterations. Later clean-scan comparisons retain the epic's 5% limit and use a
same-binary control before attributing a regression.

## Checked #872 evidence

The checked [`issue-872-v0.19.0-vs-candidate-sympy-paired-2026-07-20.v1.json`](../bench/cache/issue-872-v0.19.0-vs-candidate-sympy-paired-2026-07-20.v1.json)
contains all 180 raw rows from 30 alternating AB/BA replays on the pinned SymPy checkout. The
official executable came from the verified `aarch64-apple-darwin` v0.19.0 archive at commit
`0985e696`; the candidate is commit `2ac8b411`. Both roles independently passed exact
clean/cold/warm output equivalence.

| Phase | Official p50 / p95 | Candidate p50 / p95 | Candidate delta p50 / p95 |
| --- | ---: | ---: | ---: |
| Clean | 1081.86 / 1124.18 ms | 1075.26 / 1127.46 ms | -0.61% / +0.29% |
| Empty store | 1128.42 / 1192.17 ms | 1120.79 / 1182.97 ms | -0.68% / -0.77% |
| Warm store | 690.65 / 758.04 ms | 702.69 / 757.30 ms | +1.74% / -0.10% |

This is the locked baseline, not a claim that the current cache is already instant. On the
candidate's warm run all 1,584 inputs hit, but the p50 cache stage still takes 48.3 ms, the
store is 379,910,220 bytes, and total p50 remains 702.69 ms because most global work is repeated.

The checked [`issue-872-mutation-matrix-receipt-2026-07-20.v1.json`](../bench/cache/issue-872-mutation-matrix-receipt-2026-07-20.v1.json)
seals the complete 2,100-row raw matrix by SHA-256 while retaining every summary and source
identity in the repository. All 14 executable mutations passed 30 replays. Representative warm
hit/miss closures are no-op `3/0`, leaf edit `2/1`, provider export edit `1/2`, high fan-out
`1/33`, Swift global barrier `0/3`, and same-size/restored-mtime edit `3/1`. The 2,668,770-byte
raw report stays under `target/` and is reproducible from the receipt instead of being duplicated
in the repository.

The deterministic scale tiers also pass exact clean/cold/warm equivalence. These are one-replay
capacity smokes, deliberately not p50/p95 acceptance evidence:

| Files | Clean | Empty store | Warm store | Store | Warm peak RSS |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 37.40 ms | 74.30 ms | 40.73 ms | 6.46 MiB | 23.75 MiB |
| 10,000 | 256.99 ms | 707.71 ms | 320.34 ms | 64.65 MiB | 151.45 MiB |
| 100,000 | 2732.10 ms | 7294.07 ms | 3699.44 ms | 646.87 MiB | 1372.92 MiB |

The 100k numbers make the next engineering constraint explicit: #873 and its successors must
reduce both repeated global work and the roughly linear serialized-store footprint. A scheduled
30-replay 100k run remains release evidence; it does not belong in ordinary PR CI.

The checked [#873 portable-CAS evidence](portable-cache-artifacts.md#checked-873-performance-evidence)
repeats the published-binary SymPy comparison after replacing v14's u64/JSON entries. Across 30
alternating replays, clean p50 is +2.0%, cold p50 +4.7%, and warm p50 +5.5% versus the official
binary; exact same-binary output equivalence passes for both roles. The independently checksummed
named-MessagePack store is 190,665,950 bytes, 49.8% smaller than the official 380,153,028-byte
store, while warm p50 RSS is 6.5% lower. Those numbers price the #873 trust boundary before later
issues remove repeated pipeline stages.

## What the current cache actually reuses

The published v0.19.0 cache is schema v11 and the locked #872 candidate is schema v14. #873 moved
the 0.20 development tree to layered CAS v1 while keeping only units/syntax active. #874 now reuses
source snapshots, raw lowering, dependency-aware resolved IL, and units/syntax. A warm clean-Git
hit avoids source reads for lowering and skips parsing; the still-global line-frequency/ranking
stage may read source later. Dirty, untracked, and non-Git inputs are read so their exact bytes,
rather than mtime/size, establish identity.

CAS v1 replaces the u64 entry name with a stage/schema-separated SHA-256 address over the complete
post-resolution semantic/reporting identity and unit-affecting options. An independent payload
SHA-256, exact length, and envelope identity make corrupt or misplaced bytes clean misses. Paths,
`FileId`s, and interner ids are portable and rebound; names, spans, suppression, facets, and full
evidence records remain identity-bearing. Resolved entries add a deterministic
consumer-visible export/dependency context, so provider-private changes leave importers hot while
export, ambiguity, deletion/rename, and Swift-global changes reach their consumers. Global
detection, source-line frequency reads, family construction/ranking, and presentation still
repeat; #875 owns that boundary.
See [portable cache artifacts](portable-cache-artifacts.md).

#275 is the required cross-file regression. A provider literal and importer that converge with
an inline literal must remain converged on an empty store, warm store, and after the provider
changes. The cache key must follow the resolved semantic dependency rather than source mtime or
size.

## Measurement phases

Each mutation replay has two revisions and five subprocesses:

1. clean scan of the seed revision;
2. empty-store seed scan, compared with step 1;
3. apply the declared mutation and verify its source identity;
4. clean and empty-store scans of the new revision;
5. history-bearing scan using the store seeded in step 2, compared with both step-4 scans.

`NOSE_TIME=1` supplies stage timings. Cache instrumentation reports files, hits, misses, bytes
read, and bytes written. #874 additionally emits a `nose.invalidation/v1` JSON closure with
source/raw/resolved counts, exact reasons, global dependency markers, and explicit fail-safe
over-invalidation. The harness retains that object on cached candidate rows and also measures
recursive regular-file store bytes and peak
RSS for each subprocess. Report summaries use the ordinary median and nearest-rank p95
(`ceil(0.95 * n)`, one-indexed) over at least 30 successful replays. Raw rows remain in the
artifact; p50/p95 never replace them. A checked receipt may seal a large local raw report while
retaining its hash, byte size, row count, provenance, identities, equivalence, and complete
summaries.

## Workloads and mutation closure

[`mutation-manifest.v1.json`](../bench/cache/mutation-manifest.v1.json) is the executable
inventory. It pins SymPy, Prettier, Netty, and Fastlane plus deterministic 1k, 10k, and 100k
synthetic tiers. The matrix covers no-op, leaf and provider edits, high fan-out, add/delete/
rename, embedded regions, ignore/exclude/root changes, analysis versus view config, baseline/
ignore inputs, semantic packs, Swift global barriers, same-size restored-mtime edits, and
base/branch switches.

Every row names its changed source identity and expected invalidation closure. The closure is a
correctness expectation, not permission to reuse more work: implementations may conservatively
recompute, but a high-fanout closure at or below 10% may not reparse or reresolve the full corpus
once the dependency-aware engine is under test.

The synthetic generator uses unique, stable checkout-relative paths and source bytes; it must
not manufacture 100k identical cache keys. Real repositories must match their pinned commits.
The 100k tier is an explicit scheduled benchmark, not part of ordinary PR CI.

## Reproduce and validate

Build the candidate from a committed revision, then run the complete mutation matrix. The
validator recomputes every summary from raw rows and rechecks every per-replay output identity;
a development run below 30 replays cannot validate as release evidence.

```sh
cargo build --release -p nose-cli
REVISION=$(git rev-parse HEAD)
python3 scripts/cache-query-regression.py \
  --binary target/release/nose --binary-revision "$REVISION" \
  --fixture all --replays 30 \
  --output target/cache-mutation-matrix.json
python3 scripts/cache-query-regression.py \
  --validate-report target/cache-mutation-matrix.json
python3 scripts/cache-query-regression.py \
  --write-receipt target/cache-mutation-matrix.json \
  --output target/cache-mutation-matrix-receipt.json
```

For the paired published-release comparison, use the same candidate arguments plus the pinned
real workload and verified official executable:

```sh
python3 scripts/cache-query-regression.py \
  --binary target/release/nose --binary-revision "$REVISION" \
  --compare-official-binary target/v0.19.0/nose \
  --compare-official-revision 0985e6963c58d5a97e523bc532b88aa5e34f2ef9 \
  --official-target aarch64-apple-darwin \
  --official-archive target/v0.19.0/nose-cli-aarch64-apple-darwin.tar.xz \
  --root bench/repos/sympy --label sympy --replays 30 \
  --output target/cache-sympy-paired.json
```

The harness verifies both official archive and executable checksums before starting. It gives
the two binaries separate stores, alternates their order on every replay, and requires exact
same-binary equivalence. A failed phase or missing cache evidence never produces an output
artifact.
