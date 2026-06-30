# Recall-loss diagnostics

`nose verify --recall-loss-report <file>` writes a local JSON report for the
strictness/recall tradeoff in exact semantic matching. It is a diagnostics
artifact, not telemetry: nose does not send it anywhere, and raw source snippets
are omitted by default.

Use this report when a semantic-kernel or semantic-pack change touches exact
admission. The hard rule stays unchanged: stricter admission must not introduce
false merges. The report adds the missing second question: if exact admission got
stricter, what recall did it close and which capability or evidence gap explains
that loss?

## Command

```sh
nose verify <path> --max-violations 0 --recall-loss-report recall-loss.json
```

The command reuses the same interpreter oracle as `nose verify`. The human
stdout remains the existing soundness/completeness report; the JSON artifact is
written only to the requested path.

Compare two local reports with:

```sh
python3 scripts/recall-loss-diff.py before.json after.json
```

The comparison is deterministic and suitable for PR comments: it shows hard gate
deltas, completeness and under-merge deltas, oracle exclusion deltas by reason
and obligation, admission rejection deltas by reason, and top opportunities
added or removed.

## Report shape

The current schema is `recall_loss_report.v1.json`:

| field | meaning |
|---|---|
| `schema_version` | Report schema version. Starts at `1`. |
| `privacy` | Local-artifact flags. `remote_collection` is `false`; `raw_source_snippets_included` is `false` by default. |
| `summary` | Total units, interpretable units, excluded units, canon checks, and exact-admission rejection count. |
| `soundness_gate` | Fingerprint groups, false merges, advisory disagreements, canon-preservation violations, `--max-violations`, and gate result. |
| `completeness` | Behavior groups, behavior-equal pairs, fingerprint-equal pairs, completeness percentage, and under-merged groups. |
| `oracle_under_merges` | Behavior-equal but fingerprint-split pairs, sorted by value-Jaccard nearness. This is the structured form of the `--leads` signal. |
| `oracle_exclusions` | Fail-closed oracle exclusions by reason, optional obligation attribution, and unit location. |
| `import_snapshot_census` | Corpus-level imported immutable snapshot diagnostics: successful snapshot record counts, unresolved binding-import miss counts by reason/language, and stable hash/location rows for follow-up fixtures. |
| `admission_rejections` | Interpretable units whose exact semantic claim is closed, with structured reason, gate, capability, missing evidence, #594 obligation family/subreason, oracle status, and stable location. |
| `by_reason` | Rollups for admission rejections by reason/gate/capability. |
| `by_obligation` | Rollups for interpretable admission rejections by #594 obligation family and stable subreason. |
| `top_opportunities` | Ranked under-merge opportunities that future capability work can turn into fixtures or focused follow-up issues. |

The current admission-rejection taxonomy is diagnostics-only; it does not widen
or narrow product admission by itself.

`oracle_exclusions.by_obligation` is separate from top-level `by_obligation`.
Top-level `by_obligation` counts only oracle-interpretable units whose exact
semantic claim was closed. `oracle_exclusions.by_obligation` counts
fail-closed units that the oracle could not interpret but that still have a
diagnostics-only capability attribution, such as a lowered runtime/protocol
boundary. Excluded-unit attribution reuses the same `reason`,
`missing_evidence`, `obligation_family`, and `obligation_subreason` vocabulary
as admission rejections, but carries `oracle_status: "excluded"` and does not
open exact admission.

| reason | meaning |
|---|---|
| `import-symbol-callee-identity-proof-missing` | An ordinary call is interpretable, but exact admission lacks reusable proof of the callee/import/symbol target. |
| `receiver-domain-proof-missing` | A receiver method call needs receiver-domain evidence rather than selector-name inference. |
| `library-api-occurrence-proof-missing` | A canonical builtin/API occurrence lacks admitted pack or producer evidence. |
| `hof-demand-effect-proof-missing` | A higher-order surface lacks a demand, effect, and materialization profile. |
| `source-surface-proof-missing` | A source construct, operator, comprehension, Rust macro invocation, or syntax distinction is required but not proven. |
| `mutation-effect-boundary` | Mutation, place, side-effecting call, or effect obligations close exact admission until an effect-preserving contract exists. |
| `unsupported-runtime-boundary` | Runtime/protocol boundaries such as raw lowered constructs, try/throw, splat, or keyword-argument surfaces intentionally fail closed. |
| `value-fingerprint-too-small` | The unit is strict-exact-safe, but its value fingerprint is below the non-degenerate exact-claim floor. |
| `unattributed-strict-exact-unsafe` | Fallback for unknown strict-exact rejection. This should stay visible and should trend toward zero. |

Unknown cases must remain explicit as `unattributed-strict-exact-unsafe`; do not
guess.

Each admission rejection also carries an `obligation_family` and
`obligation_subreason`. These fields are diagnostics-only and refine broad
reason buckets into the cross-language vocabulary from [scheduling-channel-callback-obligations-594](scheduling-channel-callback-obligations-594.md).
They do not change exact admission.

| obligation family | typical subreason | meaning |
|---|---|---|
| `callback-demand-effect` | `callback-member-call-effect-proof-missing`, `callback-rust-macro-call-effect-proof-missing`, `callback-direct-function-call-effect-contract-missing`, `callback-imported-function-call-effect-contract-missing`, `callback-assignment-effect-proof-missing`, `callback-runtime-boundary-proof-missing`, `callback-identity-or-shape-proof-missing`, `promise-then-callback-demand-effect-contract-missing`, `future-callback-demand-effect-contract-missing`, `goroutine-callback-effect-contract-missing`, `defer-callback-effect-contract-missing`, `mapping-callback-demand-effect-profile-missing`, `predicate-callback-demand-effect-profile-missing`, `flattening-callback-demand-effect-profile-missing`, `optional-callback-demand-effect-profile-missing`, or `reduction-callback-demand-effect-profile-missing` | A HOF/callback surface lacks timing, callback identity, effect visibility, result role, call-shape proof, or materialization proof. |
| `executor-callback` | `promise-executor-timing-contract-missing`, `promise-executor-resolve-reject-callback-contract-missing`, `promise-executor-throw-to-rejection-contract-missing`, or `promise-executor-callback-effect-contract-missing` | A Promise/Future-like constructor callback needs executor timing, resolve/reject callback identity, thrown-to-rejected outcome, and callback-effect proof before exact use. |
| `receiver-mutation` | `effect-preserving-contract-missing` | A mutation/place/effect boundary blocks exact admission. |
| `rejection-channel` | `promise-reject-rejected-value-channel-contract-missing`, `promise-catch-rejection-continuation-contract-missing`, `promise-finally-settlement-continuation-contract-missing`, `promise-then-rejection-continuation-contract-missing`, legacy `promise-rejection-channel-contract-missing`/`promise-rejection-continuation-contract-missing`, or `exception-channel-contract-missing` | Rejection, catch/finally settlement, throw, rescue, or non-local error flow must remain distinct from ordinary return values. |
| `success-error-result-channel` | `promise-factory-settled-value-contract-missing`, `promise-aggregate-all-fulfilled-contract-missing`, `promise-aggregate-all-settled-contract-missing`, `promise-aggregate-first-fulfilled-contract-missing`, `promise-aggregate-result-channel-contract-missing`, `promise-then-fulfillment-continuation-contract-missing`, `future-settled-value-channel-contract-missing`, `future-fulfillment-continuation-contract-missing`, `future-settlement-continuation-contract-missing`, `async-aggregate-all-completion-contract-missing`, `async-aggregate-completion-contract-missing`, or `async-aggregate-result-channel-contract-missing` | Success, empty/default, error/rejection, and aggregate result channels need explicit result-shape proof. |
| `cancellation-liveness-boundary` | `promise-aggregate-first-settled-contract-missing`, `promise-aggregate-cancellation-liveness-contract-missing`, `abort-signal-cancellation-contract-missing`, `abort-signal-lifecycle-contract-missing`, `abort-controller-signal-lifecycle-contract-missing`, `scheduler-wait-cancellation-liveness-contract-missing`, `timer-cancellation-liveness-contract-missing`, `interval-cancellation-liveness-contract-missing`, `task-cancellation-liveness-contract-missing`, `async-aggregate-first-completion-contract-missing`, or `async-aggregate-cancellation-liveness-contract-missing` | First-settled, cancellation, liveness, stop, and early-exit behavior must stay separate from all-value result channels. |
| `scheduling-boundary` | `async-await-scheduling-contract-missing`, `async-function-scheduling-contract-missing`, `async-block-scheduling-contract-missing`, legacy `promise-await-scheduling-contract-missing`, `promise-async-function-scheduling-contract-missing`, `future-async-block-scheduling-contract-missing`, `promise-non-construct-call-boundary-contract-missing`, `scheduler-wait-timing-contract-missing`, `scheduler-yield-microtask-order-contract-missing`, `timer-scheduling-contract-missing`, `task-spawn-scheduling-contract-missing`, `task-yield-scheduling-contract-missing`, `future-drive-scheduling-contract-missing`, `goroutine-scheduling-contract-missing`, or `runtime-protocol-boundary-contract-missing` | A lowered runtime/protocol construct needs scheduling or protocol semantics before exact use. |
| `channel-boundary` | `channel-send-synchronization-contract-missing`, `channel-receive-value-channel-contract-missing`, `channel-receive-status-contract-missing`, `channel-select-readiness-contract-missing`, `channel-select-case-selection-contract-missing`, `channel-select-default-liveness-contract-missing`, legacy `channel-send-receive-protocol-contract-missing`, `channel-select-protocol-contract-missing`, or `channel-protocol-contract-missing` | Channel send/receive/select semantics need blocking, synchronization, close/status, readiness, and liveness evidence before exact use. |
| `exception-channel` | `exception-channel-contract-missing` or `future-exception-continuation-contract-missing` | Try/throw/error propagation and exceptional future continuations are explicit channel boundaries, not scheduling boundaries. |
| `lifecycle-materialization-boundary` | `interval-async-iteration-lifecycle-contract-missing`, `task-handle-lifecycle-contract-missing`, `defer-lifecycle-ordering-contract-missing`, `generator-yield-lifecycle-contract-missing`, or `generator-yield-protocol-contract-missing` | Repeated emission, deferred execution, suspension, task handles, one-shot views, reusable collections, and materialization require explicit lifecycle proof. |
| `ambiguous-selector-boundary` | `receiver-domain-proof-missing`, `promise-then-promise-like-receiver-proof-missing`, `library-api-occurrence-evidence-missing`, or a call-target proof label | Selector, receiver, library API, or callee identity proof is missing. |
| `source-protocol-boundary` | `source-surface-contract-missing`, `rust-macro-expansion-contract-missing` | A source/protocol syntax distinction is required but not proven. |
| `non-degenerate-fingerprint-floor` | `non-degenerate-value-fingerprint` | The unit is otherwise exact-safe but too small for a non-degenerate exact claim. |
| `unattributed-boundary` | `strict-exact-safe-tree-missing` | A strict-exact rejection still lacks a more specific capability attribution. |

The checked-in baseline summaries and the five-cycle recovery log are described
in [recall-loss-recovery-loop](recall-loss-recovery-loop.md).
The #572 cycle also records a diagnostics-only refinement: expression-statement
calls that need an effect contract stay in the effect boundary bucket, and
unmodeled Rust macros such as `format!` stay in the source-surface bucket until
a macro expansion or library contract proves their behavior.
The #574 cycle keeps the same `import-symbol-callee-identity-proof-missing`
reason but splits its `missing_evidence` labels by call-target surface, such as
`local-or-parameter-call-target-proof`, `scoped-path-call-target-proof`,
`member-call-target-proof`, imported/global target proof labels, and admitted
target-present call-contract proof labels. Build the checked-in census with
`scripts/recall-loss-callee-census.py`.

The post-#594 callback diagnostics refinement keeps the same
`hof-demand-effect-proof-missing` reason, but HOF rejections now also expose
kind-specific and callback-specific `missing_evidence` labels such as
`hof-map-callback-demand-effect-profile`, `hof-filter-callback-demand-effect-profile`,
`hof-callback-call-effect-proof`, `hof-callback-assignment-effect-proof`,
`hof-callback-runtime-boundary-proof`, and `hof-callback-identity-proof`. The
checked baselines are [callback-demand-effect-diagnostics-2026-06-28.v1.json](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v1.json)
and [callback-demand-effect-diagnostics-2026-06-28.v2.json](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v2.json).
The follow-up [callback-demand-effect-diagnostics-2026-06-28.v3.json](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v3.json)
keeps exact admission closed but splits callback call-effect proof by
producer-facing call shape: member calls (`10`), Rust macro calls (`8`),
direct-function effect contracts (`3`), and imported-function effect contracts
(`1`) on the local `crates` surface.

Promise protocol diagnostics keep exact admission closed while splitting
runtime-boundary evidence by scheduling, executor callback, rejection channel,
and aggregate-result obligations. Current reports use the language-neutral
`async-await-scheduling-contract` label for `Source::Protocol(Await)` across
JS/TS, Python, Rust, and Swift, while legacy checked artifacts may still contain
the older Promise-specific await label. The follow-up [oracle-exclusion obligation reporting](../bench/recall_loss/oracle-exclusion-obligation-reporting-2026-06-30.v1.json)
keeps that label visible even when await-only runtime/protocol units are excluded
before admission-rejection rows exist: JS/TS, Python, Rust, and Swift fixtures
roll up under `oracle_exclusions.by_obligation` as
`scheduling-boundary/async-await-scheduling-contract-missing`, while the
top-level interpretable `by_obligation` stays separate. The follow-up [cross-language async-function obligation reporting](../bench/recall_loss/cross-language-async-function-obligation-reporting-2026-06-30.v1.json)
extends the same reporting vocabulary to `Source::Protocol(AsyncFunction)` for
JS/TS, Python, Rust, and Swift runtime bodies, plus
`Source::Protocol(AsyncBlock)` for Rust async blocks. New reports use
`async-function-scheduling-contract` and `async-block-scheduling-contract`;
legacy checked artifacts may still mention
`promise-async-function-scheduling-contract` or
`future-async-block-scheduling-contract`. Promise receiver/producer labels such
as `promise-async-function-return-producer-proof` remain Promise-specific. The
follow-up [non-JS async runtime API obligation reporting](../bench/recall_loss/non-js-async-runtime-api-obligation-reporting-2026-06-30.v1.json)
keeps exact admission closed while extending the same runtime-boundary
attribution process to Python `asyncio.create_task`/`sleep`/`gather`/`wait`,
Rust `tokio`/`async-std` spawn and qualified
`tokio`/`futures`/`futures_util` `join!`/`select!` macros, and Swift `Task`
creation. New reports use shared `task-spawn-scheduling-contract`,
`task-handle-lifecycle-contract`, `task-cancellation-liveness-contract`, and
`async-aggregate-*` labels; interpretable rejections stay in top-level
`by_obligation`, while excluded runtime units stay under
`oracle_exclusions.by_obligation`. The follow-up [non-JS async runtime
attribution hardening](../bench/recall_loss/non-js-async-runtime-attribution-hardening-2026-06-30.v1.json)
keeps those labels reporting-only but requires stronger runtime identity proof:
Python `asyncio.*` needs import-backed namespace evidence with no path-visible
local `asyncio` module, Rust spawn and aggregate paths must be qualified
`tokio`/`async_std`/`futures`/`futures_util` paths whose root is not locally
defined in the same file, and Swift `Task` must be unshadowed and not
corpus-visible as a local Swift definition before task/aggregate obligations are
attributed. The
follow-up [non-JS async runtime import proof](../bench/recall_loss/non-js-async-runtime-import-proof-2026-06-30.v1.json)
keeps exact admission closed but widens attribution to import-backed spellings:
Python `asyncio` namespace aliases such as `import asyncio as aio; aio.wait(...)`
and Rust imported runtime bindings such as `use tokio::spawn; spawn(...)`,
`use tokio::join; join!(...)`, and `use futures::select; select!(...)`. The
matching [120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-import-proof-2026-06-30.v1.json)
adds `11` Rust imported-binding occurrences over the qualified-only audit and
finds no Python `asyncio` alias occurrences in the pinned corpus. The
follow-up [imported-binding proof artifact](../bench/recall_loss/non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json)
keeps the same reporting-only boundary while extending attribution to Python
`from asyncio import ...` bindings and Rust brace imports. The follow-up [Swift
structured-concurrency artifact](../bench/recall_loss/swift-structured-concurrency-obligation-reporting-2026-06-30.v1.json)
then maps `Task.sleep`, `Task.yield`, and task-group calls to timer,
task-yield, aggregate, cancellation/liveness, result-channel, and
exception-channel obligations. The follow-up [Java CompletableFuture
artifact](../bench/recall_loss/java-completablefuture-obligation-reporting-2026-06-30.v1.json)
adds generic Future obligation labels for proof-backed Java
`CompletableFuture` static calls and exact-import-backed CompletionStage-style
receiver continuations:
`future-settled-value-channel-contract`,
`future-fulfillment-continuation-contract`,
`future-settlement-continuation-contract`,
`future-exception-continuation-contract`, and
`future-callback-demand-effect-contract`. It keeps exact admission closed while
pricing `40` lexical Java future reporting candidates in the pinned corpus and
leaving `276` broad `CompletableFuture` mentions closed. The
follow-up [Rust block_on future-drive artifact](../bench/recall_loss/rust-block-on-future-drive-obligation-reporting-2026-07-01.v1.json)
keeps exact admission closed and maps qualified/import-backed Rust
`tokio_test::block_on` calls plus proof-backed tokio runtime receiver chains to
`future-drive-scheduling-contract` plus the existing
`future-settled-value-channel-contract`. Selector-only `.block_on` calls and
unproven field/parameter receivers such as `self.rt.block_on(...)` initially
remained closed until runtime receiver/type evidence existed. The [Rust local
runtime provenance artifact](../bench/recall_loss/rust-block-on-local-runtime-provenance-2026-07-01.v1.json)
adds proof-backed local variables whose last visible assignment is direct
`Handle::current()`, `Runtime::new().unwrap()/expect/?`, or
`Builder::new_*().build().unwrap()/expect/?`. The [Rust parameter runtime
provenance artifact](../bench/recall_loss/rust-block-on-parameter-runtime-provenance-2026-07-01.v1.json)
then adds nominal `tokio::runtime::Runtime`/`Handle` parameter receivers backed
by fully qualified type text or scope-visible exact imported-binding evidence;
struct fields, nested brace imports such as `use tokio::{ runtime::{Runtime}, ... }`, type
aliases, wrappers, and `map_err(...)?` construction remained closed in that
slice. The [Rust nested brace runtime provenance artifact](../bench/recall_loss/rust-nested-brace-runtime-provenance-2026-07-01.v1.json)
then adds per-item evidence for nested static brace imports, allowing those
`Runtime` parameter receivers to reuse the same `block_on` reporting. Struct
fields remained closed in that slice. The [Rust self-field runtime provenance artifact](../bench/recall_loss/rust-self-field-runtime-provenance-2026-07-01.v1.json)
then adds exact `self.<field>.block_on(...)` receivers when a same-scope struct
field declaration proves `tokio::runtime::Runtime` or `Handle` through fully
qualified or exact imported-binding type evidence. In the Tokio `sync_bridge.rs`
spot check, future-drive oracle exclusions move from `0` to `13` with `0` false
merges. Non-self fields, local struct fields, project-local `tokio` roots or
aliases, wildcard/relative imports, type aliases, wrappers,
and constructor-assigned fields remain closed in that slice. The [Rust local
self-field runtime provenance artifact](../bench/recall_loss/rust-local-self-field-runtime-provenance-2026-07-01.v1.json)
then extends the same receiver type-provenance capability to function/block
local `struct` plus local `impl` declarations. In Tokio `task_local_set.rs`, the
local `self.rt.block_on(...)` row moves from
`receiver-mutation/effect-preserving-contract-missing` to
`scheduling-boundary/future-drive-scheduling-contract-missing` with `0` false
merges. Duplicate local structs, wrong local `Runtime` imports, same-scope
`Runtime` types, namespace aliases named `tokio`, non-self fields, type aliases,
wrappers, and constructor-assigned fields remain closed. The [Rust map_err runtime
provenance artifact](../bench/recall_loss/rust-block-on-map-err-runtime-provenance-2026-07-01.v1.json)
then opens only success-channel-preserving `Result::map_err` adapters over
already proven `Runtime::new()` or `Builder::build()` results, moving two
Nushell direct local block_on spot checks from `0` to `1` future-drive evidence
unit each while wrapper-returned Results, non-Result `map_err` calls, and
constructor-assigned fields stay closed. The
follow-up [Go channel protocol pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-go-channel-protocol-2026-06-30.v1.json)
keeps exact admission closed while refining Go protocol-boundary reporting into
channel send synchronization, receive value, comma-ok receive status, select
readiness/case/default, goroutine scheduling plus callback effect, and defer
lifecycle plus callback effect obligations. The 120-repo protocol-node pricing
records `4,294` channel receives, `1,525` sends, `155` comma-ok receives,
`1,920` select parents, `3,590` select cases, `546` select defaults, `1,949`
goroutines, and `17,521` defers; select parents and arms are counted separately
because lowering preserves them as distinct source-backed protocol boundaries.
The [Go select receive-status artifact](../bench/recall_loss/go-select-receive-status-protocol-2026-07-01.v1.json)
fills the select communication-case gap: `case _, ok := <-ch` keeps select
readiness as the primary rollup but now also carries
`channel-receive-status-contract` in `missing_evidence`. Exact channel/select
recovery remains closed.
The follow-up [non-JS async runtime scope-shadowing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-scope-shadowing-2026-06-30.v1.json)
keeps exact admission closed while making Python/Rust async runtime attribution
scope-aware: unrelated local shadows in other functions no longer suppress
import-backed `asyncio` or Rust runtime reporting, but same-scope,
enclosing-scope, and module-level shadows remain closed. The 120-repo pricing
total stays unchanged at `146,880`, so the improvement is stricter report
attribution rather than a new source-prevalence slice. Python/Rust
async-runtime diagnostics are computed from the source-preserving unit root
before falling back to the normalized root, so normalized alpha names cannot
reopen Python `asyncio` alias shadows in the report.
The follow-up [non-JS async runtime breadth artifact](../bench/recall_loss/non-js-async-runtime-breadth-2026-07-01.v1.json)
keeps the same reporting-only policy while adding Python event-loop drive,
timeout, cancellation-shield, thread-safe submission, and thread-offload helpers
plus Swift continuation bridge functions. New reports use existing
`future-drive-scheduling-contract`, `timer-scheduling-contract`,
`timer-cancellation-liveness-contract`, `task-cancellation-liveness-contract`,
`task-spawn-scheduling-contract`, `future-settled-value-channel-contract`,
`future-settlement-continuation-contract`,
`future-callback-demand-effect-contract`, and `exception-channel-contract`
labels; exact recovery remains closed. The matching [120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-breadth-2026-07-01.v1.json)
adds `107` source-prevalence occurrences over the prior scope-shadowing audit
with `0` false merges on the local `crates` gate.
The
checked [promise-protocol diagnostics](../bench/recall_loss/promise-protocol-diagnostics-2026-06-28.v1.json)
connect the JS/TS source-prevalence group (`29,094` Promise/async occurrences)
to report labels such as legacy `promise-await-scheduling-contract`,
`promise-async-function-scheduling-contract`,
`promise-executor-callback-effect-contract`,
`promise-aggregate-result-channel-contract`,
`promise-rejection-channel-contract`, and
`promise-non-construct-call-boundary-contract`.
The follow-up [promise-protocol hard negatives](../bench/recall_loss/promise-protocol-hard-negatives-2026-06-28.v1.json)
keep `semantic_admission_delta = 0` and pin the closed boundaries that must
hold before any Promise recovery slice opens: async-function/sync convergence,
executor or factory/sync convergence, Promise chain/custom receiver
convergence, thenable assimilation, unsupported settled producers, and
aggregate first-settled/result-channel differences.
The first recovery slice is [promise-resolve-recovery-2026-06-28.v1.json](../bench/recall_loss/promise-resolve-recovery-2026-06-28.v1.json).
It opens only dependency-closed `Promise.resolve(value)` factories whose
argument is proven non-thenable-safe, while preserving the same hard negatives
for sync payloads, possible thenables, explicit PromiseLike values, executors,
aggregate channels, and rejection channels.
The reporting-only [promise rejection/continuation diagnostics](../bench/recall_loss/promise-rejection-continuation-diagnostics-2026-06-28.v1.json)
then split the former rejection-channel catch-all into `Promise.reject`
rejected-value channels, `.catch` rejection continuations, and `.finally`
settlement continuations. `.catch` and `.finally` also carry callback
demand/effect labels, but exact Promise continuation admission remains closed.
The follow-up [promise then obligation diagnostics](../bench/recall_loss/promise-then-obligation-diagnostics-2026-06-28.v1.json)
does the same for `.then`: selector-only or custom receivers report
`promise-then-promise-like-receiver-proof`, while fulfillment continuation,
rejection continuation, and callback demand/effect stay visible as distinct
missing-evidence labels.
The checked [promise continuation report-row fixture](../bench/recall_loss/promise-continuation-report-rows-2026-06-28.v1.json)
turns those labels into actual local `admission_rejections`: focused
`.then`, `.catch`, and `.finally` units are all oracle-interpretable, have zero
oracle exclusions, and report three fail-closed Promise continuation rows without
opening exact admission.
The follow-up [promise local continuation recovery](../bench/recall_loss/promise-local-continuation-recovery-2026-06-29.v1.json)
opens a narrow exact slice for first-party local Promise continuations while
preserving the recall-loss vocabulary for everything still closed. It admits
`Promise.reject`, `.catch`, two-argument `.then`, handler-returned
`Promise.resolve` flattening, and `catch`/`then(undefined, onRejected)`
convergence only when the receiver, producer, and callback are dependency-closed.
Custom thenables, unsafe `.finally` handlers, aggregate combinators, broad async
scheduling, and sync payload equivalence remain reportable under the existing
obligation buckets.
The reporting follow-ups [promise receiver-producer diagnostics](../bench/recall_loss/promise-receiver-producer-diagnostics-2026-06-29.v1.json)
and [promise call-return callee diagnostics](../bench/recall_loss/promise-call-return-callee-diagnostics-2026-06-29.v1.json)
keep those remaining receivers fail-closed but make the next capability gaps
specific: constructor-created promises, async-function returns, generic
call-return receivers, and then member/local/imported call-return callee shapes
all have named missing-evidence labels. These labels are attribution only; exact
admission still requires explicit callee identity plus returned `PromiseLike`
domain proof.
The [same-file async-function return recovery](../bench/recall_loss/promise-async-function-return-recovery-2026-06-29.v1.json)
slice opens the narrow JS/TS direct-call case behind that requirement. A direct
call to a JS/TS source-proven async function now has `PromiseLike` result-domain
evidence, and only pure non-thenable-safe returned payloads feed local `.then`
fulfillment recovery. The report should therefore move those receivers out of
`promise-async-function-return-producer-proof` and leave any still-closed work in
continuation, rejection, callback, or scheduling obligations.
The [direct-function Promise return recovery](../bench/recall_loss/promise-direct-function-return-recovery-2026-06-29.v1.json)
slice opens the next proof-backed direct-call subset: a same-file non-async
single-return function can provide `PromiseLike` result-domain evidence only
when the returned expression already has PromiseLike domain proof. The report
should therefore move those receivers out of
`promise-call-return-direct-function-return-domain-proof` and leave unproven
parameter callees, member/imported call returns, unsafe thenables, constructors,
unsafe `.finally` handlers, aggregate channels, and broad scheduling in their
existing fail-closed obligations.
The [direct-method Promise return recovery](../bench/recall_loss/promise-direct-method-return-recovery-2026-06-29.v1.json)
slice opens the proof-backed subset of member call-return receivers: an existing
DirectMethod target record plus returned-expression PromiseLike domain proof can
provide call-result `Domain(PromiseLike)` for non-async single-return methods.
The value graph evaluates only the returned expression and closes on receiver
context, so selector-only member calls, dynamic dispatch, imported members,
receiver-dependent methods, unsafe thenables, constructors, unsafe `.finally`
handlers, aggregate channels, and broad scheduling remain in fail-closed
obligations.
The [imported Promise call-return boundary](../bench/recall_loss/promise-imported-call-return-boundary-2026-06-29.v1.json)
keeps imported function/member receivers closed but sharpens the report
vocabulary: target-present imported Promise receivers now require a
settled-value contract rather than mere return-domain proof. Imported
call-target evidence proves a module/export/member coordinate, not a local body
whose fulfilled or rejected payload can be evaluated.
The [Promise imported settled-value contract](../bench/recall_loss/promise-imported-settled-value-contract-2026-06-29.v1.json)
adds that contract as a semantic-kernel capability. Recovery opens only when a
builtin, dependency-closed `PromiseSettledValue` record names a settled channel
and exact payload node for the same imported producer call, with separate
`CallTarget::ImportedFunction`/`ImportedMember`, `Domain(PromiseLike)`, and
Promise continuation API evidence. Imported producers without the contract still
report the settled-value missing labels.
The [branch-return Promise producer recovery](../bench/recall_loss/promise-branch-return-producer-recovery-2026-06-29.v1.json)
extends the direct-function and DirectMethod slices from single-return bodies to
supported branch-return bodies. A receiver should move out of
`promise-call-return-direct-function-return-domain-proof` only when every
returned expression on the supported paths has PromiseLike domain evidence.
Same-channel fulfilled/fulfilled or rejected/rejected branches can recover
through a Promise Phi; mixed fulfilled/rejected branches, imported receivers,
selector-only members, and missing return-domain proof remain in fail-closed
obligations.
The [Promise finally settlement recovery](../bench/recall_loss/promise-finally-settlement-recovery-2026-06-29.v1.json)
then opens only the exact-safe local `.finally` subset. A safe local
`makeResolved().finally(() => 9)` fixture should increase total unit coverage
without adding a new runtime-boundary rejection, while raw `.finally(p, h)`,
parameterized handlers, possible thenables, selector-only receivers, imported
producers without settled-value contracts, aggregate combinators, and broad
scheduling remain attributed to the existing Promise continuation obligations.

`import_snapshot_census` is also diagnostics-only. It does not make an imported
value exact-safe. It records why a proven binding import did not become an
imported immutable snapshot after corpus import resolution. Current miss reasons
include:

| reason | meaning |
|---|---|
| `provider-module-missing` | The imported module hash has no provider file in the analyzed corpus. |
| `provider-export-missing` | A provider module exists, but no matching exported binding was found. |
| `provider-export-ambiguous` | More than one provider binding could own the same module/export coordinate. |
| `provider-external-crate-boundary` | The import targets a known external crate dependency, which is outside same-corpus provider lookup. |
| `provider-reexport-ambiguous` | More than one public or crate-visible static re-export could own the requested module/export coordinate. |
| `provider-reexport-callable-boundary` | A public or crate-visible static re-export resolves to a callable item, not an immutable literal provider value. |
| `provider-reexport-type-boundary` | A public or crate-visible static re-export resolves to a type item, not an immutable literal provider value. |
| `provider-reexport-module-namespace-boundary` | A public or crate-visible static re-export resolves to a module namespace, not an immutable literal provider value. |
| `provider-reexport-external-crate-boundary` | A public or crate-visible static re-export target resolves to a known external crate boundary. |
| `provider-reexport-target-export-missing` | A public or crate-visible static re-export exists, but its target module has no matching export in the analyzed corpus. |
| `provider-reexport-target-module-missing` | A public or crate-visible static re-export exists, but its target module is not resolved in the analyzed corpus. |
| `cross-language-boundary` | A same-coordinate provider exists only in a different lowered language. |
| `self-import-boundary` | The only matching provider is the importer file itself. |
| `importer-binding-mutated` | The importer mutates the imported binding before it could be snapshotted. |
| `provider-binding-unsafe` | The provider binding is mutated or escapes through an opaque call argument. |
| `provider-library-api-proof-missing` | The provider RHS is a factory call without admitted `LibraryApi` proof. |
| `provider-factory-arguments-not-exact-safe` | The provider factory is proven, but its arguments are not export-safe. |
| `provider-aggregate-children-not-exact-safe` | The provider aggregate has a surface proof, but its children are not export-safe imported literal values. |
| `provider-sequence-surface-not-import-literal-safe` | The provider aggregate has a proven sequence surface, but that surface is not an imported-literal value surface. |
| `provider-aggregate-child-reference-boundary` | The provider aggregate contains a child reference, field path, or index expression rather than a literal/export-safe value. |
| `provider-aggregate-child-import-coordinate-boundary` | The provider aggregate contains an import-coordinate placeholder; coordinates are proof, not imported literal values. |
| `provider-aggregate-child-surface-not-exact-safe` | A nested provider aggregate child has a sequence surface that is not exact-tree-safe. |
| `provider-aggregate-child-call-boundary` | A provider aggregate child is a call expression without a supported imported-literal child contract. |
| `provider-sequence-surface-proof-missing` | The provider aggregate lacks the sequence-surface proof required for imported literal export. |
| `unsupported-provider-rhs-shape` | The provider RHS is not a literal, supported aggregate, or supported factory call. |

The #567 import-backed immutable provenance closeout is the reference example
for using this census to end a capability slice without widening admission. See [import-backed immutable provenance closeout](import-backed-immutable-provenance-closeout-567.md)
and the checked-in [closeout artifact](../bench/recall_loss/issue-567-closeout.v1.json).

## PR reporting

For any PR that changes exact semantic admission, include this table or the same
fields in prose:

| metric | before | after | note |
|---|---:|---:|---|
| false merges |  |  | Hard gate: must stay `0` on the selected verification surface. |
| canon-preservation violations |  |  | Hard gate: must stay `0`. |
| completeness percentage |  |  | Soft signal: explain meaningful movement. |
| under-merged behavior groups |  |  | Soft signal: increased misses need attribution. |
| oracle exclusions by reason |  |  | Soft signal: budget/path/uninterpretable growth needs a cause. |
| admission rejections by structured reason |  |  | Main recall-loss signal. |
| import snapshot misses by reason |  |  | Process signal for deciding the next imported-value capability slice. |
| top attributed recall-loss bucket |  |  | Name the follow-up capability, fixture, or unsupported boundary. |

Use `scripts/recall-loss-diff.py before.json after.json` for the before/after
table when both full local reports are available.

Hard gate:

- `false_merges == 0`;
- `canon_preservation_violations == 0`.

Soft regression gate:

- any increase in under-merged groups, oracle exclusions, or admission rejections
  should be attributed to a structured reason bucket;
- intentional fail-closed recall loss should name a follow-up capability,
  fixture, or unsupported boundary;
- recall gains should state which strict evidence or capability made them safe.

## Relationship to other diagnostics

- [`oracle-value-model`](oracle-value-model.md) explains the interpreter oracle,
  value model, and `--falsify` search.
- [`type4-adversarial-coverage`](type4-adversarial-coverage.md) explains how
  `nose verify --leads` becomes Type-4 target packets.
- [`semantic-pack-architecture`](semantic-pack-architecture.md) defines the
  product behavior gate for semantic-pack and semantic-kernel changes.
- [`recall-loss-recovery-loop`](recall-loss-recovery-loop.md) defines the
  checked-in baseline summaries, report diff workflow, and cycle contract.
- [`source-facts`](source-facts.md) and [`evidence-records`](evidence-records.md)
  define the evidence that future narrow admission-rejection buckets should
  reference.
