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
- [corpus-slice baseline](../bench/recall_loss/corpus-slice.baseline.v1.json) records a small mixed-language slice across Go, Python, Ruby, TypeScript,
  Rust, and Swift.
- [#570 cycle log](../bench/recall_loss/issue-570-cycles.v1.json) records the
  first five top-bucket cycles and the unsupported runtime boundary decision.
- [#572 cycle log](../bench/recall_loss/issue-572-cycle.v1.json) records the
  first post-#570 refinement cycle, which splits expression-statement effect
  boundaries and Rust macro source surfaces out of the callee-identity bucket.
- [#574 callee census](../bench/recall_loss/issue-574-callee-census.v1.json) records the remaining callee-identity bucket by language and call-target
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
- [#567 phase 1 JS/TS constructor log](../bench/recall_loss/issue-567-phase1-js-ts-constructors.v1.json) records imported immutable provider snapshots for JS/TS `new Map(...)` and
  `new Set(...)`, reusing existing constructor `LibraryApi` proof across the
  import boundary.
- [#567 phase 2 collection factory log](../bench/recall_loss/issue-567-phase2-collection-factories.v1.json) records imported immutable provider snapshots for existing Python and Java
  collection factory contracts, reusing `LibraryApi` proof and exact-safe
  provider arguments across the import boundary.
- [#567 phase 3 import-snapshot census log](../bench/recall_loss/issue-567-phase3-import-snapshot-census.v1.json) records the reporting closeout: local recall-loss reports now expose
  successful snapshot counts plus unresolved binding-import miss reasons, so the
  next imported-value slice can be selected from corpus evidence.
- [#567 phase 4 aggregate-boundary triage log](../bench/recall_loss/issue-567-phase4-aggregate-boundary-triage.v1.json) records the first census-driven triage pass: the broad provider-aggregate miss
  bucket is split into non-import-literal sequence surfaces and child reference
  boundaries without admitting new snapshots.
- [#567 closeout log](../bench/recall_loss/issue-567-closeout.v1.json) records the epic-level audit: requirement coverage, exact-safe imported
  coordinate families, hard-negative inventory, hard-gate status, and runtime
  measurements. The narrative closeout is [import-backed immutable provenance closeout](import-backed-immutable-provenance-closeout-567.md).
- [#587 module/export census](../bench/recall_loss/issue-587-module-export-census.v1.json) records the starting point for the follow-up import-snapshot milestone:
  provider module/export miss counts by reason, crate, import surface, top
  files, and recommended implementation order.
- [post-#587 census](../bench/recall_loss/post-587-census.v1.json) records the
  current `crates` recall-loss shape after the #587 closeout: generic
  provider-module misses are gone on the checked surface, and the next
  capability targets are receiver-domain proof, callee identity, and
  mutation/effect contracts.
- [full corpus priority census](../bench/recall_loss/corpus-priority-census-2026-06-28.v1.json) records the first 120-repo follow-up: it combines per-repo recall-loss reports
  with lexical stdlib/API source prevalence so the next semantic-kernel work is
  selected from the pinned corpus instead of from `crates` alone.
- [#594 cross-language boundary census](../bench/recall_loss/cross-language-boundary-census-594-2026-06-28.v1.json) records the starting inventory for scheduling,
  success/error channel, callback demand/effect, lifecycle/materialization,
  receiver-mutation, and ambiguous-selector obligations across JS/TS, Python,
  Rust, Go, Java, Swift, Ruby, and C. It is reporting-only and records
  `semantic_admission_delta = 0`.
- [#597 obligation taxonomy report](../bench/recall_loss/issue-597-obligation-taxonomy-2026-06-28.v1.json) records the first `crates` recall-loss run with diagnostics-only
  obligation families and subreasons attached to admission rejections.
- [#598 hard-negative inventory](../bench/recall_loss/issue-598-hard-negative-inventory-2026-06-28.v1.json) maps existing cross-language tests and pricing reports to #594 hard-negative families before producer work.
- [#599 callback obligation slice](../bench/recall_loss/issue-599-callback-obligation-slice-2026-06-28.v1.json) records callback demand/effect reporting coverage without new exact admission.
- [#600 channel/scheduling obligation slice](../bench/recall_loss/issue-600-channel-scheduling-obligation-slice-2026-06-28.v1.json) records channel and scheduling-boundary reporting coverage while broad async/channel convergence remains closed.
- [Promise protocol diagnostics](../bench/recall_loss/promise-protocol-diagnostics-2026-06-28.v1.json) records the Promise/async follow-up split: await scheduling, async function scheduling, Promise executor callbacks, factories, aggregate result channels, rejection channels, and non-construct calls now have report labels while exact admission remains closed.
- [#601 first-slice closeout](../bench/recall_loss/issue-601-first-slice-closeout-2026-06-28.v1.json) records the quantified decision to keep the first exact #594 slice closed rather than force unsafe async/callback/channel admission.
- [#594 closeout](../bench/recall_loss/issue-594-closeout-2026-06-28.v1.json) records the epic-level artifact coverage, validation commands, quantitative summary, and remaining closed boundaries.
- [callback demand/effect diagnostics v3](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v3.json) records the post-#594 callback-call refinement: member call proof, Rust macro call proof, and direct/imported function effect-contract buckets are now visible without opening exact admission.
- [Promise receiver-producer diagnostics](../bench/recall_loss/promise-receiver-producer-diagnostics-2026-06-29.v1.json) records the reporting-only follow-up after local Promise continuation recovery: `.then`/`.catch`/`.finally` receiver producers now split into constructor, async-function-return, and generic call-return obligations while exact admission remains closed.
- [Promise call-return callee diagnostics](../bench/recall_loss/promise-call-return-callee-diagnostics-2026-06-29.v1.json) records the next reporting-only split inside generic call-return receivers: member, local/parameter, imported binding/member, known-target return-domain, and unknown callee shapes now have separate missing-evidence labels while exact admission remains closed.
- [Promise direct-function return recovery](../bench/recall_loss/promise-direct-function-return-recovery-2026-06-29.v1.json) records the proof-backed direct-function subset of the local/parameter call-return queue: same-file single-return functions returning proven PromiseLike expressions can now feed local Promise continuation recovery while parameter callees and member/imported call returns remain closed.
- [Promise direct-method return recovery](../bench/recall_loss/promise-direct-method-return-recovery-2026-06-29.v1.json) records the proof-backed DirectMethod subset of the member call-return queue: existing DirectMethod target evidence plus returned-expression PromiseLike domain proof can feed local Promise continuation recovery while selector-only member calls, receiver-dependent methods, dynamic dispatch, and imported members remain closed.
- [Promise imported call-return boundary](../bench/recall_loss/promise-imported-call-return-boundary-2026-06-29.v1.json) records the reporting-only imported function/member follow-up: target-present imported Promise receivers now report missing settled-value contracts instead of return-domain proof, because imported call-target identity has no local body to evaluate.
- [Promise branch-return producer recovery](../bench/recall_loss/promise-branch-return-producer-recovery-2026-06-29.v1.json) records the branch-return extension for local Promise producers: DirectFunction and DirectMethod return-domain proof can now compose every returned expression on supported paths, while mixed settlement channels, selector-only members, parameter callees, and imported receivers remain closed.
- [Promise finally settlement recovery](../bench/recall_loss/promise-finally-settlement-recovery-2026-06-29.v1.json) records the safe `.finally` continuation slice: admitted PromiseLike receivers plus absent or zero-argument non-thenable-safe handlers can preserve settlement, while rejecting finally handlers switch to the rejected channel and unsafe handlers remain closed.
- [Promise imported settled-value contract](../bench/recall_loss/promise-imported-settled-value-contract-2026-06-29.v1.json) records the reusable settled-value evidence capability for imported Promise producers: imported target identity, PromiseLike receiver proof, admitted continuation API evidence, and `PromiseSettledValue` payload/channel proof can now recover focused imported `.then`/`.catch` fixtures while ordinary imported producers without that contract remain closed.
- [Node timers Promise domain recovery](../bench/recall_loss/promise-node-timers-domain-recovery-2026-06-29.v1.json) records the ESM named-import domain-only Node `timers/promises` slice: imported `setTimeout`/`setImmediate` calls can now provide `Domain(PromiseLike)` receiver proof without settlement or payload recovery.
- [Node timers CommonJS domain recovery](../bench/recall_loss/promise-node-timers-commonjs-domain-recovery-2026-06-29.v1.json) records the follow-up that opens conservative `const` CommonJS destructuring requires for the same domain-only proof, moving the priced Node timers call-site coverage from `82` to `97` while mutable/dynamic shapes and scheduling semantics remain closed.
- [Node timers safe payload recovery](../bench/recall_loss/promise-node-timers-safe-payload-recovery-2026-06-29.v1.json) records the bounded payload follow-up: exactly `setTimeout(delay, value)` and `setImmediate(value)` now emit fulfilled `PromiseSettledValue` evidence, while option-bearing calls, possible thenable payloads, scheduler APIs, and interval streams remain closed. The current pinned corpus has `0` direct safe-payload call sites, so this is a capability and fixture gain rather than an immediate corpus recall delta.
- [Promise/scheduling closeout](../bench/recall_loss/promise-scheduling-closeout-2026-06-29.v1.json) records the decision to stop this recovery cycle after local Promise producer recovery, imported settled-value evidence, and Node timers slices. Aggregate combinators, executor timing, cancellation/liveness, scheduler APIs, interval streams, and cross-language lifecycle models move to issue [#602](https://github.com/corca-ai/nose/issues/602).
- [#602 scheduling/lifecycle boundary audit](../bench/recall_loss/scheduling-lifecycle-boundary-audit-602-2026-06-29.v1.json) records the first reporting-only #602 slice across the 120-repo corpus. It prices `142,844` source-prevalence occurrences and ranks Promise aggregates, executor timing, AbortSignal cancellation, interval lifecycle, Go goroutines, Java `CompletableFuture`, and Swift `await` as the first actionable reporting targets.
- [#602 Promise.all literal aggregate recovery](../bench/recall_loss/promise-all-literal-aggregate-recovery-2026-06-29.v1.json) records the first exact aggregate slice. It opens fulfilled-only `Promise.all` over literal arrays whose elements already recover as fulfilled Promise evidence; dynamic iterables, rejected inputs, `race`/`any`, thenables, executor timing, and sync arrays stay closed.
- [#602 Promise.allSettled literal aggregate recovery](../bench/recall_loss/promise-allsettled-literal-aggregate-recovery-2026-06-29.v1.json) records the next exact aggregate slice. It opens fulfilled-result `Promise.allSettled` over literal arrays whose elements already recover as fulfilled or rejected Promise evidence; dynamic iterables, `all`/`race`/`any`, thenables, executor timing, and sync settled-record arrays stay closed.
- [#602 Promise aggregate raw-input assimilation](../bench/recall_loss/promise-aggregate-raw-input-recovery-2026-06-29.v1.json) records the shared exact slice for literal `Promise.all` and `Promise.allSettled` raw primitive inputs. It reuses the existing non-thenable-safe proof from `Promise.resolve`; at that slice, object/function raw inputs, untyped possible thenables, dynamic iterables, `race`/`any`, executor timing, and sync aggregate equivalence stayed closed.
- [#602 Promise.race/Promise.any literal aggregate recovery](../bench/recall_loss/promise-race-any-literal-aggregate-recovery-2026-06-30.v1.json) records the first-observed exact aggregate slice. It opens only non-empty fully closed literal `Promise.race` and fully closed literal `Promise.any` with a fulfilled candidate; dynamic iterables, possible thenables, all-rejected `Promise.any` AggregateError payloads, executor timing, and sync value equivalence stay closed.
- [#602 Promise executor boundary audit](../bench/recall_loss/promise-executor-boundary-audit-2026-06-30.v1.json) records the reporting-only `new Promise(...)` readiness slice. It splits `795` constructor occurrences by inline executor shape, settlement calls, timer/scheduler use, multi-settlement, throw-to-rejection, and possible thenable payload risk; exact constructor admission remains closed with `semantic_admission_delta = 0`.
- [#602 AbortSignal cancellation boundary audit](../bench/recall_loss/abort-signal-cancellation-boundary-audit-2026-06-30.v1.json) records the reporting-only cancellation/liveness readiness slice. It splits `260` Abort mentions, `193` signal option properties, controller lifecycle pairs, direct static AbortSignal calls, and signal-aware fetch/timer/listener use; exact cancellation admission remains closed with `semantic_admission_delta = 0`.
- [#602 interval/scheduler lifecycle boundary audit](../bench/recall_loss/interval-scheduler-lifecycle-boundary-audit-2026-06-30.v1.json) records the reporting-only timer, scheduler, interval, and microtask readiness slice. It splits `780` `setTimeout`, `73` bare `setInterval`, `55` `clearInterval`, `133` `clearTimeout`, `14` `queueMicrotask`, `43` `requestAnimationFrame`, and `11` `scheduler.yield` occurrences; exact scheduling/lifecycle admission remains closed with `semantic_admission_delta = 0`.
- [#602 closeout](../bench/recall_loss/issue-602-closeout-2026-06-30.v1.json) records the epic-level completion audit. Every opened exact aggregate slice has a checked artifact, reporting-only executor/cancellation/scheduling/lifecycle slices keep `semantic_admission_delta = 0`, local `crates` gate remains at `false_merges = 0` and `canon_preservation_violations = 0`, and remaining broad surfaces are named closed obligations.
- [Cross-language await obligation reporting](../bench/recall_loss/cross-language-await-obligation-reporting-2026-06-30.v1.json) records the post-#602 reporting-only label migration: JS/TS, Python, Rust, and Swift `Source::Protocol(Await)` boundaries now report `async-await-scheduling-contract` instead of the old Promise-specific await label, with `semantic_admission_delta = 0`.

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

Regenerate the full corpus priority census with:

```sh
python3 scripts/corpus-priority-census.py \
  --jobs 4 \
  --logs-dir target/corpus-priority-census-full \
  --output target/corpus-priority-census-full.json
```

The census has two signals: `recall_loss` is oracle/strict-admission evidence,
while `source_scan` is lexical source prevalence for pricing stdlib/API
surfaces. Source prevalence never admits semantics by itself.

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
resolution scope, while the provider-aggregate slice is the small triage target.

The #567 phase 4 aggregate-boundary triage follows that target and keeps
imported snapshot admission unchanged. The broad
`provider-aggregate-children-not-exact-safe` bucket moves `3 -> 0`: two cases
are Rust `pub use context::...` re-export paths reported as
`provider-sequence-surface-not-import-literal-safe`, and one case is the
compiled semantic-pack descriptor table assembled from indexed descriptor
references, reported as `provider-aggregate-child-reference-boundary`. The full
`crates` report remains at `false_merges == 0` and
`canon_preservation_violations == 0`; completeness stays `39/83`, and
admission rejections move `716 -> 717` because this diagnostics-only pass adds
new Rust semantic tests. The decision is to keep these closed: admitting them as
snapshots would treat references as literal provider values.

The current top `crates` buckets after #567 phase 4 are:

| reason | count | next capability |
|---|---:|---|
| `receiver-domain-proof-missing` | 240 | cross-file field/constant domain provenance |
| `import-symbol-callee-identity-proof-missing` | 227 | reusable member/receiver callee identity evidence |
| `mutation-effect-boundary` | 133 | effect and place contracts |
| `source-surface-proof-missing` | 52 | Rust macro/source-surface contracts and construct/operator/comprehension evidence |
| `hof-demand-effect-proof-missing` | 30 | HOF demand/effect/materialization profile |
| `unsupported-runtime-boundary` | 14 | intentional fail-closed runtime/protocol boundary |
| `value-fingerprint-too-small` | 13 | explicit low-substance floor policy |
| `library-api-occurrence-proof-missing` | 8 | missing occurrence evidence, not selector spelling |

The #567 closeout keeps that phase-4 decision intact. The epic is complete as an
imported immutable value capability: product fixtures now cover the supported
map-default, membership, and string-affix coordinate families; hard negatives
remain closed; and import-snapshot misses are measurable. The follow-up is not
to relax aggregate child export safety. The remaining large census buckets are
module/export resolution scope and should be planned as a separate milestone if
import snapshots remain the priority.

Issue #587 is that separate milestone. Its starting census selects the
`provider-module-missing` and `provider-export-missing` rows from the #567
closeout report: `378` rows, all Rust. The largest clearly same-repo first slice
is `provider-export-missing` on `crate::...` imports (`68` rows). Before opening
that slice, split unsupported stdlib, external crate, and workspace-crate imports
out of the actionable module-resolution bucket so package semantics stay closed.

The #587 initial module-resolution slice applies that split and opens only the
literal-safe part of same-repo Rust module lookup. Rust file identity now treats
`src/lib.rs`, `src/main.rs`, and `mod.rs` as crate/module owners, and imported
snapshot lookup derives `self::...`/`super::...` aliases from the importer and
provider file identities before accepting a provider-owned immutable literal.
Non-value exports stay closed but are no longer mixed into generic miss buckets:
callables, type exports, module namespaces, Rust stdlib imports, and workspace
crate imports now have separate census reasons. On `crates`, the generic
module/export target moved `378 -> 139` (`provider-module-missing` `255 -> 130`,
`provider-export-missing` `123 -> 9`) while successful imported snapshot records
move `0 -> 1`; hard gates remain at `false_merges == 0` and
`canon_preservation_violations == 0`. The checked-in measurement is [`issue-587-module-resolution-1-3.v1.json`](../bench/recall_loss/issue-587-module-resolution-1-3.v1.json).

The #587 direct re-export slice adds proof for public Rust `pub use` bindings
without treating re-export syntax as value proof. The lowerer now emits
first-party `ReExportBinding` evidence for direct public `use` declarations,
and corpus import resolution follows one same-corpus re-export hop only when the
target is already a unique literal-safe provider export. Private `use`,
wildcard/nested brace forms, ambiguous re-exports, and non-value targets remain
closed. The same slice also recognizes same-crate bare child module aliases such
as `context::Item` from a parent module file. On `crates`, existing re-exports
mostly point at types and callables rather than literal provider values, so
successful imported snapshot records stay `1`; the generic module/export target
rows move `139 -> 91`, `provider-module-missing` moves `130 -> 89`, and
`provider-export-missing` moves `9 -> 2`, with direct re-export targets priced
as `provider-reexport-*` boundary reasons. Hard gates remain at
`false_merges == 0` and `canon_preservation_violations == 0`. The checked-in
measurement is [`issue-587-reexport-pricing.v1.json`](../bench/recall_loss/issue-587-reexport-pricing.v1.json).

The #587 residual census checks whether another same-repo provider-resolution
slice is warranted before widening implementation. After the re-export slice
and diagnostics module split, the remaining generic module/export target is
`92` rows, or `93` if the re-export target-export tail is included. Most of
that is not same-repo module resolution: `76` rows are external crate imports
(`rustc_hash`, `tree_sitter`, `anyhow`, `serde`, `regex`, `clap`, `ignore`),
`1` is a residual workspace-crate boundary gap, and `2` are residual
`std::cell` rows. The same-repo tail is much smaller: `11` relative-`super`
rows and `2` local export misses. The next
implementation slice should therefore split external/std/workspace residuals
out of `provider-module-missing` as explicit closed boundaries before deciding
whether the relative-`super` tail is worth opening. The checked-in measurement
is [`issue-587-residual-census.v1.json`](../bench/recall_loss/issue-587-residual-census.v1.json).

The #587 residual boundary split implements that diagnostics-only step. Known
external crate imports now report as `provider-external-crate-boundary`,
residual `std::cell` imports join `provider-rust-stdlib-boundary`, and the
remaining `nose_il::UnitKind` type-namespace import joins
`provider-workspace-crate-boundary`. This does not admit new snapshot values;
it only prices closed boundaries more accurately. On `crates`,
`provider-module-missing` moves `90 -> 11`, generic module/export target rows
move `92 -> 13`, and the residual set including the re-export target-export tail
moves `93 -> 14`. Successful imported snapshot records stay `1`; hard gates
remain at `false_merges == 0` and `canon_preservation_violations == 0`. The
checked-in measurement is [`issue-587-residual-boundary-split.v1.json`](../bench/recall_loss/issue-587-residual-boundary-split.v1.json).

The #587 relative-`super` closeout handles the last generic Rust
`provider-module-missing` rows. Importer-relative provider lookup already knew
how to build aliases such as `super::child`; the missing case was when the
import names the parent module itself, such as `use super::Item` or
`use super::super::Item`. That alias now resolves to the same-crate parent or
grandparent module provider. On `crates`, `provider-module-missing` moves
`11 -> 0`, generic module/export target rows move `13 -> 2`, and the residual
set including the re-export target-export tail moves `14 -> 3`. The moved rows
remain closed as non-value boundaries (`provider-callable-export-boundary`,
`provider-type-export-boundary`, or `provider-reexport-type-boundary`) rather
than becoming snapshot values; successful imported snapshot records stay `1`,
with `false_merges == 0` and `canon_preservation_violations == 0`. The remaining
tail is export-only (`2` local export misses and `1` re-export target export
miss), so #587's module-missing work is complete. The checked-in measurement is [`issue-587-relative-super-closeout.v1.json`](../bench/recall_loss/issue-587-relative-super-closeout.v1.json).

The post-#587 census confirms that import-snapshot module/provider-missing work
is no longer the leading recall-loss surface on `crates`. The current checked
run has `0` false merges and `0` canon-preservation violations, `726`
structured exact-admission rejections, and `39/83` behavior-equal pairs
converged by exact fingerprints. The largest buckets are
`receiver-domain-proof-missing` (`244`), `import-symbol-callee-identity-proof-missing`
(`231`), and `mutation-effect-boundary` (`134`). The import snapshot census now
has `1` successful Rust snapshot and `388` unresolved binding imports, but the
remaining rows are explicit closed boundaries: callable exports (`110`),
external crates (`76`), type exports (`69`), Rust stdlib (`50`), workspace
crates (`48`), module namespaces (`20`), small re-export tails, mutation, and
two local export misses. The next milestone should therefore move to reusable
receiver-domain or member-call identity proof before more import-snapshot
resolution work. The checked-in measurement is [`post-587-census.v1.json`](../bench/recall_loss/post-587-census.v1.json).

The 2026-06-28 full corpus priority census broadens that view from `crates` to
all 120 pinned repos. The hard gate remains closed (`false_merges == 0` and
`canon_preservation_violations == 0`), but the leading recall-loss buckets are
different at product scale: mutation/effect contracts (`71,884`), callee
identity (`50,322`), unsupported runtime boundaries (`20,128`), and value
fingerprint floor (`16,006`). The first full-corpus run also exposes a process
gap that `crates` did not show: `unattributed-strict-exact-unsafe` is `1,896`,
mostly Python (`1,429`), so future cycles must continue reducing that bucket
while widening exact admission.

The same census adds a separate stdlib/API source-prevalence scan. Raw
prevalence is led by C string/memory and allocation calls, but those are
high-risk pointer/effect/lifetime surfaces. The safer initial semantic-kernel
order is therefore: Go `strings` transforms, Java `Optional`, Java
Arrays/Collections partial-coverage audit, Go `sort`/`slices`/`maps`, and
Python HOF/runtime attribution before widening `itertools`/`functools`. This is
still a pricing result, not semantic proof; every slice must add fixtures,
before/after recall-loss counts, and the same hard gate evidence. The checked
summary is [`corpus-priority-census-2026-06-28.v1.json`](../bench/recall_loss/corpus-priority-census-2026-06-28.v1.json).

The first concrete slice from that order admits Go `strings.Join` only through
imported namespace proof and reuses the existing ordered `Join` builtin instead
of adding a narrower feature-specific semantic. Trim, split, replace, and case
transforms remain closed until they have equivalent value semantics and hard
negative fixtures.

The Java `Optional` slice starts with fully-qualified `java.util.Optional<T>`
receiver proof for `isPresent()` and `orElse(default)`. Bare imported
`Optional<T>` remains closed deliberately; the next capability needed there is
import-backed Java type-domain proof, not another one-off Optional feature row.

The Java `Arrays`/`Collections` audit scans all pinned Java repos and classifies
`7,265` lexical method calls: `4,608` are already supported or partially
supported, `2,657` remain unsupported, and only `2` stay as lexical
false-positive/unknown boundaries. The top unsupported capability buckets are
copy-result domain proof (`586`), mutation/effect contracts (`886`), array
content equality (`329`), representation strings (`264`), and wrapper aliasing
(`266`).

The Go `sort`/`slices`/`maps` audit resolves simple import aliases and
classifies `1,339` corpus calls: `121` `slices.Contains` calls are already
supported, `1,218` remain unsupported, and no observed method is left
unclassified. The leading buckets are mutation/effect (`600`),
mutation+callback (`341`), copy-result domain (`82`), ordering preconditions
(`60`), and collection equality (`33`).

The JS/TS builtin partial-coverage audit masks comments/strings and scans all
pinned JavaScript/TypeScript repos, including `.js`, `.ts`, JSX/TSX, and
embedded Vue/Svelte script surfaces. It classifies `42,619` builtin-shaped
occurrences: `225` are supported, `10,558` are supported-partial, `31,836` are
unsupported, and no observed classified row is left unknown. The leading
next-work groups are Promise async/scheduling boundaries (`29,094`), mutation
and effect contracts (`3,053`), cardinality receiver proof (`3,007`), Map/Set
receiver-domain proof (`1,982`), and Array HOF receiver/callback proof
(`1,279`). The only 5,000+ group is processed as a closed boundary:
`await`, async functions, Promise combinators, `new Promise`, `catch`/`finally`,
and unsupported thenables stay closed until scheduling, exception, aggregate
result, rejection-channel, and callback-effect obligations are modeled. The
checked summary report is [`js-ts-stdlib-partial-audit-2026-06-28.v1.json`](../bench/recall_loss/js-ts-stdlib-partial-audit-2026-06-28.v1.json).

The Python HOF/runtime audit now has a v3 decision report that parses broader
AST scope bindings, function decorators/defaults, and call shapes. It classifies
`21,384` calls across builtins, `itertools`, and `functools`: `18,369` are
supported or partially supported, `2,947` are unsupported, `68` need stricter
runtime attribution because a bare builtin name is lexically shadowed, and no
observed boundary is left unknown. The ranked next-work groups are materializer
domain proof (`8,432`), ordering/key/comparator semantics (`2,182`), HOF
callback and iterable-source proof (`668`), `itertools` lifecycle/view contracts
(`242`), callable/decorator runtime identity (`225`), combinatoric iterator
shape contracts (`178`), callback reduction (`120`), and runtime attribution
(`68`). The v3 report processes the `8,432` materializer-domain occurrences as
a boundary split: `list`/`set`/`tuple`/`frozenset` stay gated by existing
LibraryApi occurrence, unshadowed builtin proof, source-iterator provenance,
and result-domain proof. The checked summary report is [`python-hof-runtime-audit-2026-06-28.v3.json`](../bench/recall_loss/python-hof-runtime-audit-2026-06-28.v3.json).

The Rust stdlib partial-coverage audit masks comments/strings and classifies
`104,133` Rust stdlib-shaped operations across `Option`, `Result`, iterator
adapters/HOFs, `Vec`, membership/map lookup, `std::collections` factories,
mutation, ordering, and allocation surfaces. Already admitted constructors and
`Vec` factories account for `65,514` occurrences, supported-partial rows account
for `34,670`, unsupported rows account for `3,949`, and no classified row is
left unknown. The v2 report processes five 5,000+ groups covering `32,361`
occurrences: iterator HOF callback proof, mutation/effect contracts, iterator
adapter/result-domain proof, Option/Result channel proof, and iterator lifecycle
and shape contracts. All keep `semantic_admission_delta = 0`; mutation/effect
contracts become stricter by adding missing Rust `sort_by_key` receiver-mutation
evidence. The largest remaining next-work group is receiver-domain proof
(`4,822`), followed by ordering, factory-domain, allocation/lifetime, and
reduction contracts. The checked summary report is [`rust-stdlib-partial-audit-2026-06-28.v2.json`](../bench/recall_loss/rust-stdlib-partial-audit-2026-06-28.v2.json).

The Swift stdlib partial-coverage audit masks comments/strings and classifies
`17,754` Swift stdlib-shaped operations across cardinality properties,
collection/map factories, sequence HOFs, membership, sequence views, mutation,
ordering, and reductions. Supported-partial rows account for `11,083`
occurrences, unsupported rows account for `6,671`, and no observed classified
row is left unknown. The leading next-work groups are cardinality receiver proof
(`7,633`), mutation/effect contracts (`4,382`), membership receiver proof
(`1,302`), collection/map factory domain proof (`1,289`), sequence view
lifecycle contracts (`1,218`), sequence HOF receiver/callback proof (`859`),
ordering semantics (`585`), reductions (`324`), and `Array(repeating:count:)`
repeat-factory shape contracts (`162`). The v2 report processes the `7,633`
cardinality receiver-proof occurrences as an existing-contract group:
`count`/`isEmpty` stay gated by property/method occurrence evidence and
ExactCollection receiver proof, with selector-only uses still closed. The
checked summary report is [`swift-stdlib-partial-audit-2026-06-28.v2.json`](../bench/recall_loss/swift-stdlib-partial-audit-2026-06-28.v2.json).

The 5,000+ follow-up pass processes seven high-volume groups across the Python,
Rust, and Swift reports, covering `48,426` occurrences. The important outcome is
not broader admission by frequency: every processed group records
`semantic_admission_delta = 0`, six groups keep strictness unchanged, and the
Rust mutation/effect group is stricter because `sort_by_key` now marks receiver
mutation before later exact receiver use.

With the JS/TS audit included, the processed 5,000+ stdlib/builtin audit set is
eight groups covering `77,520` occurrences. The added JS/TS group also records
`semantic_admission_delta = 0`: it makes the largest Promise/async surface
visible for future recall-loss reporting without opening async equivalence.

The #594 cross-language boundary census turns those language-specific audits
into one obligation matrix and adds conservative Ruby/C pricing. It records
`207,689` boundary-shaped occurrences across the eight pinned primary-language
groups. The leading families are success/error result channels (`62,945`),
lifecycle/materialization boundaries (`42,010`), scheduling boundaries
(`28,751`), ambiguous selector boundaries (`19,116`), receiver mutation
(`16,530`), and combined callback families (`18,637`). This does not open
semantic admission; it selects the next work as vocabulary/reporting, hard
negatives, then callback/channel producer evidence before any narrow exact
slice. The design vocabulary is [scheduling-channel-callback-obligations-594](scheduling-channel-callback-obligations-594.md).
Local `--recall-loss-report` artifacts now also include `by_obligation` and
per-rejection `obligation_family`/`obligation_subreason` fields, so broad reason
buckets can be tracked against this vocabulary without changing exact
admission.
The #594 closeout keeps the milestone reporting-only: `207,689`
source-prevalence occurrences are classified, `18,637` callback-shaped
occurrences and `95,805` channel/scheduling-shaped occurrences are priced, but
first exact-slice admissions remain `0`.

The first post-#594 callback diagnostics refinement turns the local `crates`
callback-demand/effect bucket into producer-facing subreasons. The broad
`hof-demand-effect-proof-missing` reason stays at `30`, but the checked [callback-demand-effect diagnostics](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v1.json) split its `by_obligation` rows into callback-effect proof (`27`), callback identity/shape proof (`2`), and predicate callback profile (`1`). Exact admission remains unchanged:
`semantic_admission_delta = 0`, with `false_merges == 0` and
`canon_preservation_violations == 0`.

The second callback diagnostics pass keeps those gates unchanged and splits the
callback-effect bucket itself. The [v2 callback-demand/effect diagnostics](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v2.json) report `22`
callback call-effect rows, `5` callback assignment-effect rows, `2` callback
identity/shape rows, and `1` predicate callback profile row; runtime-boundary
callback rows are currently `0`.

The third callback diagnostics pass keeps the same hard gate and splits the
call-effect rows by producer-facing call shape. The [v3 callback-demand/effect diagnostics](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v3.json)
report member call proof (`10`), Rust macro call proof (`8`),
direct-function effect contracts (`3`), and imported-function effect contracts
(`1`) as row-level subreasons. The underlying evidence counts also show member
call proof (`22`), Rust macro call proof (`8`), direct-function effect proof
(`5`), scoped-path call proof (`4`), imported-member call proof (`3`),
imported-function call proof (`2`), local-or-parameter call proof (`2`), and
builtin call proof (`1`), giving the next producer slice a measured queue.

The Promise protocol diagnostics pass applies the same closed-boundary approach
to JS/TS Promise work. The [promise-protocol diagnostics](../bench/recall_loss/promise-protocol-diagnostics-2026-06-28.v1.json)
keep the `29,094`-occurrence Promise/async source-prevalence group closed, but
the report vocabulary now separates await scheduling, async function scheduling,
Promise executor callbacks, Promise factories, aggregate result channels,
rejection channels, and non-construct Promise calls. JS/TS async functions also
emit `Source::Protocol(AsyncFunction)`, so an async function without `await`
stays fail-closed instead of looking like an ordinary synchronous body. The
`crates` gate remains `false_merges == 0` and
`canon_preservation_violations == 0`; local Promise fixture rows demonstrate
executor, rejection, aggregate, factory, and non-construct subreasons.
The later [promise rejection/continuation diagnostics](../bench/recall_loss/promise-rejection-continuation-diagnostics-2026-06-28.v1.json)
keep exact admission closed while splitting the rejection-channel catch-all into
`Promise.reject` rejected-value channels, `.catch` rejection continuations, and
`.finally` settlement continuations. `.catch` and `.finally` also carry callback
demand/effect evidence labels so future oracle-visible rows do not collapse into
one generic continuation bucket.
The [promise then obligation diagnostics](../bench/recall_loss/promise-then-obligation-diagnostics-2026-06-28.v1.json)
complete the next reporting-only split for `.then`: receiver proof is the
primary rollup, and fulfillment continuation, rejection continuation, and
callback demand/effect stay visible as separate missing-evidence labels. The
slice also detects expression receivers such as `db.get(id).then(f)`, which
keeps selector-only thenables visible without treating them as admitted Promise
continuations.
The [promise continuation report-row slice](../bench/recall_loss/promise-continuation-report-rows-2026-06-28.v1.json)
then moves the split from label-only diagnostics into actual recall-loss rows:
focused `.then`, `.catch`, and `.finally` fixtures produce `3/3`
oracle-interpretable admission rejections with zero oracle exclusions. The
current recovery priority is therefore quantified as receiver proof first:
`.then` has `36/39` unhinted receivers and `.catch` has `32/34`, so PromiseLike
receiver producer proof is the next dependency before exact continuation work.
The first recovery pass, [promise local continuation recovery](../bench/recall_loss/promise-local-continuation-recovery-2026-06-29.v1.json),
opens that dependency-closed local subset instead of broad async equivalence:
`Promise.reject`, `.catch`, two-argument `.then`, fulfilled/rejected value-graph
states, handler-returned `Promise.resolve` flattening, and
`catch`/`then(undefined, onRejected)` convergence. The `crates` recall-loss gate
still has `false_merges == 0` and `canon_preservation_violations == 0`; the
repo-local crates surface has no JS/TS Promise continuation runtime rows, so the
behavior-changing signal is pinned by focused value-graph and CLI equivalence
tests. The next measured queue remains producer proof for non-local Promise
receivers, then settlement/aggregate channels.
The next recovery pass, [same-file async-function return recovery](../bench/recall_loss/promise-async-function-return-recovery-2026-06-29.v1.json),
opens the smallest measured call-return producer class from the prior scan:
`79` same-file async-function call receivers across `10` JS/TS corpus repos. A
direct call to a source-proven async function now carries `PromiseLike`
result-domain evidence, and a pure non-thenable-safe returned payload can feed
local `.then` fulfillment recovery without merging with synchronous payload
code. The local `crates` gate remains at `false_merges == 0` and
`canon_preservation_violations == 0`; because `crates` has no JS/TS Promise
runtime rows, the behavior change is pinned by focused call-target,
equivalence, and recall-loss-report tests. Await, throw/rejection, possible
thenables, opaque call results, constructor receivers, imported/member call
returns, `.finally`, and aggregate channels remain closed.
The next recovery pass, [direct-function Promise return recovery](../bench/recall_loss/promise-direct-function-return-recovery-2026-06-29.v1.json),
opens the proof-backed same-file direct-function subset of the `184`
local/parameter call-return candidates from the prior 120-repo JS/TS scan.
Direct calls now get `Domain(PromiseLike)` result evidence when their target is
a non-async single-return function whose returned expression already has
PromiseLike domain proof. This lets literal `Promise.resolve`, typed
non-thenable `Promise.resolve`, and `Promise.reject` helper returns converge
with their direct Promise forms while staying distinct from synchronous
payloads. The local `crates` gate remains at `false_merges == 0` and
`canon_preservation_violations == 0`; because `crates` still has no JS/TS
Promise runtime rows, the behavior change is pinned by focused call-target,
equivalence, and recall-loss-report tests. Parameter callees, member/imported
call returns, unsafe thenables, constructors, `.finally`, aggregate channels,
and broad scheduling remain closed.
The next recovery pass, [Promise finally settlement recovery](../bench/recall_loss/promise-finally-settlement-recovery-2026-06-29.v1.json),
opens the exact-safe local `.finally` subset without widening scheduling or
thenable assimilation. `Promise.resolve(1).finally(() => 9).then(...)`
converges with the direct `Promise.resolve(1).then(...)` form, rejected
producers preserve their rejected channel through safe finally handlers, and
finally handlers returning `Promise.reject(reason)` move the result to that
rejected channel. The recall-loss fixture now includes a safe `finallyLocal`
unit that adds no runtime-boundary rejection. Parameterized handlers, possible
thenables, selector-only receivers, imported producers without settled-value
contracts, constructors, aggregate combinators, and broad async scheduling stay
closed.
The branch-return follow-up [promise-branch-return-producer-recovery-2026-06-29.v1.json](../bench/recall_loss/promise-branch-return-producer-recovery-2026-06-29.v1.json)
extends that producer proof to supported direct-function and DirectMethod bodies
where every returned expression carries PromiseLike domain evidence. Same-channel
fulfilled/fulfilled or rejected/rejected branches recover through Promise Phi
states; mixed fulfilled/rejected branches stay closed. The local `crates` gate
still reports `false_merges == 0` and `canon_preservation_violations == 0`,
with the focused branch-return equivalence and recall-loss fixtures pinning the
JS/TS behavior.
The follow-up [imported Promise call-return boundary](../bench/recall_loss/promise-imported-call-return-boundary-2026-06-29.v1.json)
does not open exact admission. It sharpens the imported target-present labels
exposed by the call-return diagnostics: imported function/member receivers need
a settled-value or rejection-channel contract, not return-domain proof alone,
because an import coordinate does not expose an evaluable local body. The source
scan keeps the next imported queue quantified at `105` imported-member
candidates across `9` repos and `73` imported-binding candidates across `15`
repos, while focused report/equivalence tests keep import-backed Promise member
calls distinct from direct Promise forms and synchronous payloads.
The follow-up [Promise imported settled-value contract](../bench/recall_loss/promise-imported-settled-value-contract-2026-06-29.v1.json)
adds that reusable contract surface without guessing from import identity. A
producer call can recover only when builtin `PromiseSettledValue` evidence names
the settled channel and exact payload node, the same call has admitted imported
call-target identity, the receiver has `Domain(PromiseLike)` proof, and the
continuation has admitted Promise API evidence. Focused imported `.then` and
`.catch` fixtures now recover behind Promise boundaries; contractless imported
producers, unsafe fulfilled thenable payloads, selector-only members, aggregate
combinators, constructors, and broad scheduling remain closed.
The follow-up [Node timers safe payload recovery](../bench/recall_loss/promise-node-timers-safe-payload-recovery-2026-06-29.v1.json)
applies that same settled-value capability to the Node `timers/promises` subset
only where the documented API has no options object that can inject
AbortSignal rejection. Exactly `setTimeout(delay, value)` and
`setImmediate(value)` can now name the fulfilled payload, while
`setTimeout(delay, value, options)`, `setImmediate(value, options)`,
possible-thenable payloads, scheduler APIs, interval streams, and broad
scheduling equivalence stay closed. The current 120-repo corpus scan found
`0` direct safe-payload call sites, so the measured pinned-corpus recall delta
is intentionally `0`; the benefit is that future safe call sites and focused
fixtures use the shared kernel contract rather than a selector shortcut.
The cycle closeout is recorded in [Promise/scheduling closeout](../bench/recall_loss/promise-scheduling-closeout-2026-06-29.v1.json).
The current `crates` gate reports `false_merges == 0`,
`canon_preservation_violations == 0`, and `0` Promise/scheduling unsupported
runtime rows; the remaining `14` unsupported runtime rows are
exception-channel contracts. That closeout deliberately stops API-by-API
Promise expansion here. The next work item should be a broader scheduling,
aggregate, cancellation, and lifecycle capability epic, tracked as
[#602](https://github.com/corca-ai/nose/issues/602), with its own corpus
pricing, hard negatives, local gates, performance checks, and docs.
The first #602 reporting slice is [scheduling/lifecycle boundary audit](../bench/recall_loss/scheduling-lifecycle-boundary-audit-602-2026-06-29.v1.json).
It adds a reusable lexical pricing script for the 120-repo corpus and keeps
`semantic_admission_delta = 0`. The scan prices `142,844` scheduling,
aggregate, cancellation, channel, executor, lifecycle, and exception
occurrences by file language. The first recommended targets are `Promise.all`
(`397` occurrences), `Promise.race` (`32`), `new Promise` (`795`),
AbortController/AbortSignal (`260`), `setInterval` (`73`), Go goroutine
statements (`1,949`), Java `CompletableFuture` (`306`), and Swift `await`
(`8,689`). The attached current `crates` gate still reports `false_merges == 0`
and `canon_preservation_violations == 0`; relevant runtime rows are still only
the existing `14` exception-channel rows.
The first exact follow-up is [Promise.all literal aggregate recovery](../bench/recall_loss/promise-all-literal-aggregate-recovery-2026-06-29.v1.json).
It opens only the fulfilled literal-array subset, with `397` broad
`Promise.all` occurrences, `201` literal-array boundary occurrences, and `0`
direct safe-seed occurrences in the pinned 120-repo corpus. The local `crates`
gate remains `false_merges == 0` and `canon_preservation_violations == 0`.
The next exact aggregate follow-up is [Promise.allSettled literal aggregate recovery](../bench/recall_loss/promise-allsettled-literal-aggregate-recovery-2026-06-29.v1.json).
It opens only the fulfilled-result all-settled literal-array subset, with `17`
broad `Promise.allSettled` occurrences and `8` literal-array boundary
occurrences in the pinned corpus. The following [Promise aggregate raw-input assimilation](../bench/recall_loss/promise-aggregate-raw-input-recovery-2026-06-29.v1.json)
slice reuses the existing non-thenable-safe proof to treat raw primitive
aggregate elements as fulfilled inputs for already-admitted literal
`Promise.all` and `Promise.allSettled` calls. The corpus scan finds `8`
`Promise.all` literal arrays and `1` `Promise.allSettled` literal array with a
direct raw non-thenable element, with `3` fully lexical direct-safe candidates.
The [Promise.race/Promise.any literal aggregate recovery](../bench/recall_loss/promise-race-any-literal-aggregate-recovery-2026-06-30.v1.json)
follow-up opens the first-observed aggregate subset through the same aggregate
contract family. `Promise.race` recovers only non-empty literal arrays where
every element has closed settlement or non-thenable-safe raw-input proof, and
returns the first element's settlement. `Promise.any` recovers only fully closed
literal arrays with at least one fulfilled candidate, and returns the first
fulfilled element. The corpus scan finds `32` broad `Promise.race` occurrences,
`31` `Promise.race` literal arrays, `1` broad/literal `Promise.any` occurrence,
and `0` fully closed lexical candidates for either opened path in the pinned
corpus. Dynamic iterables, object/function raw inputs, untyped possible
thenables, all-rejected `Promise.any` AggregateError payloads, executor timing,
and sync aggregate equivalence stay closed.

The [Promise executor boundary audit](../bench/recall_loss/promise-executor-boundary-audit-2026-06-30.v1.json)
then keeps constructor recovery reporting-only instead of opening a broad
executor model. The pinned corpus has `795` `new Promise(...)` occurrences
across `18` repos; `756` use inline arrow executors and `30` use function
executors, but the high-risk boundaries dominate: `664` have extra executor
calls, `319` mention timer/scheduler-like constructs, `180` have multiple
settlement calls, `338` resolved payload sites are possible thenable
assimilation boundaries, and `9` contain `throw`. The only lexical direct
single-settlement upper bound is `27` scalar resolves plus `4` scalar rejects.
This slice therefore records `semantic_admission_delta = 0` and names the next
admission requirements before any constructor exact recovery: static-global
constructor proof, resolve/reject callback identity, single-settlement
precedence, throw-to-rejection and throw-after-settlement ordering, explicit
executor effects, non-thenable payload proof, and preservation of the Promise
boundary.

The [AbortSignal cancellation boundary audit](../bench/recall_loss/abort-signal-cancellation-boundary-audit-2026-06-30.v1.json)
is the next reporting-only #602 readiness slice. Runtime-boundary diagnostics
now name `AbortSignal.abort`, `AbortSignal.any`, `AbortSignal.timeout`, and
`new AbortController()` as cancellation/lifecycle obligations rather than
opaque unsupported runtime rows. The pinned corpus has `260` Abort mentions,
`156` `AbortController` constructors, `175` `.abort()` selector calls, `323`
`.signal` property reads, `193` `signal` option properties, `6` signal-bearing `fetch`
calls, `2` signal-bearing timer calls, and `2` signal-bearing
`addEventListener` calls. The artifact keeps `semantic_admission_delta = 0`;
future exact cancellation work must prove signal identity, abort ordering,
abort reason propagation, listener/timer/fetch rejection behavior, and
controller-signal lifecycle before merging cancellation-sensitive forms.

The [interval/scheduler lifecycle boundary audit](../bench/recall_loss/interval-scheduler-lifecycle-boundary-audit-2026-06-30.v1.json)
continues the reporting-only #602 readiness work for timer, interval,
scheduler, and microtask surfaces. Runtime-boundary diagnostics now name global
timer scheduling, `scheduler.wait` cancellation/liveness, `scheduler.yield`
microtask ordering, `setInterval` repeated-emission lifecycle, and
`clearInterval` plus one-shot timer/frame cancellation lifecycle as structured
obligations. The pinned
corpus has `780` `setTimeout` calls, `57` `setImmediate` calls, `73` bare
`setInterval` calls, `23` member `.setInterval` calls, `55` `clearInterval`
calls, `133` `clearTimeout` calls, `14` `queueMicrotask` calls, `43`
`requestAnimationFrame` calls, and `11` `scheduler.yield` calls. The artifact
keeps `semantic_admission_delta = 0`;
future exact scheduling work must prove callback identity, callback
demand/effect, task/microtask/timer ordering, interval cardinality, and
cancellation/cleanup behavior before merging scheduled operations.

The [#602 closeout](../bench/recall_loss/issue-602-closeout-2026-06-30.v1.json)
closes this broad boundary milestone at the capability boundary. The opened
exact work is limited to dependency-backed literal Promise aggregate slices;
executor, cancellation, scheduler, timer, interval, and cross-language
lifecycle surfaces remain fail-closed with named obligations. The current local
`crates` report has `false_merges = 0`, `canon_preservation_violations = 0`,
and no Promise/scheduling/aggregate/cancellation/lifecycle unsupported-runtime
rollup rows; the remaining `14` unsupported-runtime boundary rows are named
`exception-channel-contract-missing` obligations.

## See Also

- [recall-loss-diagnostics](recall-loss-diagnostics.md) defines the report
  format and bucket taxonomy that feed this loop.
- The [import-backed immutable provenance closeout](import-backed-immutable-provenance-closeout-567.md)
  milestone turned import-backed recall loss into closed
  provenance evidence.
- [semantic-pack-architecture](semantic-pack-architecture.md) defines how
  semantic-pack changes must preserve exact-channel admission boundaries.
- [source-facts](source-facts.md) describes source-origin facts used by
  recall-loss fixes that depend on provenance.
- [evidence-records](evidence-records.md) defines the shared evidence substrate
  used by diagnostics and kernel admission.
- [demand-effect-semantics](demand-effect-semantics.md) explains the demand and
  effect contracts that make stricter stdlib matching safe.
