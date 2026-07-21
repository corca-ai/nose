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

## Checked #874 evidence

The checked [`issue-874-dependency-invalidation-sympy-paired-2026-07-20.v1.json`](../bench/cache/issue-874-dependency-invalidation-sympy-paired-2026-07-20.v1.json)
contains 30 alternating AB/BA replays of implementation commit `e1617924` against the
checksum-verified published v0.19.0 `aarch64-apple-darwin` binary. Both roles independently passed
exact clean/empty/history output equivalence across all 180 rows.

| Phase | Official p50 / p95 | #874 p50 / p95 | #874 delta p50 / p95 |
| --- | ---: | ---: | ---: |
| Clean | 1224.78 / 1472.50 ms | 1170.97 / 1367.38 ms | -4.39% / -7.14% |
| Empty store | 1333.26 / 1628.77 ms | 1984.47 / 2382.15 ms | +48.84% / +46.25% |
| Warm store | 843.82 / 985.50 ms | 839.03 / 1031.98 ms | -0.57% / +4.72% |

The clean path remains inside the epic's 5% gate and is faster in this paired run. The empty-store
cost is intentionally reported as a regression: #874 now writes source, raw-IL, dependency,
resolved-IL, and units artifacts instead of only the published release's units cache. On a no-op
warm run all 1,584 raw, resolved, and unit regions hit; 1,510 resolved regions are raw
pass-throughs, so they do not duplicate payloads. Total warm p50 is effectively neutral while
p95 is 4.72% higher because global detection and source-line ranking still repeat. #875 owns that
remaining latency boundary.

The added layers do not recreate the earlier uncompressed expansion: the #874 store is
333,945,915 bytes versus the official 380,153,028 bytes (-12.15%). Warm p50/p95 peak RSS is
841,506,816 / 850,673,664 bytes versus 1,066,860,544 / 1,090,748,416 bytes (-21.12% / -22.01%).

The checked [`issue-874-mutation-matrix-receipt-2026-07-20.v1.json`](../bench/cache/issue-874-mutation-matrix-receipt-2026-07-20.v1.json)
seals a 4,356,455-byte, 2,100-row raw report. All 14 mutations passed 30 replays with exact
clean/empty/history equality. Representative history-bearing unit hit/miss closures are no-op
`3/0`, leaf edit `2/1`, provider-private edit `2/1`, provider export edit `1/2`, high fan-out
`1/33`, add/delete/rename `3/1`, embedded-region edit `5/1`, restored-mtime edit `3/1`, and Swift
global barrier `0/3`.

## Checked #875 evidence

The checked [`issue-875-incremental-global-sympy-paired-2026-07-20.v1.json`](../bench/cache/issue-875-incremental-global-sympy-paired-2026-07-20.v1.json)
contains all 180 rows from 30 alternating AB/BA replays of implementation commit `e93bdc05`
against the checksum-verified published v0.19.0 binary. Both roles independently passed exact
clean/empty/history output equivalence.

| Phase | Official p50 / p95 | #875 p50 / p95 | #875 delta p50 / p95 |
| --- | ---: | ---: | ---: |
| Clean | 1194.41 / 1451.44 ms | 1193.42 / 1428.13 ms | -0.08% / -1.61% |
| Empty store | 1353.92 / 1667.48 ms | 3080.78 / 3572.17 ms | +127.54% / +114.23% |
| Warm store | 887.12 / 991.24 ms | 917.04 / 970.76 ms | +3.37% / -2.07% |

The no-op warm path reuses all 318,443 candidate buckets, 647,289 scores, 26,356 connected
evaluations, 1,584 syntax streams, and 5,172 family-line analyses in the pinned SymPy workload.
Its candidate stage is 34.0 ms p50 and source-line stage 22.7 ms p50. Warm p95 improves 2.07%,
while p50 is 3.37% slower; warm p50/p95 RSS improves 19.97%/19.55%. Clean remains neutral.

The empty-store regression is intentionally not normalized away: building the new global state
more than doubles cold latency and grows the store 17.93% to 448,315,564 bytes. #876 owns compact,
transactional, bounded generations and must price this measured debt directly.

The checked [`issue-875-mutation-matrix-receipt-2026-07-20.v1.json`](../bench/cache/issue-875-mutation-matrix-receipt-2026-07-20.v1.json)
seals a 4,356,466-byte, 2,100-row raw report (`e66a61fa…`) covering all 14 mutations over 30
replays. Every clean/empty/history comparison passed byte-for-byte, including add/delete/rename,
provider fan-out, semantic-pack and config changes, embedded regions, Swift global barriers, and
same-size restored-mtime edits.

## Checked #876 evidence

The checked [`issue-876-transactional-store-sympy-paired-2026-07-20.v1.json`](../bench/cache/issue-876-transactional-store-sympy-paired-2026-07-20.v1.json)
contains all 180 rows from 30 alternating AB/BA replays of implementation commit `6b13adaa`
against the checksum-verified published v0.19.0 binary. Both roles independently passed exact
clean/empty/history output equivalence.

| Phase | Official p50 / p95 | #876 p50 / p95 | #876 delta p50 / p95 |
| --- | ---: | ---: | ---: |
| Clean | 1120.06 / 1480.92 ms | 1163.42 / 1388.26 ms | +3.87% / -6.26% |
| Empty store | 1320.03 / 1662.34 ms | 8864.49 / 9388.52 ms | +571.54% / +464.78% |
| Warm store | 893.96 / 1038.31 ms | 1866.58 / 1920.24 ms | +108.80% / +84.94% |

The active store is 148,668,018 bytes for 27,214,294 bytes of Python source: 5.46× source and
60.89% smaller than the official binary's 380,153,026-byte store. This passes both #876 disk
gates. Clean p50/p95 RSS is 1.13%/2.39% below official, so the clean resource gate also passes.

The then-remaining resource gate was deliberately not marked complete. Warm no-op p50/p95 RSS is
713,596,928/743,718,912 bytes, 66.91%/68.64% of official rather than the required ≤60%. A separate
one-run leaf-edit characterization measured 809,074,688 bytes against official's 1,086,472,192
bytes (74.47%); it is diagnostic, not 30-replay release evidence. At that revision the warm path
still restored the whole corpus and every unit before applying the incremental global state. The
checked #877 leaf evidence below supersedes this characterization and closes the ≤60% gate.

The first-generation cost is also visible rather than normalized away. Per-object filesystem sync
was removed only for immutable, checksummed CAS entries—a lost or corrupt entry is a safe miss—while
generation manifests and `CURRENT` retain file-and-directory sync. Compact serialization and
compression still make the empty-store and warm runs slower than v0.19. #877 avoids the
whole-corpus restore for exact no-op and independently provable leaf updates without weakening
integrity or disk bounds.

## Checked #877 evidence

The checked [`issue-877-policy-leaf-sympy-paired-2026-07-21.v1.json`](../bench/cache/issue-877-policy-leaf-sympy-paired-2026-07-21.v1.json)
contains all 180 rows from 30 alternating AB/BA replays of implementation commit `42bfbdd5`
against the checksum-verified published v0.19.0 binary. Each replay seeds a store on pinned SymPy
commit `da4a5fa5`, changes one dependency-free production leaf, and independently proves
clean/empty-store/history-store byte equality for both binaries.

| Phase | Official p50 / p95 | #877 p50 / p95 | #877 delta p50 / p95 |
| --- | ---: | ---: | ---: |
| Clean | 1247.10 / 1586.36 ms | 1063.56 / 1337.27 ms | -14.72% / -15.70% |
| Empty store | 1430.24 / 1749.78 ms | 8544.00 / 9270.56 ms | +497.38% / +429.81% |
| Warm leaf | 933.86 / 1045.05 ms | 1680.05 / 2287.41 ms | +79.90% / +118.88% |

Warm-leaf p50/p95 peak RSS is 345,186,304/347,062,272 bytes versus official
1,069,785,088/1,084,768,256 bytes: **32.27%/31.99% of official**, comfortably inside #876's
≤60% criterion. Clean RSS is 1.00%/2.41% lower than official, and clean time remains inside the
#877 5% limit. Every candidate history row reports exactly 1,583 unit hits and one miss. The
managed store is 148,899,990 bytes at p50 versus official 380,106,480 bytes (-60.83%). This closes
#876's last resource gate.

The latency debt is not hidden: direct per-region restoration keeps only compact units live, but
its checksum, decompression, and deserialization work makes this leaf workload slower than the
published cache. First-generation construction also remains much slower. Later #871 milestones
may reduce that cost; neither latency is part of the closed #876 resource criterion.

## Checked #878 watch-session evidence

The checked [`issue-878-watch-session-2026-07-21.v1.json`](../bench/cache/issue-878-watch-session-2026-07-21.v1.json)
contains 30 leaf revisions at each deterministic session tier. Every one of the 60 emitted full
dashboard snapshots equals a fresh no-cache query of that revision. Each tier is also killed
mid-run with `SIGKILL`, reopened against the same transactional store, and compared with a fresh
query before measurement continues.

| Files | Ready p50 / p95 | End-to-end p50 / p95 | Active peak RSS | Store |
| ---: | ---: | ---: | ---: | ---: |
| 10,000 | 97.78 / 100.88 ms | 110.23 / 113.27 ms | 210.52 MB | 34.91 MB |
| 100,000 | 597.80 / 631.48 ms | 610.39 / 789.56 ms | 1.93 GB | 360.75 MB |

Ready latency runs from the first event in the debounced batch through the complete dashboard
snapshot. End-to-end also includes JSON serialization and pipe delivery. Both p95 values pass the
epic's 250ms/1s active-session gates with the full snapshot contract intact.

The report verifies the downloaded v0.19.0 Apple Silicon archive (`097c7e76…`) and executable
(`0f73ea54…`) against the published baseline manifest, and binds the passing #877 one-shot
clean/history equivalence artifact. The synthetic files are intentionally tiny, so their per-file
store ratios are not substituted for the real large-source resource gate: the #877 SymPy store is
the checked 5.46×-or-better evidence. The active-session RSS above is reported separately and not
normalized against a one-shot process; it is the cost of retaining units and detection state for
sub-second revisions.

## What the current cache actually reuses

The published v0.19.0 cache is schema v11 and the locked #872 candidate is schema v14. #873 moved
the 0.20 development tree to layered CAS v1, #874 activated source/raw/resolved IL reuse, and #875
now reuses global detection, syntax components, line document frequencies, and family-line
analyses. A warm clean-Git hit avoids source reads for lowering and skips parsing; an unchanged
line manifest also avoids loading the full line index. #877 additionally restores units directly
without materializing the raw/resolved corpus for an exact no-op or one dependency-free leaf whose
export and resolution summaries are unchanged. Dirty, untracked, and non-Git inputs are read so
their exact bytes, rather than mtime/size, establish identity.

The foreground policy is bounded by reuse value. One-shot scans of at most 512 discovered source
files retain the complete source/raw/resolved and line-index history used by dependency-aware
invalidation. Above that boundary, the common exact no-op/independent-leaf path retains compact
unit and snapshot state but does not publish the fallback portable-IL layer or persistent line
dictionary; line weighting uses the same clean parallel implementation. Large cold scans publish
those unit payloads as one chunk-verified pack instead of one filesystem object per region. The
pack's SHA-256-bound table records the ordered region keys, offsets, lengths, and per-region CRC32;
a damaged table or used region is a miss and the exact source regenerates the pack. Pack
publication overlaps later detection and ranking work, but the transactional generation commit
joins the publication first, so a snapshot can never reference an unfinished pack. A miss still
lowers and resolves exact source, so this boundary changes only performance and cache
observability, never query output. Watch sessions always retain their in-memory incremental line
state, and small provider/high-fanout workloads keep the full dependency history.
First-generation persistent detection state is similarly capped at 20,000 units; large one-shot
runs use the clean detector while the unit cache and active watch session remain available.

CAS v1 replaces the u64 entry name with a stage/schema-separated SHA-256 address over the complete
post-resolution semantic/reporting identity and unit-affecting options. Ordinary entries use an
independent payload SHA-256, exact length, and envelope identity; the large unit pack uses its
SHA-256 table identity plus exact bounds and per-region checksums. Corrupt or misplaced bytes are
clean misses in either representation. Paths, `FileId`s, and interner ids are portable and
rebound; names, spans, suppression, facets, and full evidence records remain identity-bearing.
Resolved entries add a deterministic
consumer-visible export/dependency context, so provider-private changes leave importers hot while
export, ambiguity, deletion/rename, and Swift-global changes reach their consumers. Global
detection buckets/pair scores/components, connected and same-unit witnesses, syntax components,
line frequencies, and family diffs/weights update from persistent state. Query filters, rendering,
and final presentation remain request-local.
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

To reproduce the checked leaf-update comparison, add a safe repository-relative replacement:

```sh
  --leaf-path sympy/plotting/pygletplot/plot_object.py \
  --leaf-find 'if self.visible:' \
  --leaf-replace 'if self.visible is True:'
```

The harness verifies both official archive and executable checksums before starting. It gives
the two binaries separate stores, alternates their order on every replay, and requires exact
same-binary equivalence. A failed phase or missing cache evidence never produces an output
artifact.

For the active-session tiers and crash replay:

```sh
python3 scripts/watch-session-benchmark.py \
  --binary target/release/nose --replays 30 \
  --output target/watch-session.json
python3 scripts/watch-session-benchmark.py \
  --validate-report target/watch-session.json
```

The runner requires the local published v0.19.0 archive and executable to match the baseline
manifest before measuring. It records every raw revision row; a tier below 30 replays, any clean
snapshot mismatch, a failed crash restart, or a p95 above its fixed target makes validation fail.
