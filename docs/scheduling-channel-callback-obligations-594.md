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
The follow-up [non-js-async-runtime-api-obligation-reporting-2026-06-30.v1.json](../bench/recall_loss/non-js-async-runtime-api-obligation-reporting-2026-06-30.v1.json)
extends the same reporting-only process to non-JS runtime APIs without adding a
new kernel enum: Python `asyncio` task/timer/aggregate calls, Rust async spawn
and qualified `tokio`/`futures`/`futures_util` `join!`/`select!` macros, and
Swift `Task` creation now report shared `task-*` and `async-aggregate-*`
obligations. The matching 120-repo audit prices
Rust async spawn at `349` occurrences / `3` repos, Swift `Task` at `210` / `12`,
Python `asyncio.sleep` at `104` / `6`, qualified Rust
`tokio`/`futures`/`futures_util` `join!`/`try_join!` at `68` / `2`, Python
`asyncio.gather` at `17` / `4`, Python `asyncio.create_task`/`ensure_future`
at `14` / `3`, qualified Rust `tokio`/`futures`/`futures_util` `select!` at
`5` / `1`, and Python `asyncio.wait` at `4` / `3`. The follow-up [non-js-async-runtime-attribution-hardening-2026-06-30.v1.json](../bench/recall_loss/non-js-async-runtime-attribution-hardening-2026-06-30.v1.json)
keeps exact admission closed while requiring import-backed Python `asyncio`
with no path-visible local module, qualified Rust spawn/aggregate paths whose
root is not locally defined in the same file, and unshadowed Swift `Task` roots
with no corpus-visible Swift `Task` definition before those shared obligations
are attributed. The next [non-JS async runtime import-proof artifact](../bench/recall_loss/non-js-async-runtime-import-proof-2026-06-30.v1.json)
keeps the same capability boundary but reuses existing imported namespace and
imported member proof so Python `asyncio` aliases and Rust imported runtime
bindings receive the same reporting-only obligations. Its matching [120-repo
pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-import-proof-2026-06-30.v1.json)
adds `11` Rust imported-binding occurrences over the qualified-only pricing and
records `0` Python `asyncio` alias occurrences in the pinned corpus.
The next [imported-binding proof artifact](../bench/recall_loss/non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json)
keeps exact admission closed while extending that same capability to Python
`from asyncio import ...` bindings and Rust brace imports backed only by
binding evidence. Its matching [120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json)
adds `2` Python imported `asyncio.sleep` occurrences over the prior artifact;
Rust direct and brace imported runtime rows stay at `11` priced occurrences,
but brace evidence-only imports are now actionable by the reporter.
The follow-up [Python asyncio sleep reporting artifact](../bench/recall_loss/python-asyncio-sleep-reporting-2026-07-02.v1.json)
aligns the original direct `asyncio.sleep` timer row with the same
runtime-boundary reporting capability. The 120-repo audit moves `104`
occurrences across `6` repos to reporting-supported closed-boundary status and
leaves no Python closed-boundary rows in the scheduling lifecycle audit.
The follow-up [Ruby exception-channel reporting artifact](../bench/recall_loss/ruby-exception-reporting-2026-07-02.v1.json)
applies the existing `Throw`/`Try` runtime-boundary capability to Ruby exception
flow. Unqualified `raise` calls now report exception-channel obligations,
`rescue` remains a `Try` boundary, and the old broad `raise/rescue` lexical row
is superseded by concrete reporting-supported rows while receiver-qualified
`.raise` overlaps remain outside the concrete rows.
The follow-up [Swift structured-concurrency artifact](../bench/recall_loss/swift-structured-concurrency-obligation-reporting-2026-06-30.v1.json)
keeps that same capability boundary and maps Swift `Task.sleep`, `Task.yield`,
and task-group calls onto shared timer, task-yield, aggregate,
cancellation/liveness, result-channel, and exception-channel obligations. Its
matching [120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-swift-structured-concurrency-2026-06-30.v1.json)
raises total source prevalence from `142,847` to `143,178`: `Task.sleep`
contributes `161` occurrences / `10` repos, task groups `153` / `9`,
`Task.yield` `12` / `3`, and the audit now counts `5` already-supported
`Task.detached(...)` spawn occurrences. Exact admission remains closed.
The follow-up [Rust block_on future-drive artifact](../bench/recall_loss/rust-block-on-future-drive-obligation-reporting-2026-07-01.v1.json)
keeps exact admission closed and maps qualified/import-backed Rust
`tokio_test::block_on` calls and proof-backed tokio runtime receiver chains
onto `future-drive-scheduling-contract` plus
`future-settled-value-channel-contract`. This is a reusable scheduling/result
capability, not selector-name admission: arbitrary `.block_on` receivers remain
closed without tokio runtime identity proof. The follow-up [Rust local runtime
provenance artifact](../bench/recall_loss/rust-block-on-local-runtime-provenance-2026-07-01.v1.json)
extends that capability to local variables whose last visible assignment is a
proof-backed `Handle::current()`, `Runtime::new().unwrap()/expect/?`, or
`Builder::new_*().build().unwrap()/expect/?` chain. The follow-up [Rust
parameter runtime provenance artifact](../bench/recall_loss/rust-block-on-parameter-runtime-provenance-2026-07-01.v1.json)
reuses the same capability for nominal `tokio::runtime::Runtime`/`Handle`
parameter receivers backed by fully qualified type text or exact
scope-visible imported-binding evidence. The follow-up [Rust nested brace
runtime provenance artifact](../bench/recall_loss/rust-nested-brace-runtime-provenance-2026-07-01.v1.json)
extends the import-evidence side of that capability to nested static brace
imports such as `use tokio::{runtime::{Runtime}}`. The follow-up [Rust
self-field runtime provenance artifact](../bench/recall_loss/rust-self-field-runtime-provenance-2026-07-01.v1.json)
reuses the same receiver type-provenance capability for exact
`self.<field>.block_on(...)` receivers whose same-scope struct field declaration
proves `tokio::runtime::Runtime` or `Handle`; Tokio `sync_bridge.rs` moves from
`0` to `13` future-drive oracle exclusions with `0` false merges. Non-self
fields, local struct fields, project-local `tokio` roots or aliases including
raw-identifier spellings, wildcard/relative imports, type aliases, wrappers,
and constructor-assigned fields remain closed in that slice. The follow-up [Rust local self-field runtime provenance artifact](../bench/recall_loss/rust-local-self-field-runtime-provenance-2026-07-01.v1.json)
reuses the same capability for function/block-local `struct` plus local `impl`
declarations. Tokio `task_local_set.rs` moves the local
`self.rt.block_on(...)` row from
`receiver-mutation/effect-preserving-contract-missing` to
`scheduling-boundary/future-drive-scheduling-contract-missing` with `0` false
merges; duplicate local structs, wrong local `Runtime` imports, same-scope
`Runtime` types, namespace aliases named `tokio`, non-self fields, type aliases,
wrappers, and constructor-assigned fields remain closed. The follow-up [Rust map_err
runtime provenance artifact](../bench/recall_loss/rust-block-on-map-err-runtime-provenance-2026-07-01.v1.json)
opens only success-channel-preserving `Result::map_err` adapters over already
proven `Runtime::new()` or `Builder::build()` results; wrapper-returned Results
and non-Result `map_err` calls stay closed. The follow-up [Rust Builder config
runtime provenance artifact](../bench/recall_loss/rust-block-on-builder-config-runtime-provenance-2026-07-01.v1.json)
reuses the same receiver-provenance capability for receiver-preserving Tokio
Builder configuration methods such as `start_paused`, `unhandled_panic`,
`thread_keep_alive`, `global_queue_interval`, `event_interval`, and
`disable_lifo_slot`. The pinned corpus has `34` such occurrences across `15`
files in `tokio`; callback hooks and exact block_on/await convergence stay
closed.
The follow-up [Java CompletableFuture artifact](../bench/recall_loss/java-completablefuture-obligation-reporting-2026-06-30.v1.json)
keeps exact admission closed and maps proof-backed Java
`CompletableFuture.supplyAsync`/`runAsync`, settled factories, `allOf`/`anyOf`,
and exact-import-backed CompletionStage-style receiver continuations onto
reusable future, task, aggregate, callback, and exception obligations. Its
matching [120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-java-completablefuture-2026-06-30.v1.json)
splits `40` lexical Java future reporting candidates out of the broad
`CompletableFuture` bucket: `14` settled factories / `2` repos, `12`
async factories / `4` repos, `10` settlement continuations / `2` repos, and
`4` `allOf` calls / `2` repos. Broad `CompletableFuture` mentions remain
closed at `276` occurrences, and exact recovery still requires dependency-closed
executor timing, callback identity/effects, exceptional completion, and result
channel contracts.
The follow-up [Java Executor/Future local/this-field artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-java-local-this-field-executor-future-2026-07-01.v1.json)
keeps exact admission closed while extending the same receiver-domain capability
to exact-import-backed `CompletableFuture`, `Future`, `ScheduledFuture`,
`Executor`, `ExecutorService`, and `ScheduledExecutorService` parameter, local
variable, and explicit `this.<field>` receivers. It follows the earlier
parameter-only [receiver artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-java-executor-future-2026-07-01.v1.json)
without increasing the lexical corpus price: the same declaration-backed
receiver-method candidates are now implemented for local and explicit field
anchors instead of parameter anchors only.
`CompletableFuture`/`Future` handle methods (`get`, `cancel`, `isCancelled`,
and `isDone`) now report settled-value, exception, cancellation/liveness, and
handle-lifecycle obligations; executor `execute`, `submit`, `invokeAll`,
`invokeAny`, `schedule`, and repeating schedule calls now report task
scheduling, timer/interval lifecycle, aggregate, callback/effect,
future-settled, cancellation/liveness, and exception obligations according to
the proven receiver type. The 120-repo pricing artifact adds `858`
reporting-supported Java receiver-method candidates: `192` `Future.get`, `184`
cancel/status-cancellation calls, `166` `Executor.execute`, `146`
`ExecutorService.submit`, `106` `Future.isDone`, `21` scheduled timers, `20`
repeating schedules, `19` `invokeAll`, and `4` `invokeAny`. The broad
`Executor/Future` lexical bucket remains closed at `3,297` occurrences until
wrapper aliases, project-specific executors, callback identity/effects, result
channels, cancellation, and lifecycle contracts are dependency-closed.
The follow-up [Java wildcard Executor/Future artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-java-wildcard-executor-future-2026-07-01.v1.json)
keeps the same exact-admission boundary and extends receiver-domain provenance
from exact imports to `import java.util.concurrent.*`. The frontend emits
wildcard-derived import-symbol evidence for the supported concurrent types, and
the verifier still rejects local type declarations or explicit same-name imports
from other packages before reporting obligations. On the current `origin/main`
baseline, the pinned corpus moves Java reporting-supported receiver-method
candidates from `858` to `1,093` (`+235`): `Future.get` `192 -> 249`,
cancel/status-cancellation `184 -> 239`, `ExecutorService.submit` `146 -> 222`,
`Executor.execute` `166 -> 180`, `Future.isDone` `106 -> 127`, scheduled timers
`21 -> 27`, repeating schedules `20 -> 22`, `invokeAll` `19 -> 21`, and
`invokeAny` `4 -> 6`. The broad `Executor/Future` lexical bucket remains closed
at `3,297` occurrences.
The follow-up [Java Future residual-accounting artifact](../bench/recall_loss/java-future-residual-accounting-2026-07-02.v1.json)
aligns the audit with the already implemented receiver-domain continuation
reporting: `FutureLike.handle/whenComplete` contributes `10` reporting-supported
settlement-continuation occurrences across `2` repos. The broad
`Executor/Future` lexical bucket is still visible at `3,297` occurrences, but it
is now a superseded overlap row rather than an actionable implementation target;
concrete static call, constructor, and receiver-method rows drive the Java
Future/Executor residual queue.
The follow-up [Java CompletableFuture receiver split artifact](../bench/recall_loss/java-completablefuture-receiver-split-2026-07-02.v1.json)
keeps using those shared FutureLike obligations for receiver-specific
`CompletableFuture` methods. `complete`/`completeExceptionally` now report
manual settlement and exception/lifecycle obligations; `join`, `getNow`, and
`isCompletedExceptionally` report settled-value, exception, lifecycle, and
cancellation/liveness observation; timeout methods add timer-backed settlement
obligations. The scope-aware audit prices `45` settlement and `45` observation
occurrences, keeps same-name receivers outside the proven scope closed, and
moves the old `230` broad type/reference mentions out of the actionable
closed-boundary queue as a superseded overlap row.
The follow-up [Java stream lifecycle split artifact](../bench/recall_loss/java-stream-lifecycle-split-2026-07-02.v1.json)
applies the same accounting discipline to stream-shaped domains. Existing
iterator identity/static collection adapter proof already supports `372`
typed `receiver.stream()` occurrences and `128` exact-import or fully qualified
`Arrays.stream(xs)` occurrences in the pinned corpus. Those are now tracked as
exact-supported audit rows, while the broad `stream/parallelStream` lifecycle
residual falls from `1,996` to `1,496` occurrences and remains closed until
untyped receiver, factory-result, arity/range overload, shadowed binding,
terminal materialization, and parallel stream scheduling semantics are proven.
The follow-up [Go channel protocol pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-go-channel-protocol-2026-06-30.v1.json)
keeps exact admission closed while making Go's source-backed protocol
boundaries reportable at the same capability level. Channel sends now report
send synchronization obligations; receives report value-channel obligations;
comma-ok receives additionally report close/status obligations; `select`
parents, cases, and defaults report readiness, case-selection, and default
liveness obligations; goroutines and `defer` report callback-effect
obligations alongside their scheduling/lifecycle boundaries. The 120-repo
protocol-node pricing records `4,294` channel receives, `1,525` sends, `155`
comma-ok receives, `1,920` select parents, `3,590` select cases, `546` select
defaults, `1,949` goroutines, and `17,521` defers. Exact recovery remains
closed until channel blocking, close/zero-value behavior, select readiness,
callback effects, panic/defer ordering, and goroutine scheduling are
dependency-closed.
The follow-up [Go select receive-status artifact](../bench/recall_loss/go-select-receive-status-protocol-2026-07-01.v1.json)
keeps the same capability boundary and fills the status projection inside
`select` communication cases: `case _, ok := <-ch` now carries the existing
`channel-receive-status-contract` alongside select readiness/case obligations.
The pinned corpus has `107` lexical select comma-ok receive hits across `57`
files and `7` repos. Exact select/channel recovery remains closed.
The follow-up [Ruby Thread/Fiber runtime artifact](../bench/recall_loss/ruby-thread-fiber-runtime-reporting-2026-07-01.v1.json)
keeps exact admission closed while mapping Ruby `Thread.new`, `Thread.start`,
`Thread.fork`, `Fiber.new`, and `Fiber.schedule` onto the existing
task-spawn, task-handle, cancellation/liveness, and concurrency scheduling
vocabulary. Same-file `Thread`/`Fiber` definitions keep attribution closed. Its
matching
[120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-ruby-thread-fiber-runtime-2026-07-01.v1.json) marks the Ruby Thread/Fiber row
reporting-supported, prices `74` occurrences across `11` repos, and raises
total source prevalence from `146,987` to `146,988` by adding `Thread.start`.
The follow-up [Ruby yield source-protocol artifact](../bench/recall_loss/ruby-yield-source-protocol-reporting-2026-07-01.v1.json)
reuses the source-protocol boundary capability for Ruby block `yield` without
adding a Ruby-specific exact admission path or widening generator-yield
semantics. `yield a, b` now uses `Source::Protocol(BlockYield)` and stays
distinct from ordinary multiple-value `return a, b` until block identity,
callback argument/result role, effect visibility, non-local control, and
exception behavior are proven. The 120-repo audit prices `801` Ruby yield
occurrences across `17` repos and marks the row reporting-supported
closed-boundary.

The follow-up [Go protocol reporting-support artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-go-protocol-reporting-support-2026-07-01.v1.json)
keeps exact admission closed while aligning Go protocol reporting with the
existing source-backed lowering and runtime obligations. Go `go` now carries a
scheduled callback demand/effect profile, `defer` carries a scope-exit deferred
callback profile, and channel/select protocol rows are marked
reporting-supported closed-boundaries. The 120-repo audit keeps the same Go
source prevalence: `17,521` defers, `4,294` channel receives, `3,590` select
cases, `1,949` goroutines, `1,920` select parents, `1,525` channel sends, `546`
select defaults, and `155` comma-ok receives.
The follow-up [non-JS async runtime scope-shadowing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-scope-shadowing-2026-06-30.v1.json)
keeps the same reporting-only boundary while making Python/Rust runtime
attribution scope-aware. Python `asyncio` aliases/imported bindings and Rust
imported runtime bindings now ignore unrelated local shadows in other functions,
but same-scope, enclosing-scope, and module-level shadows still close reporting.
The 120-repo pricing total stays unchanged at `146,880`, so this hardens report
provenance without adding a new API-specific kernel feature or opening exact
admission. Python/Rust async-runtime diagnostics now prefer source-preserving
unit roots before normalized fallback, keeping Python `asyncio` alias shadow
decisions tied to lexical source evidence.
The follow-up [non-JS async runtime breadth artifact](../bench/recall_loss/non-js-async-runtime-breadth-2026-07-01.v1.json)
keeps exact admission closed while adding Python `asyncio.run`,
`wait_for`, `shield`, `run_coroutine_threadsafe`, and `to_thread`, plus Swift
checked/unsafe continuation bridges, to the same shared obligation vocabulary.
The matching [120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-breadth-2026-07-01.v1.json)
raises total source prevalence from `146,880` to `146,987`: Python contributes
`34` newly priced helper occurrences across `7` repos, and Swift continuation
bridges contribute `73` occurrences across `8` repos. The slice reuses
future-drive, timer, task, future-settled, future-settlement, future-callback,
cancellation/liveness, and exception-channel obligations; no new kernel API or
exact admission path is opened.
The follow-up [Python async protocol lifecycle artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-python-async-lifecycle-2026-07-01.v1.json)
keeps the same capability-first policy for source syntax rather than library
selectors. Python `async for` statements and comprehensions now report async
iteration lifecycle, value-channel, and scheduling obligations, while
`async with` reports async context lifecycle, cleanup, exception-channel, and
scheduling obligations. Exact admission remains closed; the 120-repo audit
prices `114` `async for` and `361` `async with` occurrences across `5` repos
with `0` false merges on the checked `crates` gate.
The follow-up [Swift async iteration artifact](../bench/recall_loss/swift-async-iteration-protocol-reporting-2026-07-01.v1.json)
reuses that same source-protocol capability for Swift `for await` and
`for try await` loops. Swift async sequence loops now report async iteration
lifecycle, value-channel, and scheduling obligations; throwing async loops also
preserve the existing exception-channel obligation through a separate `try`
source-protocol fact. The 120-repo audit prices `193` Swift async iteration
occurrences across `11` repos, and representative Swift NIO, Composable
Architecture, and Alamofire spot checks move async-iteration lifecycle evidence
units from `0` to `31` with `0` false merges. Exact async-sequence recovery
remains closed.
The follow-up [Swift async task source-protocol artifact](../bench/recall_loss/swift-async-task-source-protocol-2026-07-01.v1.json)
keeps exact admission closed while extending source syntax reporting to Swift
async closures and `async let`. Async closures reuse the existing
`AsyncFunction` protocol boundary; `async let` adds the reusable
`TaskSpawn` source protocol capability and maps it onto task-spawn scheduling,
task-handle lifecycle, and cancellation/liveness obligations rather than a
Swift-only feature row. The 120-repo audit prices `100` async closures across
`4` repos and `51` async-let bindings across `7` repos. Alamofire/Swift
NIO/Vapor spot checks move `task_spawn` raw protocol tags from `0` to `36` and
async-function tags from `110` to `139` with `0` false merges.
The follow-up [Swift throwing callable artifact](../bench/recall_loss/swift-throwing-callable-protocol-reporting-2026-07-01.v1.json)
keeps exact admission closed while extending the existing `TryPropagation`
source protocol to body-bearing plain and typed `throws`/`rethrows` functions
and throwing closures. Async throwing callables now report scheduling and
exception-channel obligations together instead of losing the declaration-level
error channel when the body has no explicit `try`. The 120-repo audit prices
`7,008` throwing functions across `17` repos and `169` throwing closures across
`6` repos; the broad `throws`/`try` bucket remains closed at `26,608`
occurrences.
The follow-up [Rust async closure source-protocol artifact](../bench/recall_loss/rust-async-closure-source-protocol-2026-07-01.v1.json)
applies the same async callable boundary to Rust `async |...|` and
`async move |...|` closures without adding a Rust-only feature. Rust async
closures reuse `AsyncFunction`; Rust `async { ... }` blocks remain the separate
`AsyncBlock` protocol surface. The pinned 120-repo audit has `0` Rust async
closure occurrences, so this is a parity/hard-negative slice, while the audit
now keeps the `1,342` Rust async-block occurrences across `4` repos distinct
from closure syntax.
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
Same-file direct calls to JS/TS source-proven async functions now provide
dependency-backed `PromiseLike` result-domain evidence, and pure
non-thenable-safe returned payloads can feed local `.then` fulfillment recovery.
Non-JS async functions remain scheduling/protocol facts only. This is still a
producer proof, not broad scheduling equivalence: `await`, throw/rejection
paths, possible thenables, opaque call results, constructors, imported/member
call returns, `.finally`, and aggregate combinators remain closed.
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
| scheduling boundary | `DemandOperation::AsyncContinuation`, `GeneratorSuspension`, `CallbackInvocation`, `ChannelOperation`, `ProtocolBoundary` | task/thread/goroutine/microtask timing is not synchronous equivalence proof |
| cancellation/early exit | short-circuit demand profiles and future protocol facts | cancellation, stop, break, first-settled, and early-exit behavior must be explicit |
| lifecycle/materialization | `SequenceSurface`, `Domain`, iterator adapter/materializer rows | one-shot views, reusable collections, type-directed materializers, and allocation/lifetime are separate |
| receiver mutation | `Effect(ReceiverMutation)`, place/effect contracts | mutation can close later exact receiver use; it does not create pure value equality |
| ambiguous selector | `Symbol`, `Import`, `CallTarget`, `Domain`, `LibraryApi` occurrence proof | a method/property/function name is only a selector until all required evidence is dependency-closed |

## Existing Mapping

- `DemandEffectProfile` already carries eager, fold, short-circuit, lazy HOF,
  async continuation, generator, scheduled/deferred callback, channel, and
  protocol-boundary timing. This is the contract side of the vocabulary.
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
| Python | builtins `map`/`filter`, `itertools`, `functools`, decorators, materializers, `asyncio` task/timer/aggregate APIs | callback/lifecycle reporting, shared task/aggregate runtime obligations, then narrow producer evidence for already admitted iterator builtins |
| Rust | iterator HOFs, `Option`/`Result`, mutation/effect, iterator views, async task spawn and `join!`/`select!` macros | reuse lazy callback, channel, task, and aggregate vocabulary; keep type-directed `collect`, mutating APIs, and exact async runtime semantics closed |
| Go | `sort`/`slices`/`maps`, mutation callbacks, channel/goroutine/defer/select protocol obligations | keep exact channel/goroutine/defer semantics closed until blocking, close/status, select readiness, callback effects, panic/defer ordering, and scheduling are proven |
| Java | `Arrays`/`Collections`, Optional/Future/Stream-shaped domains, `CompletableFuture` static/continuation reporting, mutation/wrapper APIs | keep Java Future exact recovery closed until executor timing, callback/effect, exception-channel, cancellation, wrapper aliasing, and stream lifecycle obligations are proven |
| Swift | Sequence HOFs, cardinality, mutation, views, reductions, `throws`/`async`, `Task` creation | reuse callback/effect, scheduling, task lifecycle, and cancellation buckets; keep selector-only collection methods and exact task semantics closed |
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
the [interval/scheduler lifecycle boundary audit](../bench/recall_loss/interval-scheduler-lifecycle-boundary-audit-2026-06-30.v1.json),
and the [Go channel protocol pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-go-channel-protocol-2026-06-30.v1.json):
thenable/custom Promise receivers and Promise aggregate distinctions stay pinned
by existing semantic-boundary tests, while executor timing, scheduler ordering,
interval liveness, cancellation-sensitive AbortSignal forms, and Go
channel/goroutine/defer protocol distinctions are reporting-only until
behavior-changing hard negatives exist.

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
The follow-up [cross-language async-function obligation reporting](../bench/recall_loss/cross-language-async-function-obligation-reporting-2026-06-30.v1.json)
does the same for async function and block boundaries. Runtime-body async
functions in JS/TS, Python, Rust, and Swift now preserve and report the shared
`Source::Protocol(AsyncFunction)` obligation as
`scheduling-boundary/async-function-scheduling-contract-missing`; Rust async
blocks preserve `Source::Protocol(AsyncBlock)` and report
`scheduling-boundary/async-block-scheduling-contract-missing`. This is a kernel
capability reuse, not a Promise feature: Promise-specific producer labels such
as `promise-async-function-return-producer-proof` remain tied to JS/TS Promise
receiver recovery.

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
