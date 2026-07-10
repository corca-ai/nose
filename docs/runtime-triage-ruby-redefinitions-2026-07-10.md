# Ruby redefinition runtime triage

This page records the diagnosis, fix, and reproducible evidence for
[#807](https://github.com/corca-ai/nose/issues/807). It follows the
[runtime triage](runtime-triage.md) process.

## Regression boundary

Commit `f968dcbd` expanded the fail-closed Ruby same-file redefinition analysis for
the option absence-channel work in #804. The semantic behavior was intentional,
but the implementation repeatedly turned file-level facts into per-occurrence work:

- receiver occurrences reran the completed-file redefinition query while emitting
  library-API evidence;
- raw and canonical evidence admission reran the same query after the first-party
  producer had already proved it;
- Method-object and InstructionSequence resolution rescanned all assignments and
  possible targets from recursive helpers; and
- the InstructionSequence receiver search ran before rejecting unrelated method
  selectors.

The one-iteration regression artifact compares the parent `d28d82d7` with
`f968dcbd`:

| Repository | Parent | Regressed | Delta | Family delta |
| --- | ---: | ---: | ---: | ---: |
| `asciidoctor` | 80.50 ms | 13,969.54 ms | +17,254% | 0 |
| `fastlane` | 347.84 ms | 26,617.65 ms | +7,552% | +1 |
| `sidekiq` | 50.42 ms | 2,102.15 ms | +4,070% | 0 |
| **Aggregate** | **478.75 ms** | **42,689.33 ms** | **+8,817%** | **+1** |

The no-family-growth `asciidoctor` and `sidekiq` results classify the regression as
accidental analysis cost rather than justified capability growth.

## Fix

Commit `96c37985` keeps the #804 fail-closed behavior while making the underlying
facts reusable:

- `RubyRedefinitionCache` memoizes completed-IL class/method queries and the Ruby
  core `nil?` safety result;
- `RubyDynamicMethodChangeIndex` builds assignment, Method-object target, and
  literal-mutator buckets once, preserving arena order;
- InstructionSequence receiver resolution memoizes `(file, node, depth)` results
  and rejects unrelated selectors before recursive analysis; and
- raw/canonical admission consumes the asserted first-party occurrence evidence
  after checking builtin provenance, callee shape, and the expected empty dependency
  set instead of repeating the whole-file proof.

The cache is scoped to one completed IL arena. Normalization rebuilds an arena rather
than mutating nodes in place, so each rebuilt IL receives a fresh cache.

## Focused result

The final focused comparison alternated the parent and fix for 15 measured
iterations after two warmups:

| Repository | Parent median | Fix median | Delta | Classification |
| --- | ---: | ---: | ---: | --- |
| `asciidoctor` | 81.42 ms | 83.00 ms | +1.94% / +1.58 ms | small-or-noisy |
| `fastlane` | 346.01 ms | 353.07 ms | +2.04% / +7.06 ms | small-or-noisy |
| `sidekiq` | 51.94 ms | 53.05 ms | +2.15% / +1.11 ms | small-or-noisy |
| **Aggregate** | **479.37 ms** | **489.12 ms** | **+2.03%** | **within budget** |

The 15-iteration same-binary control moved from 495.91 ms to 486.39 ms
(-1.92%). Subtracting that control gives an approximate adjusted delta of +3.95%,
still below the 5% investigation threshold. Every focused repository is below 5%
raw delta, and all no-family-growth repositories are also below 5 ms absolute delta.

## Product-output check

The full pinned 120-repository corpus compared `f968dcbd` with `96c37985` using
`nose query <repo> all top=0 --mode semantic --format json`:

- 120/120 output hashes were identical;
- 120/120 byte counts were identical;
- 120/120 family counts were identical; and
- aggregate one-iteration runtime changed from 68,384.66 ms to 26,487.81 ms
  (-61.27%).

The focused fix also preserves the expected #804 parent-to-head product changes:
`fastlane` remains at 29 families rather than the parent's 28, while the fix output
is byte-identical to `f968dcbd` on all three focused repositories.

## Reproduction and artifacts

The machine-readable artifacts include the exact harness commands, source SHAs,
binary SHA-256 values, raw runs, medians, stage timings, family counts, byte counts,
and output hashes:

- [original focused regression](../bench/recall_loss/ruby-redefinition-runtime-regression-2026-07-10.v1.json)
- [15-iteration parent-versus-fix comparison](../bench/recall_loss/ruby-redefinition-runtime-focused-2026-07-10.v1.json)
- [15-iteration same-binary control](../bench/recall_loss/ruby-redefinition-runtime-same-binary-2026-07-10.v1.json)
- [all-120 product-query comparison](../bench/recall_loss/ruby-redefinition-query-regression-all-repos-2026-07-10.v1.json)

The broad artifact was checked with:

```sh
python3 scripts/check-query-regression.py \
  bench/recall_loss/ruby-redefinition-query-regression-all-repos-2026-07-10.v1.json \
  --max-runtime-delta-pct 5
```

It reported zero output-drift repositories and `runtime_status: within-threshold`.

Measurements ran on Darwin 25.5.0 arm64, model `Mac17,7`, 18 logical CPUs,
128 GiB RAM, Rust 1.96.0, and Python 3.14.5. The pinned repositories and prune
manifest in `bench/` define the corpus state.

## Verification

The focused semantic battery and implementation gates used for the fix are:

```sh
cargo test -p nose-cli option_boundaries -- --nocapture
cargo test -p nose-semantics
cargo clippy --all-targets --all-features -- -D warnings
python3 scripts/check-file-lengths.py
```

The broader Type-4, replay, and repository CI gates remain required before merge.
