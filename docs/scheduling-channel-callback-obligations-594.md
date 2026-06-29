# Scheduling, channel, and callback obligations

Status: planning and reporting contract for issue
[#594](https://github.com/corca-ai/nose/issues/594).

This page defines the minimal cross-language obligation vocabulary for
scheduling boundaries, success/error channels, and callback demand/effect
surfaces. It is deliberately a capability vocabulary, not a feature list. New
language or stdlib support should map evidence and contracts onto these
obligations instead of adding selector-specific admission paths.

## Census

The starting evidence is [cross-language-boundary-census-594-2026-06-28.v1.json](../bench/recall_loss/cross-language-boundary-census-594-2026-06-28.v1.json).
It normalizes the existing JS/TS, Python, Rust, Go, Java, and Swift audit
reports and adds conservative Ruby/C lexical pricing. The report is
source-prevalence evidence only; it records `semantic_admission_delta = 0`.

Top obligation families in the census:

| family | occurrences | policy |
|---|---:|---|
| success/error result channel | 62,945 | already large because Rust `Option`/`Result` constructors are supported, but helper/default/callback forms still need receiver and channel proof |
| lifecycle/materialization boundary | 42,010 | iterators, views, factories, allocation, and materializers need source/receiver/domain proof before exact use |
| scheduling boundary | 28,751 | dominated by JS/TS `await` and async functions; keep closed until scheduling and channel obligations are explicit |
| ambiguous selector boundary | 19,116 | selector/property spelling needs receiver, symbol, domain, and occurrence proof |
| receiver mutation | 16,530 | mutation is effect evidence, not value equivalence |
| callback families combined | 18,637 | useful next producer target because it reuses existing HOF demand/effect contracts across languages |

The census recommendation is to design and report obligations first, then start
with callback demand/effect reporting before any broad async or channel
admission.

The first reporting slices are:

- [issue-599-callback-obligation-slice-2026-06-28.v1.json](../bench/recall_loss/issue-599-callback-obligation-slice-2026-06-28.v1.json) for callback demand/effect rows;
- [issue-600-channel-scheduling-obligation-slice-2026-06-28.v1.json](../bench/recall_loss/issue-600-channel-scheduling-obligation-slice-2026-06-28.v1.json) for channel and scheduling-boundary rows.

The first exact-admission decision is [issue-601-first-slice-closeout-2026-06-28.v1.json](../bench/recall_loss/issue-601-first-slice-closeout-2026-06-28.v1.json):
no slice is opened in this milestone because the measured candidates still lack
the complete callback, channel, scheduling, or aggregate-result obligations
needed for exact convergence.

The first follow-up diagnostics artifact is [callback-demand-effect-diagnostics-2026-06-28.v1.json](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v1.json).
It keeps exact admission closed but splits the local `crates` HOF
demand/effect rows by callback-effect proof, callback identity/shape proof, and
predicate callback profile.

The follow-up [callback-demand-effect-diagnostics-2026-06-28.v2.json](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v2.json) splits callback-effect proof further into callback call effects, callback assignment effects, and callback runtime boundaries.

The next [callback-demand-effect-diagnostics-2026-06-28.v3.json](../bench/recall_loss/callback-demand-effect-diagnostics-2026-06-28.v3.json)
keeps the same exact-admission boundary and splits callback call effects by
producer-facing call shape: member calls (`10`), Rust macro calls (`8`),
direct-function effect contracts (`3`), and imported-function effect contracts
(`1`) on `crates`.

The [promise-protocol-diagnostics-2026-06-28.v1.json](../bench/recall_loss/promise-protocol-diagnostics-2026-06-28.v1.json)
slice keeps Promise exact admission closed while making the JS/TS Promise/async
source-prevalence group (`29,094` occurrences) reportable as scheduling,
executor callback, rejection-channel, aggregate-result, factory, and
non-construct call obligations. JS/TS async functions now emit a fail-closed
`Source::Protocol(AsyncFunction)` boundary even when the body has no `await`.
The follow-up [promise-protocol-hard-negatives-2026-06-28.v1.json](../bench/recall_loss/promise-protocol-hard-negatives-2026-06-28.v1.json)
pins the Promise-specific hard negatives before any recovery slice opens:
async-function/sync, Promise executor/sync, Promise.resolve/sync,
Promise.then/custom receiver, possible or explicit thenable assimilation, and
Promise.all/Promise.race convergence all remain closed.
The first narrow recovery slice, [promise-resolve-recovery-2026-06-28.v1.json](../bench/recall_loss/promise-resolve-recovery-2026-06-28.v1.json),
lets dependency-closed `Promise.resolve(value)` enter exact semantic matching only
when `value` is proven non-thenable-safe, so the Promise factory boundary is
retained and broader scheduling, executor, aggregate, and rejection channels stay
closed.
The follow-up [promise-rejection-continuation-diagnostics-2026-06-28.v1.json](../bench/recall_loss/promise-rejection-continuation-diagnostics-2026-06-28.v1.json)
is reporting-only: `Promise.reject`, `.catch`, and `.finally` now have separate
missing-evidence labels for rejected-value channels, rejection continuations,
settlement continuations, and callback demand/effect obligations. It does not
admit Promise continuation equivalence.
The next reporting slice, [promise-then-obligation-diagnostics-2026-06-28.v1.json](../bench/recall_loss/promise-then-obligation-diagnostics-2026-06-28.v1.json),
splits `.then` itself into receiver proof, fulfillment continuation, rejection
continuation, and callback demand/effect obligations. Receiver proof is the
primary rollup because the JS/TS audit has `36/39` `.then` occurrences with
unhinted receivers.
The [promise-continuation-report-rows-2026-06-28.v1.json](../bench/recall_loss/promise-continuation-report-rows-2026-06-28.v1.json)
slice makes `.then`, `.catch`, and `.finally` visible as focused
`admission_rejections` while keeping exact continuation admission closed. Its
next recovery queue is receiver-first: `68` unhinted `.then`/`.catch` receiver
occurrences must be attributed before fulfillment, rejection, or callback
continuation recovery can be considered.
The first behavior-changing follow-up, [promise-local-continuation-recovery-2026-06-29.v1.json](../bench/recall_loss/promise-local-continuation-recovery-2026-06-29.v1.json),
opens only local first-party Promise continuations. It adds contract evidence
for `Promise.reject`, `.catch`, and two-argument `.then`; represents
fulfilled/rejected Promise states in the value graph; flattens handler-returned
`Promise.resolve`; and lets `Promise.reject(...).catch(handler)` converge with
`Promise.reject(...).then(undefined, handler)` when the producer and handler are
dependency-closed. Async/await scheduling, arbitrary thenables, custom
receivers, `.finally`, aggregate combinators, and sync payload equivalence stay
closed.
The next reporting-only follow-up, [promise-receiver-producer-diagnostics-2026-06-29.v1.json](../bench/recall_loss/promise-receiver-producer-diagnostics-2026-06-29.v1.json),
splits Promise continuation receiver producers without opening exact admission:
constructor receivers map to settlement-channel proof, async-function returns
map to scheduling proof, and generic call-return receivers remain ambiguous
callee/selector proof. The 120-repo JS/TS scan found `835` generic call-return
receivers, `49` same-file async-function call receivers, and only `2`
constructor receivers, so constructor exact semantics should not be the next
priority.
The follow-up [promise-call-return-callee-diagnostics-2026-06-29.v1.json](../bench/recall_loss/promise-call-return-callee-diagnostics-2026-06-29.v1.json)
splits the generic call-return bucket by callee shape. The revised 120-repo
scan found `932` member call-return candidates, `184` local/parameter
candidates, `105` imported-member candidates, `73` imported-binding candidates,
and `79` same-file async-function call candidates. Broad member recovery is the
largest surface but remains the riskiest; exact recovery should require both
callee identity and returned `PromiseLike` domain proof, with narrower
async/direct-return slices priced first.
The first such recovery slice is [promise-async-function-return-recovery-2026-06-29.v1.json](../bench/recall_loss/promise-async-function-return-recovery-2026-06-29.v1.json).
Same-file direct calls to source-proven async functions now provide
dependency-backed `PromiseLike` result-domain evidence, and pure
non-thenable-safe returned payloads can feed local `.then` fulfillment recovery.
This is still a producer proof, not broad scheduling equivalence: `await`,
throw/rejection paths, possible thenables, opaque call results, constructors,
imported/member call returns, `.finally`, and aggregate combinators remain
closed.
The follow-up [promise-direct-function-return-recovery-2026-06-29.v1.json](../bench/recall_loss/promise-direct-function-return-recovery-2026-06-29.v1.json)
opens the next producer-proof slice inside the `184` local/parameter
call-return candidates from the JS/TS corpus scan. A same-file direct function
call can now become a PromiseLike receiver only when direct callee evidence
points to a non-async single-return function and the returned expression already
has PromiseLike domain proof. This admits literal and typed non-thenable
`Promise.resolve` helper returns plus `Promise.reject` helper returns, while
parameter callees, member/imported call returns, unsafe thenables,
constructors, `.finally`, aggregate channels, and broad scheduling remain
closed.
The follow-up [promise-direct-method-return-recovery-2026-06-29.v1.json](../bench/recall_loss/promise-direct-method-return-recovery-2026-06-29.v1.json)
opens the proof-backed DirectMethod subset inside the `932` member
call-return candidates from the JS/TS corpus scan. A member call can become a
PromiseLike receiver only when an existing DirectMethod call-target record
points to a non-async single-return method and the returned expression already
has PromiseLike domain proof. The value graph evaluates only that returned
expression and closes on receiver-context reads, so selector-only member calls,
dynamic dispatch, imported members, unsafe thenables, constructors, `.finally`,
aggregate channels, and broad scheduling remain closed.
The follow-up [promise-imported-call-return-boundary-2026-06-29.v1.json](../bench/recall_loss/promise-imported-call-return-boundary-2026-06-29.v1.json)
keeps imported function/member receivers closed and renames their target-present
obligation to settled-value contracts. Import coordinates prove identity, not
payload recovery; exact Promise continuation recovery for imported producers
must first model fulfilled/rejected value channels explicitly.
The follow-up [promise-branch-return-producer-recovery-2026-06-29.v1.json](../bench/recall_loss/promise-branch-return-producer-recovery-2026-06-29.v1.json)
then broadens the local producer proof from single-return bodies to supported
branch-return bodies. DirectFunction and DirectMethod result-domain evidence can
depend on every returned expression on the supported paths, and the value graph
recovers only same-channel Promise Phi states. Mixed fulfilled/rejected
branches, selector-only members, parameter callees, imported receivers without
settled-value contracts, and broad scheduling remain closed.
The follow-up [promise-finally-settlement-recovery-2026-06-29.v1.json](../bench/recall_loss/promise-finally-settlement-recovery-2026-06-29.v1.json)
opens the exact-safe `.finally` subset without changing the broader scheduling
boundary. `Promise.finally` now has a builtin Promise contract and can recover
only when the receiver has admitted PromiseLike proof and the handler is absent
or a zero-argument lambda returning a non-thenable-safe value, a fulfilled
Promise boundary, or a rejected Promise boundary. Fulfilled finally handlers
preserve the original settlement, rejecting finally handlers move the result to
the rejected channel, and parameterized handlers, possible thenables,
selector-only receivers, imported producers without settled-value contracts,
aggregates, and broad async scheduling remain closed.
The follow-up [promise-imported-settled-value-contract-2026-06-29.v1.json](../bench/recall_loss/promise-imported-settled-value-contract-2026-06-29.v1.json)
adds the imported-producer capability needed after the imported call-return
boundary split. Imported call-target identity can now compose with admitted
`Domain(PromiseLike)`, Promise continuation API evidence, and builtin
`PromiseSettledValue` payload/channel proof. That opens only focused imported
`.then`/`.catch` fixtures whose fulfilled or rejected payload is explicitly
contracted; ordinary imported producers, possible fulfilled thenables,
selector-only members, aggregates, constructors, and broad scheduling remain
closed.
The Node `timers/promises` follow-ups reuse that same split. The ESM recovery
is recorded in the [ESM domain artifact](../bench/recall_loss/promise-node-timers-domain-recovery-2026-06-29.v1.json),
the [CommonJS domain artifact](../bench/recall_loss/promise-node-timers-commonjs-domain-recovery-2026-06-29.v1.json),
and the [safe payload artifact](../bench/recall_loss/promise-node-timers-safe-payload-recovery-2026-06-29.v1.json)
show ESM named imports and conservative `const` CommonJS destructuring requires
providing PromiseLike receiver-domain proof for `setTimeout`/`setImmediate`,
raising the priced slice from `82` to `97` call sites. Only the no-options
payload arities emit fulfilled `PromiseSettledValue`, and the current 120-repo
direct named-binding scan found `0` such safe-payload call sites. Options
objects, scheduler APIs, interval streams, namespace/default imports,
mutable/dynamic CommonJS shapes, possible thenables, and broad scheduling stay
closed.
The [promise-scheduling-closeout-2026-06-29.v1.json](../bench/recall_loss/promise-scheduling-closeout-2026-06-29.v1.json)
artifact closes this recovery cycle. It records that Promise reporting,
local producer recovery, imported settled-value contracts, and the bounded Node
timers slices are complete for this tranche. Aggregate combinators, executor
timing, cancellation/liveness, scheduler APIs, interval streams, and
cross-language async/channel/lifecycle models should move to a separate
capability epic, issue [#602](https://github.com/corca-ai/nose/issues/602),
instead of continuing as API-by-API Promise expansion.
The first #602 slice is the reporting-only [scheduling/lifecycle boundary audit](../bench/recall_loss/scheduling-lifecycle-boundary-audit-602-2026-06-29.v1.json).
It adds a 120-repo pricing script for scheduling, aggregate, cancellation,
channel, executor, lifecycle, and exception surfaces without opening exact
admission. The scan prices `142,844` source-prevalence occurrences and ranks
the first next-work targets as Promise aggregates, executor timing,
AbortSignal cancellation, interval lifecycle, Go goroutines, Java
`CompletableFuture`, and Swift `await`. The local `crates` gate remains
`false_merges == 0` and `canon_preservation_violations == 0`.
The first exact #602 follow-up is [Promise.all literal aggregate recovery](../bench/recall_loss/promise-all-literal-aggregate-recovery-2026-06-29.v1.json).
It satisfies only the all-fulfilled aggregate obligation for unshadowed
`Promise.all` calls with literal array arguments whose elements already recover
as fulfilled Promise boundaries. The result remains a Promise boundary carrying
an ordered sequence payload. First-settled, first-fulfilled,
rejection ordering, dynamic iterable lifecycle, thenable assimilation,
executor timing, and cancellation/liveness obligations remain named closed
boundaries.
The next exact #602 follow-up is [Promise.allSettled literal aggregate recovery](../bench/recall_loss/promise-allsettled-literal-aggregate-recovery-2026-06-29.v1.json).
It satisfies the all-settled aggregate obligation only for unshadowed
`Promise.allSettled` calls with literal array arguments whose elements already
recover as fulfilled or rejected Promise boundaries. The aggregate result is a
fulfilled Promise carrying ordered settled-record payloads, so fulfilled and
rejected element channels stay distinct without converging with synchronous
record arrays. Dynamic iterable lifecycle, first-settled, first-fulfilled,
executor timing, and cancellation/liveness obligations remain closed.
The shared [Promise aggregate raw-input assimilation](../bench/recall_loss/promise-aggregate-raw-input-recovery-2026-06-29.v1.json)
follow-up reuses the existing non-thenable-safe proof from `Promise.resolve`:
raw literal/scalar elements can become fulfilled aggregate inputs for already
admitted literal `Promise.all` and `Promise.allSettled` calls. This is still not
thenable assimilation. At that slice, object/function raw inputs, untyped
variables, dynamic iterables, `Promise.race`, `Promise.any`, executor timing,
and sync aggregate equivalence remained closed.

The [Promise.race/Promise.any literal aggregate recovery](../bench/recall_loss/promise-race-any-literal-aggregate-recovery-2026-06-30.v1.json)
follow-up opens only the first-observed subset that can be expressed with the
same aggregate-settlement capability. `Promise.race` admits non-empty literal
arrays only when every element has closed settlement or non-thenable-safe
raw-input proof, then preserves the first element's fulfilled/rejected channel.
`Promise.any` admits fully closed literal arrays only when at least one element
is fulfilled, then preserves the first fulfilled payload. Dynamic iterables,
possible thenables, all-rejected `Promise.any` AggregateError payloads,
executor timing, cancellation/liveness, and sync value equivalence remain
closed.

The [Promise executor boundary audit](../bench/recall_loss/promise-executor-boundary-audit-2026-06-30.v1.json)
is the next reporting-only #602 slice. It does not admit constructor
settlement recovery. Instead it prices `795` `new Promise(...)` occurrences and
splits the executor queue by inline shape, resolve/reject calls,
timer/scheduler use, multi-settlement, throw-to-rejection, side-effect calls,
and possible thenable payload risks. Only `27` scalar resolve and `4` scalar
reject direct single-settlement occurrences are lexical upper bounds for a
future exact slice; even those remain closed until executor timing, callback
identity, settlement precedence, thrown-error ordering, callback effects, and
non-thenable payload proof are represented.

The [AbortSignal cancellation boundary audit](../bench/recall_loss/abort-signal-cancellation-boundary-audit-2026-06-30.v1.json)
is the following reporting-only #602 slice. It adds named runtime-boundary
labels for `AbortSignal.abort`, `AbortSignal.any`, `AbortSignal.timeout`, and
`new AbortController()` without admitting cancellation equivalence. The 120-repo
scan splits `260` Abort mentions into controller lifecycle, direct static
AbortSignal calls, signal option properties, and signal-aware `fetch`,
timer, listener, and scheduler surfaces. Exact cancellation remains closed until
signal identity, abort ordering, abort reasons, rejection/cleanup behavior, and
controller-signal lifecycle are dependency-closed obligations.

The [interval/scheduler lifecycle boundary audit](../bench/recall_loss/interval-scheduler-lifecycle-boundary-audit-2026-06-30.v1.json)
is the next reporting-only #602 slice. It adds named runtime-boundary labels for
global timer scheduling, `scheduler.wait` timing plus cancellation/liveness,
`scheduler.yield` microtask ordering, `setInterval` repeated-emission
lifecycle, interval cancellation, and one-shot timer/frame cancellation. The
120-repo scan prices `780`
`setTimeout` calls, `57` `setImmediate` calls, `73` bare `setInterval` calls,
`55` `clearInterval` calls, `133` `clearTimeout` calls, `14`
`queueMicrotask` calls, `43` `requestAnimationFrame` calls, and `11`
`scheduler.yield` calls. Exact
scheduling remains closed until callback identity, callback effects, ordering,
cardinality, and cancellation cleanup are dependency-closed obligations.

## Minimal Vocabulary

| obligation | existing substrate | exact-channel rule |
|---|---|---|
| synchronous callback | `DemandEffectProfile` per-element/eager profiles, `LibraryApi` HOF rows, `CallTarget` when needed | callback identity, receiver/domain proof, arity, result role, and effect visibility must be explicit |
| lazy callback | pull-lazy HOF profiles, iterator producer/source proof, terminal materialization proof | construction must not expose callback effects or exceptions unless a terminal demand proves observation |
| eager callback | eager HOF profiles and source comprehension facts | callback errors/effects may be visible only when the API occurrence and receiver/source evidence prove eager demand |
| effect-only callback | HOF/effect contracts plus `Effect` evidence | ignored callback results must not be consumed as value equivalence; observable effects must stay ordered |
| reduction callback | fold/reduction demand profiles | accumulator identity, callback result role, initial value, and effect order must be represented |
| executor callback | source/API occurrence plus async/protocol demand profile | executor timing and thrown/rejected outcomes must be represented before any producer/factory convergence |
| success/error result channel | `Domain(Option/Result/FutureLike/PromiseLike)`, constructor/predicate rows, default contracts | success, empty, default, error, panic, and rejection channels must remain distinct |
| exception channel | `Source::Protocol`, static-error control, effect-free throw checks | thrown/rescued/non-local control must not be collapsed into ordinary return values |
| rejection channel | Promise/Future-like contracts and async demand profiles | rejected values, catch/then continuations, finally settlement, aggregate rejection, and thenable assimilation stay closed until proven |
| scheduling boundary | `DemandOperation::AsyncContinuation`, `GeneratorSuspension`, `ChannelOperation`, `ProtocolBoundary` | task/thread/goroutine/microtask timing is not synchronous equivalence proof |
| cancellation/early exit | short-circuit demand profiles and future protocol facts | cancellation, stop, break, first-settled, and early-exit behavior must be explicit |
| lifecycle/materialization | `SequenceSurface`, `Domain`, iterator adapter/materializer rows | one-shot views, reusable collections, type-directed materializers, and allocation/lifetime are separate |
| receiver mutation | `Effect(ReceiverMutation)`, place/effect contracts | mutation can close later exact receiver use; it does not create pure value equality |
| ambiguous selector | `Symbol`, `Import`, `CallTarget`, `Domain`, `LibraryApi` occurrence proof | a method/property/function name is only a selector until all required evidence is dependency-closed |

## Existing Mapping

- `DemandEffectProfile` already carries eager, fold, short-circuit, lazy HOF,
  async continuation, generator, channel, and protocol-boundary timing. This is
  the contract side of the vocabulary.
- `Source` facts anchor syntax/protocol distinctions such as await, async
  functions, yield, casts, calls, comprehensions, ranges, and patterns. They do
  not approve exact clones by themselves.
- `Effect` facts cover builder append, binding writes, receiver mutation, fixed
  self-field writes, non-overloadable index writes, and opaque argument escape
  risks. They are used to close unsafe paths or prove narrow place/effect
  contracts.
- `LibraryApiContract` rows own admitted API occurrences and attach result,
  receiver, source, shadowing, demand, and effect obligations.
- `CallTarget` and `Symbol` facts prove identity. They are required before a raw
  callee or imported/member selector can mean a stable operation.
- `Domain` and `SequenceSurface` facts prove receiver/result shape. They prevent
  selector-only collection, map, iterator, option/result, Promise/Future, and
  materializer admission.

This mapping means #594 does not need a new public pack API first. The next code
work should improve reporting and producers that attach these existing concepts
to specific obligation buckets.

## Language Mapping

| language | current #594 surfaces | first safe direction |
|---|---|---|
| JS/TS | `await`, async functions, Promise executor/combinators/rejection, Array HOFs, mutations | report scheduling/rejection/executor separately; keep broad async convergence closed |
| Python | builtins `map`/`filter`, `itertools`, `functools`, decorators, materializers | callback/lifecycle reporting, then narrow producer evidence for already admitted iterator builtins |
| Rust | iterator HOFs, `Option`/`Result`, mutation/effect, iterator views | reuse lazy callback and channel vocabulary; keep type-directed `collect` and mutating APIs closed |
| Go | `sort`/`slices`/`maps`, mutation callbacks, channel/goroutine surfaces for future scans | add effect/callback reporting before exact sort or goroutine/channel semantics |
| Java | `Arrays`/`Collections`, Optional/Future/Stream-shaped domains, mutation/wrapper APIs | split receiver mutation, wrapper aliasing, channel, and stream callback obligations |
| Swift | Sequence HOFs, cardinality, mutation, views, reductions, `throws`/`async` future work | reuse callback/effect and lifecycle buckets; keep selector-only collection methods closed |
| Ruby | Enumerable blocks, `raise`/`rescue`, Thread/Fiber surfaces | block timing and exception-channel reporting before expanding Enumerable support |
| C | callback comparators, allocation/lifetime, memory mutation, `errno`, non-local jumps, threads | keep pointer/lifetime and mutation separate from callback/error-channel evidence |

## Hard-Negative Classes

Every behavior-changing leaf under #594 must include adjacent hard negatives for
the relevant families:

- synchronous callback vs deferred/asynchronous callback;
- callback invoked zero, one, many, or unknown times;
- callback result consumed vs ignored;
- callback side effect visible before vs after the surrounding expression;
- success value vs empty/default/error/exception/rejection channel;
- aggregate success vs first-success, first-error, or first-settled behavior;
- cancellation, early exit, throw, panic-like, or non-local jump;
- receiver mutation vs pure factory/materializer/view;
- one-shot iterator/view/stream reuse vs reusable collection;
- shadowed, ambiguous, wrong-language, or dependency-broken selector evidence.

The current inventory is checked in as [issue-598-hard-negative-inventory-2026-06-28.v1.json](../bench/recall_loss/issue-598-hard-negative-inventory-2026-06-28.v1.json).
It maps existing tests and checked-in audit reports to these families before any
new exact admission is opened.
The #602 reporting slices extend that map in the [scheduling/lifecycle boundary audit](../bench/recall_loss/scheduling-lifecycle-boundary-audit-602-2026-06-29.v1.json),
the [Promise executor boundary audit](../bench/recall_loss/promise-executor-boundary-audit-2026-06-30.v1.json),
the [AbortSignal cancellation boundary audit](../bench/recall_loss/abort-signal-cancellation-boundary-audit-2026-06-30.v1.json),
and the [interval/scheduler lifecycle boundary audit](../bench/recall_loss/interval-scheduler-lifecycle-boundary-audit-2026-06-30.v1.json):
thenable/custom Promise receivers and Promise aggregate distinctions stay pinned
by existing semantic-boundary tests, while executor timing, scheduler ordering,
interval liveness, and cancellation-sensitive AbortSignal forms are
reporting-only until behavior-changing hard negatives exist.

The [#602 closeout](../bench/recall_loss/issue-602-closeout-2026-06-30.v1.json)
marks this milestone complete as a broad boundary model rather than an API
expansion. Literal Promise aggregate slices are the only exact admissions opened
under #602; executor, cancellation, scheduler, timer, interval, and
cross-language lifecycle surfaces remain named closed obligations for future
epics.

The post-#602 [cross-language await obligation reporting](../bench/recall_loss/cross-language-await-obligation-reporting-2026-06-30.v1.json)
artifact records that async/await reporting uses the language-neutral
`async-await-scheduling-contract` label for `Source::Protocol(Await)` across
JS/TS, Python, Rust, and Swift. Legacy checked Promise diagnostics may still
mention `promise-await-scheduling-contract`, but new recall-loss reports should
reserve Promise-specific labels for Promise API/producer semantics, not for the
shared await protocol boundary.
The follow-up [oracle-exclusion obligation reporting](../bench/recall_loss/oracle-exclusion-obligation-reporting-2026-06-30.v1.json)
keeps the same capability vocabulary visible when a runtime/protocol unit is
excluded before oracle interpretation. Await-only JS/TS, Python, Rust, and
Swift fixtures now report under `oracle_exclusions.by_obligation` as
`scheduling-boundary/async-await-scheduling-contract-missing`; exact admission
and top-level interpretable `by_obligation` remain unchanged.

## Non-API Statement

This page does not add a public semantic-pack API. The vocabulary should narrow
future pack schema names only after the internal reporting and producer paths
are proven. Until then, packs continue to emit facts and contracts; the kernel
continues to decide whether dependency-closed obligations are sufficient for
exact admission.

## See Also

- [demand-effect-semantics](demand-effect-semantics.md) explains the existing demand/effect substrate that #594 reuses.
- [evidence-records](evidence-records.md) defines the evidence rows that carry proof into the kernel.
- [recall-loss-recovery-loop](recall-loss-recovery-loop.md) records the measured recovery process and checked-in baselines.
- [semantic-pack-architecture](semantic-pack-architecture.md) sets the pack/kernel responsibility boundary for exact admission.
- [semantic-kernel-roadmap](semantic-kernel-roadmap.md) tracks remaining demand-aware semantic work.
