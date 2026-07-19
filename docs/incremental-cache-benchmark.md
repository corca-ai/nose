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
change, so the published-release comparison uses the documented semantic projection while a
same-binary control prices noise. The mutation manifest fixes the source and output identity
algorithms.

## Published v0.19.0 baseline

The checked [`official-v0.19.0-binaries.v1.json`](../bench/cache/official-v0.19.0-binaries.v1.json)
pins every published platform archive, its extracted executable, the annotated tag object and
peeled commit, and the release distribution manifest. A source rebuild is not an accepted
baseline. The downloaded archive and extracted executable must both match before measurement.

The same manifest binds the already checked 120-repository release evidence: established
semantic `28,964.56 ms` and expanded default `36,159.03 ms`, each with one warmup and three
alternating iterations. Later clean-scan comparisons retain the epic's 5% limit and use a
same-binary control before attributing a regression.

## What the current cache actually reuses

The published v0.19.0 cache is schema v11; current main is schema v12. Both always rediscover,
read, parse, and lower every selected source, rebuild corpus import facts, and repeat global
detection, family construction/ranking, and presentation. They reuse only per-file
normalize/extract units and syntax streams, keyed by post-resolution IL plus unit-affecting
options. In particular, a warm hit does **not** skip parsing. This narrower statement replaces
the old CI documentation claim.

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
read, and bytes written; the harness also measures recursive regular-file store bytes and peak
RSS for each subprocess. Report summaries use the ordinary median and nearest-rank p95
(`ceil(0.95 * n)`, one-indexed) over at least 30 successful replays. Raw rows remain in the
artifact; p50/p95 never replace them.

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
