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
- [#580 cycle log](../bench/recall_loss/issue-580-cycle.v1.json) records the
  Rust struct-expression surface slice: struct literals now carry exact-safe
  `SequenceSurface` proof, which closes the imported-member target-present
  follow-ups exposed by #578 while keeping raw sequences closed.
- [#582 cycle log](../bench/recall_loss/issue-582-cycle.v1.json) records the
  receiver-domain recovery slice: iterator-adapter result domains,
  dependency-backed literal binding domains, normalized binding proof-chain
  admission, and mutation-closed strict exact receiver use.
- [#567 phase 1 JS/TS constructor log](../bench/recall_loss/issue-567-phase1-js-ts-constructors.v1.json)
  records imported immutable provider snapshots for JS/TS `new Map(...)` and
  `new Set(...)`, reusing existing constructor `LibraryApi` proof across the
  import boundary.
- [#567 phase 2 collection factory log](../bench/recall_loss/issue-567-phase2-collection-factories.v1.json)
  records imported immutable provider snapshots for existing Python and Java
  collection factory contracts, reusing `LibraryApi` proof and exact-safe
  provider arguments across the import boundary.
- [#567 phase 3 import-snapshot census log](../bench/recall_loss/issue-567-phase3-import-snapshot-census.v1.json)
  records the reporting closeout: local recall-loss reports now expose
  successful snapshot counts plus unresolved binding-import miss reasons, so the
  next imported-value slice can be selected from corpus evidence.

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

The #580 recovery slice reduces overall exact-admission rejections from `735` to
`707` and the callee-identity bucket from `235` to `221`, again with
`false_merges == 0` and `canon_preservation_violations == 0`. It does not loosen
untagged `Seq`: Rust struct literals now lower as `Seq("rust_struct_expression")`
and must carry matching `SequenceSurface::RustStructExpression` evidence. That
surface is exact-tree-safe but is not a collection, map, membership receiver,
map-entry list, or imported literal proof. This closes the
`imported-member-target-present-call-contract-proof` primary surface (`2 -> 0`),
reduces member-call primary loss (`98 -> 93`), and removes many Rust struct
literal source-surface losses (`73 -> 52`). Newly exact-safe but too-small units
move to the explicit value-fingerprint floor bucket (`6 -> 13`).

After #580 and before the receiver-domain slice, the top `crates` buckets were:

| reason | count | next capability |
|---|---:|---|
| `receiver-domain-proof-missing` | 241 | receiver-domain evidence instead of selector spelling |
| `import-symbol-callee-identity-proof-missing` | 221 | reusable member/receiver callee identity evidence |
| `mutation-effect-boundary` | 131 | effect and place contracts |
| `source-surface-proof-missing` | 52 | Rust macro/source-surface contracts and construct/operator/comprehension evidence |
| `hof-demand-effect-proof-missing` | 28 | HOF demand/effect/materialization profile |
| `unsupported-runtime-boundary` | 14 | intentional fail-closed runtime/protocol boundary |

These are capability gaps, not feature requests. A future PR should close a
bucket by adding reusable evidence or an admission capability, not by adding a
one-off API exception.

The #582 receiver-domain recovery slice keeps the hard gates closed while adding
local infrastructure for iterator-adapter result domains and call-node
receiver-domain consumption. Rust `iter`/`into_iter`/`iter_mut`/`copied`/
`cloned` and Java `stream` now emit `Iterator` result-domain evidence; Rust
`to_vec` emits `Collection`; Rust `collect` remains closed because its result
type is caller-selected. Strict exact consumers now read asserted `Domain`
evidence anchored to call receivers, and typed `const`/`static`/`let` plus
literal assignments emit binding-domain evidence from existing
SequenceSurface/Domain proof. Canonical builtin admission now follows the
binding proof chain after normalization inlines the receiver value, while strict
exact still closes receiver-domain use when `ReceiverMutation` evidence appears
before the use. The local `crates` run moved `receiver-domain-proof-missing`
from `241` to `239`, with `false_merges == 0`,
`canon_preservation_violations == 0`, and completeness improving from `38/82`
to `39/83`. Total exact-admission rejections moved `707 -> 708`; the increase
lands in structured callee-identity/HOF/library-API occurrence buckets, not in
unattributed unsafe exact admission. The remaining receiver-domain cases still
point at cross-file field/constant domain provenance, not more
selector-specific iterator exceptions.

The #567 phase 1 JS/TS constructor slice keeps the hard gates closed while
opening imported immutable snapshots for provider-owned `new Map(...)` and
`new Set(...)`. This does not add new API shapes: provider export safety now
reuses the existing `JsLikeMapConstructor` and `JsLikeSetConstructor`
`LibraryApi` occurrence proofs, including construct syntax and unshadowed-global
callee obligations, before copying the provider evidence into the importer.
Focused product fixtures move JS/TS imported Map defaults from `0/2` to `2/2`
supported positives and JS/TS imported Set membership from `0/2` to `2/2`,
while missing constructor evidence, provider-local `Map`/`Set` shadows,
provider/importer mutation, wrong contents, and raw import-coordinate sequences
stay closed. The full `crates` recall-loss report remains at
`false_merges == 0` and `canon_preservation_violations == 0`; admission
rejections move `708 -> 710` because this PR adds new Rust test/helper units,
and the increase is attributed to the existing callee-identity bucket.

The #567 phase 2 collection-factory slice applies the same capability path to
existing collection factory contracts instead of adding selector exceptions.
Provider export safety now admits provider-owned collection-factory calls only
through already-admitted `LibraryApi` occurrence proof plus exact-safe literal
arguments. The product fixture delta is Python imported collection membership
`0/2 -> 2/2` for builtin `set([...])` and imported
`collections.deque([...])`, and Java imported collection membership
`0/2 -> 2/2` for static-imported `List.of(...)` and `Set.of(...)` provider
bindings. Missing `LibraryApi` proof, provider-local factory shadowing,
provider/importer mutation, wrong contents, and ambiguous single-argument
`Arrays.asList(...)` provider snapshots remain closed. The full `crates`
recall-loss report remains at `false_merges == 0` and
`canon_preservation_violations == 0`; admission rejections move `710 -> 711`
because this PR adds new test/helper units, and the increase is attributed to
the existing import-symbol callee-identity bucket.

The #567 phase 3 reporting closeout adds `import_snapshot_census` to local
recall-loss reports. This is reporting-only: it does not admit new snapshots or
change clone families. The full `crates` report remains at `false_merges == 0`
and `canon_preservation_violations == 0`; admission rejections move
`711 -> 716` because the reporting implementation and CLI fixture add new Rust
test/helper units. The new census shows that `crates` currently has `0`
successful imported snapshot records and `384` unresolved binding imports:
`provider-module-missing` `255`, `provider-export-missing` `123`,
`importer-binding-mutated` `3`, and
`provider-aggregate-children-not-exact-safe` `3`. That makes the next
imported-value decision explicit: most `crates` misses are module/export
resolution scope, while the provider-aggregate slice is the small actionable
semantic export-safety surface.

The current top `crates` buckets after #567 phase 3 are:

| reason | count | next capability |
|---|---:|---|
| `receiver-domain-proof-missing` | 240 | cross-file field/constant domain provenance |
| `import-symbol-callee-identity-proof-missing` | 226 | reusable member/receiver callee identity evidence |
| `mutation-effect-boundary` | 133 | effect and place contracts |
| `source-surface-proof-missing` | 52 | Rust macro/source-surface contracts and construct/operator/comprehension evidence |
| `hof-demand-effect-proof-missing` | 30 | HOF demand/effect/materialization profile |
| `unsupported-runtime-boundary` | 14 | intentional fail-closed runtime/protocol boundary |
| `value-fingerprint-too-small` | 13 | explicit low-substance floor policy |
| `library-api-occurrence-proof-missing` | 8 | missing occurrence evidence, not selector spelling |

## See Also

- [recall-loss-diagnostics](recall-loss-diagnostics.md)
- [semantic-pack-architecture](semantic-pack-architecture.md)
- [source-facts](source-facts.md)
- [evidence-records](evidence-records.md)
- [demand-effect-semantics](demand-effect-semantics.md)
