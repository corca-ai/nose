# Recall-loss recovery loop

The recall-loss recovery loop turns `nose verify --recall-loss-report` from a
one-off artifact into a semantic-kernel process. The goal is to make exact
semantic admission stricter or equally strict while reducing unattributed recall
loss. When recall cannot be recovered safely, the loop records the missing
capability or the intentional unsupported boundary.

## Baselines

Checked-in summaries live under [bench/recall_loss](../bench/recall_loss/):

- [crates baseline](../bench/recall_loss/crates.baseline.v1.json) records the
  current `crates` surface.
- [corpus-slice baseline](../bench/recall_loss/corpus-slice.baseline.v1.json)
  records a small mixed-language slice across Go, Python, Ruby, TypeScript,
  Rust, and Swift.
- [#570 cycle log](../bench/recall_loss/issue-570-cycles.v1.json) records the
  first five top-bucket cycles and the unsupported runtime boundary decision.
- [#572 cycle log](../bench/recall_loss/issue-572-cycle.v1.json) records the
  first post-#570 refinement cycle, which splits expression-statement effect
  boundaries and Rust macro source surfaces out of the callee-identity bucket.
- [#574 callee census](../bench/recall_loss/issue-574-callee-census.v1.json)
  records the remaining callee-identity bucket by language and call-target
  surface for the #567 import-backed immutable provenance epic.
- [#576 cycle log](../bench/recall_loss/issue-576-cycle.v1.json) records the
  first recovery slice after the census: Rust brace `use` declarations now emit
  per-item imported symbol evidence that feeds the existing imported
  call-target producer.
- [#578 cycle log](../bench/recall_loss/issue-578-cycle.v1.json) records the
  next Rust scoped-path recovery slice: scoped calls whose root already has
  dependency-backed import evidence now emit imported member call-target proof.

Regenerate the full local reports with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.crates.json

cargo run -q -p nose-cli -- verify \
  bench/repos/chi/middleware/content_type.go \
  bench/repos/boltons/boltons/iterutils.py \
  bench/repos/thor/lib/thor/actions.rb \
  bench/repos/radash/src/array.ts \
  bench/repos/hyperfine/src/util/number.rs \
  bench/repos/swift-metrics/Sources/CoreMetrics/Metrics.swift \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.corpus-slice.json
```

Compare two reports with:

```sh
python3 scripts/recall-loss-diff.py before.json after.json
```

## Cycle Contract

Each semantic-kernel cycle records:

- the baseline report and selected reason bucket;
- representative fixture or linked existing fixture;
- whether the result is recovered, classified actionable, precision-hardened, or
  intentionally unsupported;
- before/after hard gate numbers;
- before/after recall-loss bucket numbers;
- docs and changelog updates.

The hard gate is not negotiable:

- `false_merges == 0`;
- `canon_preservation_violations == 0`.

The soft gate is attribution quality. A stricter admission change may increase
rejections, but the increase must land in a structured bucket with a named
capability, fixture, and follow-up policy.

## #570 Starting Result

The first coarse `crates` baseline had `758` units in the opaque
`strict-exact-unsafe` bucket. The #570 attribution pass reduced
`unattributed-strict-exact-unsafe` to `0` while preserving false merges `0` and
canon-preservation violations `0`.

The #572 refinement keeps the same hard gate while moving expression-statement
effect boundaries and unmodeled Rust macro invocations out of the
callee-identity bucket. That sharpens the remaining exact-recovery target: pure
scoped/path callees still need reusable symbol/callee evidence, while discarded
call results and unmodeled macro expansion stay closed.

The #574 census keeps the `import-symbol-callee-identity-proof-missing` count at
`264` but makes the inside of that bucket actionable. On `crates`, the remaining
units are overwhelmingly Rust (`261/264`). The largest call-target surfaces are
local-or-parameter calls (`115`), member calls (`92`), and scoped-path calls
(`45`). That points the next implementation slice at Rust local/scoped path
call-target proof before expanding the same evidence shape into broader
import-backed immutable value provenance under #567.

The #576 recovery slice reduces the callee-identity bucket from `264` to `251`
without changing the hard gate (`false_merges == 0`,
`canon_preservation_violations == 0`). It does this by proving Rust brace import
bindings such as `use crate::m::{f, T};` as per-item `Import`/`Symbol` evidence
while leaving wildcard imports, nested brace imports, and `self`/`super`-relative
brace prefixes closed. This shrinks the local-or-parameter primary surface from
`115` to `71`; the next dominant targets are scoped paths and member calls.

The #578 recovery slice reduces the callee-identity bucket from `251` to `235`
while preserving the same hard gate. It proves only import-backed Rust scoped
calls: a lowered scoped callee such as `Span::new` can emit
`CallTarget::ImportedMember` when `Span` has a unique static imported binding or
namespace proof. Raw `crate::...`, `self::...`, `super::...`,
`std/core/alloc::...`, unimported roots, and ambiguous roots remain closed. The
scoped-path primary surface drops from `72` to `50`; the remaining dominant
surface is now member/receiver call-target proof, with two newly exposed
`imported-member-target-present-call-contract-proof` follow-ups.

The current top `crates` buckets are:

| reason | count | next capability |
|---|---:|---|
| `receiver-domain-proof-missing` | 240 | receiver-domain evidence instead of selector spelling |
| `import-symbol-callee-identity-proof-missing` | 235 | reusable member/receiver callee identity evidence |
| `mutation-effect-boundary` | 132 | effect and place contracts |
| `source-surface-proof-missing` | 73 | Rust macro/source-surface contracts and construct/operator/comprehension evidence |
| `hof-demand-effect-proof-missing` | 28 | HOF demand/effect/materialization profile |
| `unsupported-runtime-boundary` | 14 | intentional fail-closed runtime/protocol boundary |

These are capability gaps, not feature requests. A future PR should close a
bucket by adding reusable evidence or an admission capability, not by adding a
one-off API exception.

## See Also

- [recall-loss-diagnostics](recall-loss-diagnostics.md)
- [semantic-pack-architecture](semantic-pack-architecture.md)
- [source-facts](source-facts.md)
- [evidence-records](evidence-records.md)
- [demand-effect-semantics](demand-effect-semantics.md)
