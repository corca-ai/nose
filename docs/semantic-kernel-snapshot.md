# Semantic kernel snapshot

Snapshot date: 2026-07-01. The current implementation has an internal
semantic-kernel facade, evidence-gated field state, sequence-surface contracts,
proof-backed append fragment evidence, operator-law contracts, typed import
facts, source-fact gates for construct/macro/literal/operator provenance,
receiver-domain evidence resolution, and a shared evidence-record substrate for
source, domain, import, symbol-identity, type-alias, guard,
place/effect, mutation-risk effect, selected library API occurrence,
value-domain/law contracts, and sequence-surface facts.
Cross-file immutable import replacement now covers dependency-backed root
literals, supported imported collection/map values, and Go imported namespace
members without treating raw names or import coordinate literals as proof.
The imported provider-value lane reuses existing `LibraryApi` occurrence
capabilities across the import boundary for JS/TS `new Map(...)`/`new Set(...)`,
Python builtin/imported collection factories, and Java collection/map
factories; provider-local shadows, mutation facts, raw coordinate sequences, and
ambiguous provider factory shapes stay closed.
JS/TS, Python, Rust, and Swift runtime-body async functions are preserved as raw
async protocol boundaries with `Source::Protocol(AsyncFunction)` evidence, even
when the body has no `await`. JS/TS, Python, Rust, and Swift `await`
expressions are preserved as raw async protocol boundaries with
`Source::Protocol(Await)` evidence instead of being erased into their operand.
The near-channel fingerprint build can look through supported async protocol
boundaries while the graded-witness build keeps an explicit protocol wrapper, so
async/sync twins can surface as `async-mirror` transformation leads without
opening exact admission.
JS/TS and Python `yield` expressions are preserved as generator protocol
boundaries with `Source::Protocol(Yield)`. Rust `async {}` and `?` are likewise
preserved as protocol boundaries with `Source::Protocol(AsyncBlock)` and
`Source::Protocol(TryPropagation)`. Go
goroutine spawn, deferred calls, channel send/receive, receive-status
projections, and `select` boundaries are also preserved as raw source-backed
protocol anchors rather than ordinary calls, values, or sequence tags. Their
runtime-boundary reporting now splits channel send synchronization, receive
value channels, comma-ok receive status, select readiness/case/default,
goroutine callback effects, and defer callback effects while keeping exact
admission closed. Python
`asyncio` task/timer/aggregate calls, including import-backed namespace aliases,
Rust `tokio`/`async-std` spawn and `join!`/`select!` macros, including
imported runtime bindings, and Swift `Task` creation now report shared
runtime-boundary obligations without becoming exact recovery evidence. Java
`CompletableFuture` static calls and exact-import-backed CompletionStage-style
receiver continuations likewise report reusable future/channel/callback
obligations when static import identity or receiver-domain evidence is proven.
Python
comprehension lowering now records whether a
HOF came from a list comprehension, set comprehension, dict comprehension, or
generator expression, and exact/value consumers use that surface evidence before
applying materialization or demand-sensitive laws. Admitted builtin and HOF
operations now also have internal `DemandEffectProfile` contracts for the
currently supported eager, short-circuit, append, nullish-default, reduction,
per-element callback, pull-lazy generator, async-continuation, generator
suspension, channel-boundary, and protocol-boundary shapes; these profiles
describe how an already-admitted operation is consumed, not which source API is
admitted. HOF callback timing comes from an explicit source or API demand source,
not from the raw HOF kind alone. The node-level HOF resolver distinguishes
source comprehensions, eager first-party JS-like/Ruby library HOFs, and pull-lazy
Rust iterator/Java Stream HOFs before value-graph and exact consumers open HOF
behavior.
Promise `.then` carries an async-continuation demand/effect profile in its
contract row. Exact value-graph reduction is open only for admitted
Promise-like receivers whose settled value can be recovered from supported
first-party producers: currently JS-like `Promise.resolve(value)` with an
unshadowed `Promise.resolve` proof and a non-thenable-safe value, JS-like
`Promise.reject(reason)` as a rejected channel, plus admitted
`.then(lambda)`/`.catch(lambda)` chains over those boundaries. Safe
`.finally(lambda)` passthrough is open only for admitted Promise-like receivers
and absent or zero-argument handlers returning non-thenable-safe values,
fulfilled Promise boundaries, or rejected Promise boundaries. Same-file direct
async, direct ordinary-function, and proof-backed DirectMethod producers can
also supply the settled value when dependency-backed call-result domain evidence
points to supported return paths. Branch-return producers recover only through
same-channel Promise Phi states. Imported function/member producers can supply
the settled value only through admitted `PromiseSettledValue` evidence composed
with imported call-target identity and `PromiseLike` receiver proof; source-level
imported producers without that contract still stay closed. Handler-returned
`Promise.resolve` is flattened only when the returned value is non-thenable-safe
after local substitution; handler-returned `Promise.reject` preserves the
rejected channel, and a rejecting `.finally` handler overrides the original
settlement with that rejected channel. Selector-only
`.then(...)`/`.catch(...)`/`.finally(...)`, custom thenables, shadowed
`Promise`, unsafe `Promise.resolve(obj)` arguments, mixed fulfilled/rejected
branch channels, unsafe or parameterized `.finally` handlers, unsupported
aggregate inputs, and missing or ambiguous receiver proof stay closed. Literal
Promise aggregates can recover only when the static-global aggregate call is
admitted and every element already has supported Promise settlement evidence or
non-thenable-safe raw-input proof. Raw inputs become fulfilled aggregate
elements. `Promise.all` preserves the all-fulfilled ordered payload channel;
`Promise.allSettled` preserves ordered settled-record channels; `Promise.race`
recovers the first settlement for non-empty fully closed literal arrays; and
`Promise.any` recovers the first fulfilled payload for fully closed literal
arrays with a fulfilled candidate. Dynamic iterables, possible thenables,
all-rejected `Promise.any` AggregateError payloads, executor timing, and sync
aggregate equivalence stay closed.
`new Promise(...)` constructor settlement recovery remains reporting-only. The
current executor audit prices inline executor shape, resolve/reject callback
use, timer/scheduler mentions, multiple settlement, throw-to-rejection,
side-effect calls, and possible thenable payload risk, but does not open exact
constructor admission. A future constructor slice must prove callback identity,
settlement precedence, thrown-error ordering, executor effects, non-thenable
payload safety, and Promise-boundary preservation.
AbortSignal/AbortController cancellation recovery also remains reporting-only.
Runtime-boundary diagnostics name `AbortSignal.abort`, `AbortSignal.any`,
`AbortSignal.timeout`, and `new AbortController()` as cancellation/lifecycle
obligations instead of opaque unsupported runtime rows. Exact cancellation stays
closed until signal identity, abort ordering, abort reason propagation,
listener/timer/fetch rejection behavior, and controller-signal lifecycle are
modeled explicitly.
Timer, interval, scheduler, and microtask recovery remains reporting-only as
well. Runtime-boundary diagnostics name global timer scheduling,
`scheduler.wait` timing plus cancellation/liveness, `scheduler.yield`
microtask ordering, `setInterval` repeated-emission lifecycle, and interval
cancellation plus one-shot timer/frame cancellation as structured obligations.
Exact scheduling stays closed until callback identity, callback demand/effect,
task/microtask/timer ordering, interval cardinality, and cancellation cleanup
are modeled explicitly.
#602 is now closed as a broad boundary milestone, not as an API expansion. The
checked [#602 closeout](../bench/recall_loss/issue-602-closeout-2026-06-30.v1.json)
keeps exact admissions limited to literal Promise aggregate slices and leaves
executor, cancellation, scheduler, timer, interval, and cross-language lifecycle
protocols as named closed obligations for future priced epics.
Node `timers/promises` ESM named imports and conservative `const` CommonJS
destructuring requires are a narrow imported producer slice: admitted
`LibraryApi` occurrence evidence for `node:timers/promises`/`timers/promises`
`setTimeout` and `setImmediate` materializes `Domain(PromiseLike)` at
documented arities. Exactly `setTimeout(delay, value)` and
`setImmediate(value)` also materialize fulfilled `PromiseSettledValue` evidence
for the safe no-options payload. Option-bearing arities remain domain-only
because `options.signal` can reject, and scheduler APIs, interval streams,
mutable CommonJS bindings, and dynamic destructuring patterns stay closed.
The JS/TS corpus audit [`js-ts-stdlib-partial-audit-2026-06-28.v1.json`](../bench/recall_loss/js-ts-stdlib-partial-audit-2026-06-28.v1.json)
confirms this is the largest JS/TS builtin-shaped surface in the pinned corpus:
`29,094` Promise/async occurrences are tracked as a processed closed boundary
with zero semantic-admission delta. The follow-up [promise protocol diagnostics](../bench/recall_loss/promise-protocol-diagnostics-2026-06-28.v1.json)
split that closed boundary into await scheduling, async function scheduling,
executor callback, factory, aggregate result, rejection channel, and
non-construct Promise-call labels for recall-loss reporting.
The [non-JS async runtime API reporting artifact](../bench/recall_loss/non-js-async-runtime-api-obligation-reporting-2026-06-30.v1.json)
extends that attribution capability to Python/Rust/Swift runtime APIs using
shared `task-*` and `async-aggregate-*` obligations while keeping exact
admission closed. The follow-up [non-JS async runtime import-proof artifact](../bench/recall_loss/non-js-async-runtime-import-proof-2026-06-30.v1.json)
widens that reporting to Python `asyncio` namespace aliases and Rust imported
runtime bindings by composing existing `ImportedNamespace`/`ImportedMember`
symbol proof with the same obligations instead of adding a selector-specific
kernel feature.
The next [imported-binding proof artifact](../bench/recall_loss/non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json)
uses the same capability boundary for Python `from asyncio import ...`
bindings and Rust brace-use `ImportedBinding` evidence, adding `2` newly priced
Python imported `asyncio.sleep` occurrences while keeping exact admission
closed and preserving the release `verify crates` gate at `0` false merges.
The follow-up [Swift structured-concurrency artifact](../bench/recall_loss/swift-structured-concurrency-obligation-reporting-2026-06-30.v1.json)
keeps exact admission closed while mapping `Task.sleep`, `Task.yield`, and
task-group calls onto the existing timer, task-yield, aggregate,
cancellation/liveness, result-channel, and exception-channel obligations. Its
matching [120-repo pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-swift-structured-concurrency-2026-06-30.v1.json)
prices `Task.sleep` at `161` occurrences / `10` repos, task groups at `153` /
`9`, `Task.yield` at `12` / `3`, and corrects `5` already-supported
`Task.detached(...)` spawn occurrences in the audit.
The follow-up [Rust block_on future-drive artifact](../bench/recall_loss/rust-block-on-future-drive-obligation-reporting-2026-07-01.v1.json)
keeps exact admission closed while mapping qualified/import-backed
`tokio_test::block_on` plus proof-backed `Handle::current().block_on` and
inline `Runtime`/`Builder` block-on receiver chains onto
`future-drive-scheduling-contract` plus the existing
`future-settled-value-channel-contract`. Tokio spot checks show `4` future-drive
evidence units in `tokio/tests/rt_handle_block_on.rs`, `2` of them as the
primary scheduling subreason; selector-only and unproven variable/field/parameter
`.block_on` receivers remained closed in that slice. The follow-up [Rust local runtime provenance
artifact](../bench/recall_loss/rust-block-on-local-runtime-provenance-2026-07-01.v1.json)
keeps exact admission closed while following local variables whose last visible
assignment is direct proof-backed `Handle::current()`,
`Runtime::new().unwrap()/expect/?`, or
`Builder::new_*().build().unwrap()/expect/?`. Meilisearch and Tokio spot checks
show `1` and `2` additional future-drive evidence units respectively. The
follow-up [Rust parameter runtime provenance artifact](../bench/recall_loss/rust-block-on-parameter-runtime-provenance-2026-07-01.v1.json)
keeps exact admission closed while following nominal
`tokio::runtime::Runtime`/`Handle` parameter receivers backed by fully
qualified type text or scope-visible exact imported-binding evidence. Tokio
spot checks show `2` future-drive evidence units in
`tokio/tests/fs_uring_read.rs` and `1` in
`tokio/tests/rt_unstable_eager_driver_handoff.rs`. The follow-up [Rust nested
brace runtime provenance artifact](../bench/recall_loss/rust-nested-brace-runtime-provenance-2026-07-01.v1.json)
adds per-item evidence for nested static brace imports such as
`use tokio::{runtime::{Runtime}}`; Tokio `fs_uring.rs` then shows `2`
future-drive evidence units for nested-brace `Runtime` parameter receivers.
The follow-up [Rust self-field runtime provenance artifact](../bench/recall_loss/rust-self-field-runtime-provenance-2026-07-01.v1.json)
adds exact `self.<field>.block_on(...)` receivers whose same-scope struct field
declaration proves `tokio::runtime::Runtime` or `Handle`; Tokio
`sync_bridge.rs` moves from `0` to `13` future-drive oracle exclusions with
`0` false merges. Non-self fields, local struct fields, wildcard/relative
imports, child-module parameters with only parent-module imports, project-local
`tokio` roots or aliases including raw-identifier spellings, type aliases,
wrappers, and constructor-assigned fields remain closed. The follow-up [Rust
map_err runtime provenance artifact](../bench/recall_loss/rust-block-on-map-err-runtime-provenance-2026-07-01.v1.json)
extends the same future-drive reporting to `Result::map_err` adapters that wrap
an already proven `Runtime::new()` or `Builder::build()` success channel before
`?`, `unwrap`, or `expect` exposes the runtime receiver. Nushell spot checks
move two direct local `Runtime::new().map_err(...)?` block_on paths from `0` to
`1` future-drive evidence unit each; wrapper-returned Results, non-Result
`map_err` calls, and constructor-assigned fields remain closed.
The follow-up [Go channel protocol pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-go-channel-protocol-2026-06-30.v1.json)
keeps exact admission closed while pricing `4,294` channel receives, `1,525`
sends, `155` comma-ok receives, `1,920` select parents, `3,590` select cases,
`546` select defaults, `1,949` goroutines, and `17,521` defers in the pinned
120-repo corpus. Select parents and arms are counted separately because Go
lowering preserves them as distinct source-backed protocol boundaries.
The follow-up [Go select receive-status artifact](../bench/recall_loss/go-select-receive-status-protocol-2026-07-01.v1.json)
keeps exact admission closed while preserving comma-ok receive status inside
`select` communication cases. `case _, ok := <-ch` now contributes the existing
`channel-receive-status-contract` in addition to select readiness/case
obligations; the pinned corpus has `107` lexical hits across `57` files and `7`
repos.
The follow-up [Ruby Thread/Fiber runtime artifact](../bench/recall_loss/ruby-thread-fiber-runtime-reporting-2026-07-01.v1.json)
keeps exact admission closed while mapping Ruby `Thread.new`, `Thread.start`,
`Thread.fork`, `Fiber.new`, and `Fiber.schedule` onto shared task-spawn,
task-handle, cancellation/liveness, and concurrency scheduling obligations.
Same-file `Thread`/`Fiber` definitions keep attribution closed. The matching [pricing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-ruby-thread-fiber-runtime-2026-07-01.v1.json)
marks `74` occurrences across `11` repos as reporting-supported closed
boundaries.
The follow-up [non-JS async runtime scope-shadowing artifact](../bench/recall_loss/scheduling-lifecycle-boundary-audit-non-js-async-runtime-scope-shadowing-2026-06-30.v1.json)
keeps the same capability boundary but makes Python/Rust async runtime
attribution scope-aware. Unrelated local shadows in other functions no longer
close import-backed `asyncio` or Rust runtime reporting, while same-scope,
enclosing-scope, and module-level shadows still keep exact admission closed.
The 120-repo pricing total remains `146,880`, with the release `verify crates`
gate at `0` false merges. Python/Rust async-runtime diagnostics prefer
source-preserving unit roots before normalized fallback so alpha-renamed oracle
units do not misreport Python `asyncio` alias shadows.
The follow-up [non-JS async runtime breadth artifact](../bench/recall_loss/non-js-async-runtime-breadth-2026-07-01.v1.json)
keeps exact admission closed while broadening Python and Swift use of the same
kernel obligations: Python `asyncio.run`, `wait_for`, `shield`,
`run_coroutine_threadsafe`, and `to_thread` now map to future-drive, timer,
task, cancellation/liveness, future-settled, callback, and exception labels
under the existing asyncio import/shadow guards; Swift checked/unsafe
continuation bridges now map to future-settled, settlement-continuation,
callback-demand/effect, and throwing exception labels under the existing
unshadowed free-runtime-function guard. The matching 120-repo pricing adds
`107` source-prevalence occurrences over the scope-shadowing audit and keeps the
local `crates` gate at `0` false merges.
Library/API identity is consolidated through internal `LibraryApiContract` rows
for factory, constructor, selected property/non-factory method/view surfaces,
and selected non-call sentinels, with occurrence evidence covering selected
JS-like static/global APIs and static index-membership calls, JS/TS/Java
`length` property reads, Python builtin/import-backed APIs, Rust free-name/path
APIs including `Option::Some`/`Option::None` and `Result::Ok`/`Result::Err`,
Ruby require-backed APIs, Java
`java.util` APIs including selected empty constructors, JS regex API calls, and
selected language-scoped receiver-method APIs such as collection membership,
map lookup/defaulting, map-key views, iterator identity adapters, Rust scalar
integer methods, Rust `Option::and_then`, Rust Result channel predicates,
Rust `zip`, and HOF/reduction methods.
Selected producer-covered factory/API calls now also emit dependent receiver-expression
`Domain` evidence
for their result container domain, and normalize emits binding-anchored `Domain`
evidence for immutable local/module bindings whose initializer domain and
non-mutation conditions are proven by first-party evidence/analysis.

## What exists today

nose now has a first internal semantic-kernel facade, but most of the engine is
still being migrated toward it.

- `nose-il` defines a compact shared IL, `Lang`, `Builtin`, `HoFKind`, operators,
  literals, source spans, units, and pack-facing internal `EvidenceRecord` facts.
- `nose-semantics` defines the builtin semantic profile facade: language,
  source-fact, sequence-surface, guard/import/symbol, operator, demand/effect,
  fragment, module, stdlib, builtin, method-call, property, async,
  iterator-adapter, builder-append, and factory contracts. The public crate
  surface remains a flat facade, while those proof helpers and contract rows are
  split into focused modules under `src/` and `src/library_api/`. Compiled
  builtin packs now have a small `BuiltinPackDescriptor` registry that feeds
  the existing `SemanticPackSummary` compatibility output without changing
  analysis behavior. Builtin language descriptors now report official
  parser/lowering ownership metadata for Python, JavaScript/TypeScript, Go,
  Rust, Java, C, Ruby, Swift, CSS, and HTML/Vue/Svelte embedded-region support.
  Generic language-core evidence and source facts emitted by frontend lowering
  now carry those `nose.lang.*` pack producers. Cross-file module-import
  immutable literal export/snapshot proof uses the same builtin language-core
  producer. `nose.lang.c` also owns the C file-extension identity,
  `tree-sitter-c` parser binding,
  `nose_frontend::c::lower` lowering entrypoint metadata, and the
  `c.source.cast.unsigned32` source-fact producer id. The
  `nose.python.builtins.collection_factories` descriptor owns the Python builtin
  `list`, `set`, `frozenset`, and `tuple` collection-factory contract id and
  `LibraryApi` occurrence producer id, while shadowed names and wildcard imports
  remain hard negatives. The
  `nose.python.stdlib.collection_factories` descriptor owns Python
  `collections.deque` collection-factory contract and occurrence producer ids,
  while missing imports and wrong modules remain hard negatives. The
  `nose.python.stdlib.math` descriptor owns Python `math.prod` product-reduction
  contract and occurrence producer ids, while missing imports, wrong modules,
  and shadowed `math` bindings remain hard negatives. The
  `nose.ruby.stdlib.set` descriptor owns Ruby `Set.new` collection-factory
  contract and occurrence producer ids, while missing `require "set"`, shadowed
  `Set`, and mutated local sets remain hard negatives. The
  `nose.rust.stdlib.vec` descriptor owns Rust `Vec::new` and `vec!`
  collection-factory contract and occurrence producer ids, while shadowed `Vec`
  roots and shadowed `vec` macros remain hard negatives. The
  `nose.rust.stdlib.option` descriptor owns Rust `Some`, `None`, and
  `and_then` Option API contract and occurrence producer ids, while shadowed
  Option selectors and non-Option receivers remain hard negatives. Java
  `java.util.Optional<T>` receiver proof currently admits `isPresent()` and
  `orElse(default)` through the generic builtin method-call protocol; bare
  imported `Optional<T>`, `isEmpty()`, static constructors, and callback-like
  Optional APIs remain closed until import-backed type-domain and callback
  obligations are represented. The
  `nose.rust.stdlib.result` descriptor owns Rust `Ok`/`Err` Result constructor
  provenance and exact-Result `is_ok`/`is_err` predicate occurrence producer
  ids, while shadowed selectors, local `Result` type shadows, non-Result
  receivers, callback/default helper APIs, and panic-like unwrap surfaces remain
  hard negatives. The
  `nose.rust.stdlib.integer_methods` descriptor owns Rust primitive integer
  `abs`/`min`/`max`/`clamp` method API contract and occurrence producer ids,
  while non-integer receivers and unsupported arities remain hard negatives. The
  `nose.java.stdlib.math` descriptor owns Java `Math.abs`, `Math.min`, and
  `Math.max` scalar integer API contract and occurrence producer ids, while
  missing unshadowed `Math` proof, non-integer value arguments, and unsupported
  arities remain hard negatives. The
  `nose.javascript.builtins.promise` descriptor owns JS/TS `Promise.resolve`,
  `Promise.reject`, `.then`, `.catch`, `.finally`, `Promise.all`, and
  `Promise.allSettled`, `Promise.race`, and `Promise.any` Promise API contract
  and occurrence producer ids, while
  shadowed `Promise`, missing Promise-like receiver proof, unsafe thenable
  assimilation, unsafe `.finally` handlers, all-rejected `Promise.any`, and
  unsupported aggregate inputs remain hard negatives. The
  `nose.javascript.builtins.array` descriptor owns JS/TS `Array.from`,
  `Array.isArray`, exact-Array receiver `map`/`filter`/`flatMap`, and
  `some`/`every` API contract and occurrence producer ids, while shadowed
  `Array` roots, unsupported `Array.from` arities, callback `thisArg` arities,
  sparse array literals, borrowed prototype calls, effectful callbacks, generic
  collection receivers, and deferred absence/default methods remain hard
  negatives. Pre-call monkey-patching and receiver mutation require future
  JS-specific place/effect proof. The
  `nose.javascript.builtins.boolean` descriptor owns JS/TS `Boolean(...)` API
  contract and occurrence producer ids, while shadowed `Boolean` roots and
  unsupported arities remain hard negatives. The
  `nose.javascript.builtins.regex` descriptor owns JS/TS regex literal
  `.test(...)` API contract and occurrence producer ids, while non-regex
  receivers and unsupported arities remain hard negatives. The
  `nose.javascript.builtins.static_index_membership` descriptor owns JS/TS
  static `indexOf`/`findIndex` membership API contract and occurrence producer
  ids, while non-literal receivers and float-literal receivers remain hard
  negatives. The
  `nose.javascript.builtins.collection_constructors` descriptor owns JS/TS
  `new Set(...)` and `new Map(...)` API contract and occurrence producer ids,
  while missing construct-source proof and shadowed constructor roots remain
  hard negatives. The
  `nose.rust.stdlib.collection_factories` descriptor owns selected Rust
  `std::collections::{HashSet,BTreeSet,VecDeque}::from` collection-factory
  contract and occurrence producer ids, while shadowed `std` roots remain hard
  negatives. The `nose.rust.stdlib.map_factories` descriptor owns selected Rust
  `std::collections::{HashMap,BTreeMap}::from` map-factory contract and
  occurrence producer ids, while shadowed `std` roots remain hard negatives.
  The `nose.swift.stdlib.collection_factories` descriptor owns Swift
  `Array(sequence)`, `Set(sequence)`, and
  `Dictionary(uniqueKeysWithValues:)` collection/map-factory contract and
  occurrence producer ids, while shadowed type names, same-corpus typealias
  shadows, wrong labels, implicit tuple-entry shape, and static duplicate-key
  inputs remain hard negatives. The
  `nose.java.stdlib.map_factories` descriptor owns Java `java.util.Map.of`
  and `Map.ofEntries` map-factory contract and occurrence producer ids, while
  missing `java.util.Map` imports and cross-surface `Map.entry` boundary cases
  remain hard negatives. The `nose.java.stdlib.map_entries` descriptor owns Java
  `java.util.Map.entry` map-entry contract and occurrence producer ids, while
  missing `java.util.Map` imports and shadowed `Map` roots remain hard
  negatives. The `nose.java.stdlib.collection_factories` descriptor owns Java
  `java.util.List.of`, `Set.of`, and `Arrays.asList` collection-factory
  contract and occurrence producer ids, while missing imports and
  cross-surface constructor boundary cases remain hard negatives. Imported
  provider snapshots reuse these occurrence proofs only for exact-safe provider
  arguments; ambiguous single-argument `Arrays.asList(...)` providers remain
  closed at the export boundary. The corpus audit [`java-arrays-collections-audit-2026-06-28.v1.json`](../bench/recall_loss/java-arrays-collections-audit-2026-06-28.v1.json)
  tracks the remaining Java `Arrays`/`Collections` method-level boundary mix so
  future work can choose capability slices rather than one-off API rows. The
  `nose.java.ecosystem.guava.immutable_collection_factories` descriptor owns
  Guava `ImmutableList.of`, `ImmutableSet.of`, and `ImmutableMap.of` factory
  contract and occurrence producer ids, while `copyOf`, missing imports,
  wrong-package surfaces, and local type shadows remain descriptor hard
  negatives. Static null elements/key-values, duplicate static `ImmutableMap`
  keys, and unsupported `ImmutableMap.of` arities stay closed in the semantic
  consumers. The
  `nose.java.stdlib.collection_constructors` descriptor owns Java empty
  `new ArrayList<>()` and `new LinkedList<>()` collection-constructor contract
  and occurrence producer ids, while missing imports, local type shadows, and
  conflicting explicit imports remain hard negatives. The
  `nose.java.stdlib.static_collection_adapters` descriptor owns Java
  `java.util.Arrays.stream` static collection adapter contract and occurrence
  producer ids, while missing imports and shadowed `Arrays` roots remain hard
  negatives. The
  `nose.protocols.map_get` descriptor owns Java/Rust/JS-family `map.get(key)`
  contract and occurrence producer ids, while non-map receivers and unsupported
  arities remain hard negatives. The
  `nose.protocols.map_get_default` descriptor owns Python `dict.get(key,
  default)`, Ruby `Hash#fetch(key, default)` or zero-arg block fallback, and
  Java `Map.getOrDefault(key, default)` contract and occurrence producer ids,
  while non-map receivers and unsupported arities remain hard negatives. The
  `nose.protocols.free_function_builtins` descriptor owns unshadowed
  Python/Go/Swift free-name builtin API occurrence contracts such as Python
  `len`/`range`/reductions, Go `len`/`append`, and Swift
  `abs`/`min`/`max`, while missing symbol proof, compatibility-pack evidence,
  wrong producers, and unsupported arities remain hard negatives. The
  `nose.protocols.receiver_membership` descriptor owns receiver-method
  membership contracts for Java/Rust/Ruby map-key membership, Python
  `__contains__`, JS-like `has`/`includes`, Java/Swift `contains`, and Ruby
  `member?`, while missing receiver proof, unsupported arities, and Go
  `slices.Contains` remain hard negatives. The
  `nose.protocols.map_key_views` descriptor owns Python/Ruby `keys`, Java
  `keySet`, JS-family `Map.keys()`, and JS/TS `Object.keys` static-object
  key-view contract and occurrence producer ids, while non-map receivers,
  unsupported arities, shadowed `Object`, and mutated object arguments remain
  hard negatives. The
  `nose.protocols.property_builtins` descriptor owns JS/TS/HTML-family and Java
  `.length`, plus Swift `count` and `isEmpty`, property-builtin contract and
  occurrence producer ids, while missing receiver proof, wrong-pack evidence,
  and unsupported properties remain hard negatives. The
  `nose.protocols.builtin_method_calls` descriptor owns generic method-call
  and namespace-call builtin semantics that have not moved to a narrower
  protocol or stdlib pack, while receiver/symbol/import proof and unsupported
  arities remain hard negatives. The
  `nose.protocols.string_affix_predicates` descriptor owns case-sensitive
  prefix/suffix predicate contracts under exact string receiver proof for
  receiver methods and imported `strings` namespace proof for Go
  `strings.HasPrefix`/`HasSuffix`; Ruby literal receivers provide exact string
  proof while untyped/custom Ruby receivers, multi-affix calls, wrong receivers,
  direction mismatches, and same-file `String` monkey patches remain closed.
  Same-role parameter affixes and immutable literal binding affixes remain
  admitted coordinate sources; wrong parameter coordinates, dynamic affix
  expressions, mutated bindings, Python tuple affixes, and JS/Java offset forms
  remain outside whole-string prefix/suffix proof.
  Missing or non-string receiver proof, missing or wrong Go namespace proof,
  wrong-pack or wrong-producer evidence, unsupported arities, offset forms,
  JS/TS untyped receivers, `String` object wrappers, nullable receivers,
  borrowed/custom same-name calls, and direct `String.prototype` patching remain
  hard negatives.
  The
  `nose.protocols.sequence_hof_adapters` descriptor owns Rust iterator
  `map`/`filter`/`filter_map`/`flat_map` HOF adapter occurrence provenance and
  `any`/`all`/`count` terminal proof on explicit protocol receivers. It also
  owns Swift `map`/`filter`/`flatMap` HOF occurrence provenance on proven
  Array/Collection receivers with inline effect-closed callbacks, while Swift
  `Set`, `Dictionary`, `Sequence`/`AnySequence`, `.lazy`, throwing or mutating
  callbacks, and `compactMap` remain hard negatives or unsupported boundaries.
  It also owns Ruby Enumerable `map`/`collect`/`select`/`filter`/`reject` HOF
  occurrence provenance on proven Array/Collection receivers with inline
  effect-closed blocks. Ruby calls without blocks, `Enumerator::Lazy`,
  framework relation receivers, custom same-name methods, Hash key/value
  iteration, Set ordering, mutating or raising blocks, and `flat_map` remain
  hard negatives or unsupported boundaries; `reject` carries a negated predicate
  instead of reusing the positive filter predicate.
  Rust custom methods, missing receiver proof, eager callback assumptions,
  missing terminal proof, one-shot iterator reuse, `collect_vec`, and `find`
  remain hard negatives or unsupported boundaries. The
  `nose.go.stdlib.namespace_calls` descriptor owns Go `fmt.Print*`,
  `strings.Contains`, `strings.Join`, and `slices.Contains` namespace-call API
  occurrence provenance under imported namespace proof. `strings.Contains`
  lowers to the separate `StringContains` semantic so substring membership does
  not reuse collection membership. `strings.Join` reuses the ordered `Join`
  builtin with separator and collection arguments normalized into the same shape
  as other join producers. The corpus audit [`go-stdlib-collections-audit-2026-06-28.v1.json`](../bench/recall_loss/go-stdlib-collections-audit-2026-06-28.v1.json)
  keeps the rest of Go `sort`/`slices`/`maps` separated into mutation, ordering,
  callback, iterator, copy, and equality capability buckets. The
  `nose.protocols.iterator_identity_adapters` descriptor owns Rust
  `iter`/`into_iter`/`iter_mut`/`collect`/`to_vec`/`copied`/`cloned` and Java
  `.stream()` iterator identity adapter contract and occurrence producer ids,
  while non-protocol receivers and unsupported arities remain hard negatives. The
  `nose.python.stdlib.type_domain` descriptor directly exposes its alias
  contract rows so producer id, contract id, conformance refs, and declaration
  counts come from one pack-owned table.
- The external pack API is documented as a v0 manifest/schema with examples.
  `nose-semantics` can validate local manifest files/directories as metadata,
  `nose semantic-pack check` validates local manifests plus declared fixture
  assets, and `nose query --format json` reports active builtin/local packs in
  top-level `semantic_packs`. External packs are still `metadata-only`; builtin
  producers remain compiled Rust and are expected to map onto the same
  vocabulary. The first compiled pilots are the `nose.lang.*` builtin language
  descriptor/source-fact producer set, with `nose.lang.c` also carrying C
  unsigned-cast source provenance,
  `nose.python.builtins.collection_factories`, a default builtin stdlib pack for
  Python builtin collection-factory API provenance, and
  `nose.python.stdlib.collection_factories`, a default builtin stdlib pack for
  Python `collections.deque` collection-factory API provenance,
  `nose.python.stdlib.math`, a default builtin stdlib pack for Python
  `math.prod` product-reduction API provenance,
  `nose.ruby.stdlib.set`, a default builtin stdlib pack for Ruby `Set.new`
  collection-factory API provenance,
  `nose.rust.stdlib.vec`, a default builtin stdlib pack for Rust `Vec::new` and
  `vec!` collection-factory API provenance,
  `nose.rust.stdlib.option`, a default builtin stdlib pack for Rust `Some`,
  `None`, and `and_then` Option API provenance,
  `nose.rust.stdlib.result`, a default builtin stdlib pack for Rust `Ok`/`Err`
  Result constructor provenance and exact-Result `is_ok`/`is_err` predicate
  provenance,
  `nose.rust.stdlib.integer_methods`, a default builtin stdlib pack for Rust
  primitive integer `abs`/`min`/`max`/`clamp` method API provenance,
  `nose.java.stdlib.math`, a default builtin stdlib pack for Java `Math.abs`,
  `Math.min`, and `Math.max` scalar integer API provenance,
  `nose.javascript.builtins.promise`, a default builtin stdlib pack for JS/TS
  `Promise.resolve` and `.then` Promise API provenance,
  `nose.javascript.builtins.array`, a default builtin stdlib pack for JS/TS
  `Array.from`, `Array.isArray`, and exact-Array receiver
  `map`/`filter`/`flatMap` plus `some`/`every` API provenance,
  `nose.javascript.builtins.boolean`, a default builtin stdlib pack for JS/TS
  `Boolean(...)` API provenance,
  `nose.javascript.builtins.regex`, a default builtin stdlib pack for JS/TS
  regex literal `.test(...)` API provenance,
  `nose.javascript.builtins.static_index_membership`, a default builtin stdlib
  pack for JS/TS static `indexOf`/`findIndex` membership API provenance,
  `nose.javascript.builtins.collection_constructors`, a default builtin stdlib
  pack for JS/TS `new Set(...)` and `new Map(...)` API provenance,
  `nose.rust.stdlib.collection_factories`, a default builtin stdlib pack for
  selected Rust `std::collections` collection-factory API provenance,
  `nose.rust.stdlib.map_factories`, a default builtin stdlib pack for selected
  Rust `std::collections` map-factory API provenance, and
  `nose.swift.stdlib.collection_factories`, a default builtin stdlib pack for
  Swift `Array(sequence)`, `Set(sequence)`, and
  `Dictionary(uniqueKeysWithValues:)` API provenance, and
  `nose.java.stdlib.map_factories`, a default builtin stdlib pack for Java
  `java.util.Map.of` and `Map.ofEntries` map-factory API provenance, and
  `nose.java.stdlib.map_entries`, a default builtin stdlib pack for Java
  `java.util.Map.entry` map-entry API provenance, and
  `nose.java.stdlib.collection_factories`, a default builtin stdlib pack for
  Java `java.util.List.of`, `Set.of`, and `Arrays.asList` collection-factory
  API provenance, and
  `nose.java.ecosystem.guava.immutable_collection_factories`, a default builtin
  library pack for Guava `ImmutableList.of`, `ImmutableSet.of`, and
  `ImmutableMap.of` factory API provenance, and
  `nose.java.stdlib.collection_constructors`, a default builtin stdlib pack for
  Java empty `new ArrayList<>()` and `new LinkedList<>()` collection-constructor
  API provenance, and
  `nose.java.stdlib.static_collection_adapters`, a default builtin stdlib pack
  for Java `java.util.Arrays.stream` static collection adapter API provenance,
  and
  `nose.protocols.map_get`, a default builtin protocol pack for Java/Rust/
  JS-family `map.get(key)` API provenance, and
  `nose.protocols.map_get_default`, a default builtin protocol pack for Python
  `dict.get(key, default)`, Ruby `Hash#fetch(key, default)` or zero-arg block
  fallback, and Java `Map.getOrDefault(key, default)` API provenance, and
  `nose.protocols.receiver_membership`, a default builtin protocol pack for
  receiver-method membership API provenance across map, collection, and
  set-or-map receiver contracts, and
  `nose.protocols.map_key_views`, a default builtin protocol pack for
  Python/Ruby `keys`, Java `keySet`, and JS-family `Map.keys()` API provenance,
  and
  `nose.protocols.property_builtins`, a default builtin protocol pack for
  JS/TS/HTML-family and Java `.length`, plus Swift `count` and `isEmpty`,
  property-builtin API provenance, and
  `nose.protocols.builtin_method_calls`, a default builtin protocol pack for
  generic method-call and namespace-call builtin semantics that have not moved
  to a narrower protocol or stdlib pack, and
  `nose.go.stdlib.namespace_calls`, a default builtin stdlib pack for Go
  `fmt.Print*`, `strings.Contains`, and `slices.Contains` namespace-call API
  provenance, and
  `nose.protocols.iterator_identity_adapters`, a default builtin protocol pack
  for Rust iterator identity adapters and Java `.stream()` API provenance, and
  `nose.python.stdlib.type_domain`, a default builtin stdlib pack-shaped surface
  for Python `typing`, `collections.abc`, and `asyncio` type-domain alias
  evidence.
- `nose-frontend` owns tree-sitter parsing, per-language lowering (including the
  declarative CSS/HTML frontends), `<script>`/`<style>`/markup region extraction for
  Vue/Svelte/HTML, source/domain/import/symbol/type/guard/place/effect/API/
  sequence evidence emission, and Raw-node coverage.
- `nose-normalize` owns desugaring, alpha-renaming, recursion normalization,
  immutable binding-domain evidence inference, dataflow, CFG/algebra
  normalization, type-gated value-graph rules, and the interpreter oracle. The
  value graph keeps its public facade in `value_graph.rs`, with focused internal
  modules for active builders, control/loop processing, collection/HOF/library
  value recognition, output extraction, stdlib recognizers, pure inlining,
  low-level ops, and proof-sensitive rules.
- proof-sensitive value-graph laws continue to live in named rule modules under
  `crates/nose-normalize/src/value_graph/rules/`; `clamp` and `promise_then`
  are the current examples.
- `nose-detect` owns unit extraction, strict exact-safety proof gates, exact
  fragment contracts, effect fragments, value/shape features, candidate
  generation, clustering, and ranking. The strict exact gate lives in its own
  module so evidence-backed proof policy is not mixed with unit extraction
  orchestration, and selected strict exact API paths, including first-party
  factory/constructor paths, now consume the shared `nose-semantics` admitted
  occurrence resolvers instead of locally recombining selector parsing with
  `LibraryApi` evidence checks.
- `formal/obligations` records proof obligations for proof-sensitive rules.

The current model already enforces the main product principle: exact semantic
matches must be fail-closed and false merges are bugs.

An experimental `abstraction` scan mode now exists as a weak sibling surface over
`near`, not as an exact semantic relaxation. It keeps only same-language candidates
whose family-wide normalized IL differs by exactly one shared supported literal leaf
position and emits an `abstraction_witness` with a typed hole, a reason code, checked
member count, observed literal classes, and caveats such as `numeric-domain-sensitive`.

## Implemented facade contracts

The current facade is compiled Rust, not an external manifest schema. It is
intended to make the future pack extension boundary explicit while behavior is
migrated.

- The builtin profile exposes pack id and trust policy separately from
  channel eligibility. `ChannelEligibility` describes where a fact may be used;
  builtin/default status is pack provenance, not an analysis channel.
- `Il::evidence` is now the shared internal substrate for source, domain, import,
  symbol-identity, type-alias, guard, place/effect, selected library API
  occurrence, and sequence-surface proof facts. Records carry ids, stable source anchors, kind,
  provenance, dependencies, and asserted/ambiguous status. Lookups in
  `nose-semantics` fail closed on ambiguous, conflicting, or dependency-broken
  evidence. Source-origin and parameter-domain proof is now evidence-only;
  explicitly legacy helper fallbacks remain only for proof families whose
  evidence migration is not complete.
- `OperatorSemantics` now owns the first shared operator contracts:
  comparison-direction transforms, comparison negation, equality operand
  commutativity, comparison-lattice laws, abs/min/max/selection guard laws,
  static cardinality thresholds, JS-like static `indexOf`/`findIndex`
  thresholds, and source membership operators. Algebra normalization, CFG
  branch orientation, value-graph comparison/count rewrites, and strict exact
  static-index gates consume these contracts instead of local operator tables.
  The old `primitive_order_comparisons()` helper remains as a compatibility
  wrapper around the stricter lattice law contract.
- `ValueDomain` and `ValueLaw` now own the first shared domain preconditions for
  value-graph and recursion laws. The old normalize-local `Ty` lattice and
  `types.rs` inference module are gone. `nose-semantics` infers only the coarse
  domains required by current first-party laws: numeric, boolean, string,
  sequence, or unknown. The inference consumes parameter `Domain` evidence
  first, then a conservative fixpoint over strict operator uses, literal and
  builtin result domains, and subexpression result domains. Value graph add
  commutativity/associativity, numeric negation/idempotence, boolean AC
  simplifications, factor distribution, large formula compaction, and structural
  recursion folds now consume `ValueLaw` contracts rather than a normalize-local
  type helper. Unknown remains optimistic only for the historical non-concat
  `+` policy; explicit string/sequence domain evidence keeps concatenation
  ordered, and numeric/boolean laws require positive domain proof. The current
  `ValueLawContract` remains the internal domain gate, and the first compiled
  first-party `LawPack` pilot now reports per-family provenance for selected
  proof-backed value-graph laws through `nose.value_graph.laws`. The pilot covers
  numeric common-factor distribution and integer ordered min/max clamp, including
  stable law ids, exact-proven channel, proven status, and formal obligation ids.
  Broader internal laws and external LawPack execution remain closed.
- Source facts are now first-class internal evidence for source distinctions that
  the shared IL erases. JS/TS frontends emit construct syntax, async function and
  async `await` boundaries, generator `yield` boundaries, regex literal, strict/loose equality,
  strict/loose inequality, and `instanceof` facts. Python emits async `await`,
  async function and generator `yield` boundaries, list/set/dict/generator
  comprehension surfaces, value equality/inequality, and identity
  equality/inequality facts. Swift emits async function, async `await`, and
  `try` boundaries. Go emits
  protocol facts for `go`, `defer`, channel send/receive, receive-status
  projection, `select`, and select cases/defaults. C emits source-cast facts
  for explicit unsigned 32-bit byte-lane casts, with alias-based casts depending
  on C type-alias evidence. Rust emits macro invocation syntax for selected
  macro-backed APIs, half-open/inclusive range expression facts, tuple-struct
  single-wildcard pattern facts, plus async/error protocol facts for async
  functions, `.await`, `async {}`, and `?`. These are stored directly as
  `EvidenceRecord::Source`; there is no source-fact side-table fallback.
  Normalize and detect consume source facts only where a semantic contract
  requires that exact source surface. Current JS/TS/Python/Rust/Swift async
  function and `await` nodes, JS/TS/Python `yield` nodes, Rust `async`/`?` nodes,
  Swift `try` nodes, and Go concurrency/channel
  nodes remain raw exact-closed protocol anchors until such a contract exists.
  Python returned generator/set comprehensions and unsupported cardinality
  surfaces stay exact-closed; supported list/generator terminal reductions can
  still reopen only through consumer-specific demand checks.
- Free-function builtin contracts are language- and arity-constrained.
  Supported Python/Go/Swift free builtins such as Python `len`, `sum`, `min`,
  `max`, `any`, `all`, Go `append`, and Swift `abs` require admitted
  `nose.protocols.free_function_builtins` `LibraryApi(FreeFunctionBuiltin)`
  occurrence evidence whose dependencies prove the unshadowed builtin/global
  callee before exact lowering.
- Canonical `Payload::Builtin` calls now have an explicit admission gate. A
  builtin payload is only a normalized operation shape; it is not itself proof
  that a language/library API has that meaning. Value-graph builtin folding,
  builtin fallback tags, range/len/zip/enumerate loop patterns, strict-exact
  builtin calls, function-binding safety, mutation-risk blocking, value-domain
  builtin result inference, and interpreter-oracle builtin execution now consume
  builtin semantics through `admitted_builtin_semantics_at_call`. That helper
  admits same-span `LibraryApi` occurrence evidence after desugaring, plus the
  narrow syntax-owned lowerings for Go map lookup-ok `Contains`, Go
  `Enumerate`, Python dict-comprehension `DictEntry`, JS-like `Keys`, C
  `UnsignedCast32` with source-cast evidence, and append calls with language-core
  `Effect(BuilderAppendCall)`. Receiver-dependent specializations also stay
  proof-chain-gated: Rust `unwrap_or` canonicalizes to map `GetOrDefault` only
  when its admitted method occurrence depends on an admitted pack-proven Rust map
  `get` occurrence. Raw builtin payloads remain opaque or exact-closed.
- Method contracts carry receiver obligations such as exact collection, exact
  protocol, exact option, exact string, exact primitive integer, exact map literal,
  imported namespace, or unshadowed global.
- First-party parameter type-domain producers live in `nose-semantics` as
  language-scoped contracts and are emitted by frontends as
  `EvidenceRecord::Domain` on `Param` anchors. The old common substring fallback
  over whole parameter text is gone; hard negatives such as TypeScript
  `Bitmap<K,V>` and `Blacklist<T>` do not prove map/collection domains, Java
  annotation text is ignored before array/varargs recognition, Rust fully
  qualified `std::collections` paths are covered, and C pointer parameters do
  not inherit scalar integer domains. Python imported type aliases from
  `typing`, `collections.abc`, and `asyncio` carry `ImportedBinding`
  symbol-evidence dependencies, and rebound aliases stop emitting
  parameter-domain evidence.
  Imported Python stdlib alias-derived `Domain` evidence now carries
  `nose.python.stdlib.type_domain` pack provenance, making this the first
  compiled first-party pack-shaped pilot surface.
  Python builtin collection-factory `LibraryApi` occurrence evidence for
  `list`, `set`, `frozenset`, and `tuple` now carries
  `nose.python.builtins.collection_factories` pack provenance while the existing
  shadow and wildcard-import hard negatives stay closed.
  Python imported `collections.deque` collection-factory `LibraryApi`
  occurrence evidence now carries
  `nose.python.stdlib.collection_factories` pack provenance while missing-import
  and wrong-module hard negatives stay closed.
  Ruby stdlib `Set.new` collection-factory `LibraryApi` occurrence evidence now
  carries `nose.ruby.stdlib.set` pack provenance while missing-require,
  shadowed-`Set`, and mutated-set hard negatives stay closed.
  Rust stdlib `Vec::new` and `vec!` collection-factory `LibraryApi` occurrence
  evidence now carries `nose.rust.stdlib.vec` pack provenance while shadowed
  `Vec` roots and shadowed `vec` macros stay closed.
  Selected Rust stdlib `std::collections::{HashSet,BTreeSet,VecDeque}::from`
  collection-factory `LibraryApi` occurrence evidence now carries
  `nose.rust.stdlib.collection_factories` pack provenance while shadowed `std`
  roots stay closed. Selected Rust stdlib
  `std::collections::{HashMap,BTreeMap}::from` map-factory `LibraryApi`
  occurrence evidence now carries `nose.rust.stdlib.map_factories` pack
  provenance while shadowed `std` roots stay closed. Swift stdlib
  `Array(sequence)`, `Set(sequence)`, and
  `Dictionary(uniqueKeysWithValues:)` `LibraryApi` occurrence evidence now
  carries `nose.swift.stdlib.collection_factories` pack provenance while
  shadowed type names, wrong labels, implicit tuple-entry shape, and duplicate
  static keys stay closed. Java stdlib
  `java.util.Map.of` and `Map.ofEntries` map-factory `LibraryApi` occurrence
  evidence now carries `nose.java.stdlib.map_factories` pack provenance while
  missing-import and cross-surface `Map.entry` boundary cases stay closed.
  Java stdlib `java.util.Map.entry` map-entry `LibraryApi` occurrence evidence
  now carries `nose.java.stdlib.map_entries` pack provenance while
  missing-import and shadowed-root cases stay closed.
  Java stdlib `java.util.List.of`, `Set.of`, and `Arrays.asList`
  collection-factory `LibraryApi` occurrence evidence now carries
  `nose.java.stdlib.collection_factories` pack provenance while missing-import
  and cross-surface constructor boundary cases stay closed. Guava
  `ImmutableList.of`, `ImmutableSet.of`, and `ImmutableMap.of` `LibraryApi`
  occurrence evidence now carries
  `nose.java.ecosystem.guava.immutable_collection_factories` pack provenance
  while `copyOf`, missing-import, wrong-package, local-shadow, static-null,
  duplicate static map-key, and unsupported-arity cases stay closed. Java empty
  `new ArrayList<>()` and `new LinkedList<>()` collection-constructor
  `LibraryApi` occurrence evidence now carries
  `nose.java.stdlib.collection_constructors` pack provenance while
  missing-import, local-shadow, and conflicting-import cases stay closed.
  Java stdlib `java.util.Arrays.stream` static collection adapter `LibraryApi`
  occurrence evidence now carries
  `nose.java.stdlib.static_collection_adapters` pack provenance while
  missing-import and shadowed-root cases stay closed.
  `nose-semantics` resolves receiver-domain evidence through a shared
  `DomainRequirement` contract. Consumers check exact receiver node evidence
  first, then immutable binding evidence for local or module variables, then
  scoped parameter evidence, and fail closed on
  ambiguous/conflicting/dependency-broken records without consulting a
  side-table mirror. Desugaring/idiom canonicalization, post-desugar value-graph
  receiver gates, and strict exact receiver gates consume this same helper layer
  through the shared `ReceiverDomainEvidenceIndex` cache. Desugaring and early
  idiom canonicalization still run before normalize emits additional immutable
  binding-domain evidence and therefore only see domain evidence already present
  at that point. This preserves the current
  Array/Collection/Set/Map/Option/String/Integer/Number and ByteArray
  distinctions. First-party producers also attach receiver-expression domain
  facts directly for selected admitted library/API factory results, and normalize
  emits binding-anchored `Domain` evidence with matching `nose.lang.*`
  language-core provenance for single-assignment local/module bindings whose
  initializer has asserted sequence or result-domain evidence and whose binding
  has no direct binding-write, receiver-mutation, or opaque-argument-escape risk
  under first-party `Effect` evidence. Binding-domain lookup matches the binding
  `local_hash` and only applies an assignment to
  receiver uses that occur after it. Strict exact receiver gates consume this
  resolver directly instead of caching raw collection/map names or CIDs from an
  assignment scan. Domain evidence can satisfy a receiver-domain precondition,
  but it is not exact-tree proof for the binding value: an opaque initializer
  with `Domain(Collection)` still does not make the variable generally
  exact-safe. The current mutation-risk producers are conservative and
  language-scoped; they invalidate exact assumptions but do not prove exact
  library semantics. Rust `sort_by_key` is now included in the receiver-mutation
  row so later receiver uses close before exact equivalence is considered.
- C byte-buffer and unsigned-cast alias proof is now evidence-backed. Local
  typedefs and direct quote includes emit `Type(CTypeAlias)` evidence for the
  currently supported exact-spelling `unsigned char` and unsigned 32-bit
  aliases; included aliases depend on `Import(CQuoteInclude)`. Alias-based
  `Domain(ByteArray)` parameter facts and `nose.lang.c` provenance-backed
  `Source(Cast(CUnsigned32))` facts depend on those type records. The C u16/u32
  byte-pack value-graph laws consume the first-party C byte-pack contract,
  byte-array domain proof, and source-cast proof where the u32 high lane
  requires it; raw `UnsignedCast32` payloads stay
  opaque without source-cast evidence.
- Property builtin contracts are language-constrained occurrence contracts, not
  selector guesses. JS/TS/Vue/Svelte/HTML and Java `length` reads are admitted
  only when a `LibraryApi(PropertyBuiltin(Len))` record is anchored to the
  `Field` node and its dependencies prove the receiver contract. JS-like
  `length()` is not a method-call cardinality contract. JS/TS
  `filter(...).length` is admitted only after the receiver has already entered
  a proven collection/HOF value and raw HOF calls carry admitted `LibraryApi`
  occurrence evidence. JS object `.length` remains a property read, not
  collection cardinality.
- Promise `.then` has a JS-like library API contract. Exact beta-reduction also
  requires Promise-like receiver proof and a supported settled-value producer;
  arbitrary `.then` methods and unsupported thenables remain opaque.
- Rust iterator identity adapters (`iter`, `into_iter`, `iter_mut`, `collect`,
  `to_vec`, `copied`, `cloned`) are language-, arity-, and receiver-proof
  constrained through `LibraryApiContract` and admitted `LibraryApi` occurrence
  evidence with `nose.protocols.iterator_identity_adapters` provenance. Java
  `.stream()` uses the same protocol pack. Normalize's exact protocol receiver
  admission consumes this same contract instead of accepting same-named methods
  from other languages. The Rust corpus audit [`rust-stdlib-partial-audit-2026-06-28.v2.json`](../bench/recall_loss/rust-stdlib-partial-audit-2026-06-28.v2.json)
  processes the 5,000+ HOF callback, mutation/effect, iterator-domain,
  Option/Result channel, and iterator-lifecycle groups without widening semantic
  admission, leaving receiver-domain proof as the largest remaining group.
- Java `Math.abs`, `Math.min`, and `Math.max` scalar integer APIs are language-,
  arity-, receiver-, and integer-domain constrained through admitted
  `LibraryApi` occurrence evidence with `nose.java.stdlib.math` provenance.
  JS/TS `Math.*` and Java floating `Math.*` forms remain exact-closed at the
  signed-zero/NaN boundary.
- Java/Rust/JS-family `map.get(key)` is language-, arity-, and exact-map
  receiver constrained through admitted `LibraryApi` occurrence evidence with
  `nose.protocols.map_get` provenance. Python `dict.get(key, default)`, Ruby
  `Hash#fetch(key, default)` or zero-arg block fallback, and Java
  `Map.getOrDefault(key, default)` are constrained through admitted
  `LibraryApi` occurrence evidence with
  `nose.protocols.map_get_default` provenance. Rust `unwrap_or` remains a
  separate Option/defaulting contract; when it models map lookup defaulting, the
  nested `MapGet` dependency must also be pack-proven.
- Receiver-method membership APIs are language-, arity-, and receiver-proof
  constrained through admitted `LibraryApi` occurrence evidence with
  `nose.protocols.receiver_membership` provenance. This covers Java/Rust/Ruby
  map-key membership, Python `__contains__`, JS-like `has`/`includes`,
  Java/Swift `contains`, and Ruby `member?`; Go `slices.Contains` remains a
  namespace-function contract outside this receiver-method protocol slice.
- Rust method `zip(...)` is admitted as a protocol-pair operation only through
  the Rust library method-call occurrence contract and exact protocol proof for
  both sides.
- Rust stdlib path contracts for `Some`/`Option::Some`,
  `None`/`Option::None`, `Option::and_then`, and `Vec::new` carry the exact
  selector and shadow-root requirement through `nose-semantics`. First-party
  lowering/normalization now emits admitted `LibraryApi` occurrence evidence for
  `Some(...)` calls, `Some(_)` pattern selectors, bare `None` `Var`
  occurrences, and `and_then(...)` calls only when the shadow and receiver
  obligations are satisfied. `Some(_)` pattern predicates also require the Rust
  tuple-struct wildcard `Source::Pattern` fact at the pattern span; the API
  occurrence alone is only selector proof. The Rust frontend preserves `if let`
  pattern tests instead of lowering them directly to null/not-null builtins, so
  Option absence/presence is admitted only through the contract-backed occurrence
  path plus required source-surface evidence.
- Collection factory, map factory, map-entry, and selected constructor identity now have an
  internal `LibraryApiContract`
  shape in `nose-semantics`. It separates API identity from result eligibility,
  so callers can distinguish "this is Java `Arrays.asList`" from "this argument
  can be canonicalized as a membership collection." Shared contracts cover
  Python free-name factories (`list`, `set`, `frozenset`, `tuple`), Python
  imported `collections.deque`, Rust
  `std::collections::{HashSet,BTreeSet,VecDeque,HashMap,BTreeMap}::from`, Rust
  `vec!`/`Vec::new`, Java `List.of`/`Set.of`/`Arrays.asList`, Java
  `new ArrayList<>()`/`new LinkedList<>()`, Java `Map.of`/`Map.ofEntries`/
  `Map.entry`, Ruby `require "set"; Set.new(...)`, and JS-like `new Set(...)`/
  `new Map(...)`. Normalize and strict exact gates consume this shared contract
  source. Producer-covered surfaces additionally require admitted `LibraryApi`
  occurrence evidence whose dependencies carry the local import, earlier
  top-level require, unshadowed-global, macro-invocation source,
  construct-syntax, or regex-literal proof. Selected producer-covered result
  calls emit dependent `Domain` evidence for the result receiver:
  collection-like factories as `Collection`, set factories/constructors
  including JS-like `new Set` with
  `nose.javascript.builtins.collection_constructors` provenance as `Set`, map
  factories including JS-like `new Map` with the same pack provenance as `Map`,
  JS-like one-argument `Array.from` with
  `nose.javascript.builtins.array` provenance as `Array`, and JS-like
  `Promise.resolve` plus admitted Promise `.then` as `PromiseLike`. Java
  `Arrays.asList(x)` with exactly one argument is excluded because
  array-spread versus single-element provenance is ambiguous without additional
  proof. `Map.entry`, `Array.isArray`, `Boolean`, regex `.test`, `math.prod`,
  `Arrays.stream`, pack-proven map `get`, pack-proven map get-default, iterator
  adapters, and generic method contracts do not emit result-domain evidence
  under the current vocabulary.
  Entry-shape, mutation, demand, and exact-safety obligations remain
  separate contract checks at the consumer.
- Selected non-factory library/API surfaces also consume `LibraryApiContract`
  rows before normalize, value-graph, or strict exact paths assign semantics.
  Current rows cover map-key views and wrappers, Java/Rust/JS-like pack-proven
  map `get`, Python/Java/Ruby pack-proven map get-default, Rust
  `get(...).is_some()`/`unwrap_or(...)`, JS-like `Array.isArray`, `Boolean(...)`,
  regex-literal `.test(...)`, Python `math.prod`, promise `.then`, Rust/Java
  iterator adapters, Java `Arrays.stream`, and the language-scoped method-call
  surfaces already admitted by `method_call_contract`. These rows carry callee
  identity and result obligations; local consumers still prove receiver domain,
  import/symbol identity, source facts, exact-safe arguments, fallback demand
  shape, and mutation safety.
- Selected API call occurrences now also have `LibraryApi` evidence records when
  they remain as raw call nodes. First-party lowering emits occurrence evidence
  for JS-like `Array.from(...)`, `Array.isArray(...)`, `Boolean(...)` with
  `nose.javascript.builtins.boolean` provenance, regex literal `.test(...)`
  with `nose.javascript.builtins.regex` provenance, `new Map(...)`,
  `new Set(...)`, and static `indexOf`/`findIndex` membership calls whose
  receiver is a proven static non-float collection literal with
  `nose.javascript.builtins.static_index_membership` provenance; JS-like ESM
  named imports and conservative `const` CommonJS destructuring requires from
  `node:timers/promises` or `timers/promises` for `setTimeout` and
  `setImmediate` with
  `nose.javascript.node.timers_promises` provenance, including fulfilled
  `PromiseSettledValue` records for exactly `setTimeout(delay, value)` and
  `setImmediate(value)`; Python builtin
  collection factories such as `list(...)` when the callee is proven as an
  unshadowed free name; Python
  `collections.deque(...)` when the callee is proven through
  `from collections import deque`, an alias such as
  `from collections import deque as Values`, or `import collections;
  collections.deque(...)`; Python `import math; math.prod(...)`; Rust
  `vec!(...)` when source syntax proves a macro invocation, `Vec::new()`, and selected
  primitive integer `abs`/`min`/`max`/`clamp` receiver methods with
  `nose.rust.stdlib.integer_methods` provenance when exact integer receiver
  proof is present, Java `Math.abs`, `Math.min`, and `Math.max` scalar integer
  APIs with `nose.java.stdlib.math` provenance when unshadowed `Math` and
  integer-domain proof are present, and selected
  `std::collections::{HashSet,BTreeSet,VecDeque,HashMap,BTreeMap}::from(...)`
  factory paths when their root-shadow policy is proven; Ruby
  `require "set"; Set.new(...)` when an earlier top-level `Import::Require("set")`
  depends on unshadowed `require` proof and unshadowed `Set` receiver proof
  exists; Java `java.util` static factories/adapters such as `List.of`,
  `Set.of`, and `Arrays.asList` with
  `nose.java.stdlib.collection_factories` provenance, `Map.of`/`Map.ofEntries`
  with `nose.java.stdlib.map_factories` provenance, `Map.entry` with
  `nose.java.stdlib.map_entries` provenance, `Arrays.stream` with
  `nose.java.stdlib.static_collection_adapters` provenance, plus selected empty
  `new ArrayList<>()`/`new LinkedList<>()` constructors with
  `nose.java.stdlib.collection_constructors` provenance; and
  JS-like regex-literal `.test(...)`. These records depend on
  the relevant `QualifiedGlobal`,
  `UnshadowedGlobal`, import-backed call-site `Symbol`, `Import::Require`,
  macro-invocation `Source`, construct-syntax `Source`, `SequenceSurface`, or
  regex-literal `Source` evidence. Calls
  collapsed into specialized guard surfaces emit guard evidence instead.
  `nose-semantics` resolves these records with a three-state result: admitted,
  missing, or rejected. Value-graph, idiom, strict exact, and provider snapshot
  consumers for these producer-covered surfaces require admitted occurrence
  evidence; missing, conflicting, or dependency-broken API evidence keeps the
  exact path closed. Older import/symbol/source facts are still required
  dependencies of the occurrence evidence, but they no longer act as fallback
  API-identity proof for these surfaces. Where a producer emits result-domain
  evidence, that `Domain` record depends on the `LibraryApi` occurrence record,
  so broken API proof also closes result-domain proof. Normalize-owned
  result-domain, helper receiver-domain, and helper call-site `Symbol`
  occurrence facts use the lowered file language's builtin language-core
  provenance, while the licensing `LibraryApi` occurrence keeps the specific
  builtin pack provenance; legacy broad `nose.first_party` rows are updated in
  place when the same asserted fact is refreshed, and stale broad duplicates are
  closed when an equivalent current row already exists. The `LibraryApi` record
  itself proves API identity only; source, exact-safe argument, result-shape,
  mutation, and demand/effect obligations remain separate.
- Receiver-method calls that remain as raw `Field`/`Call` nodes now emit
  `LibraryApi` occurrence evidence for the first-party method families currently
  backed by `LibraryApiContract`: pack-proven map `get`, pack-proven map
  get-default, map-key views, iterator identity adapters, and pack-proven
  builtin method-call contracts such as collection/map membership, count methods,
  string/collection predicates, Rust scalar integer methods with
  `nose.rust.stdlib.integer_methods` provenance, Java Math scalar integer
  methods with `nose.java.stdlib.math` provenance, fully-qualified Java
  `Optional.isPresent`/`orElse`, Rust `Option::and_then`, Rust `zip`, and
  HOF/reduction methods. Rust iterator HOF and terminal rows use
  `nose.protocols.sequence_hof_adapters` provenance rather than the generic
  method-call pack. The
  occurrence record is admitted only for the exact language/method/arity row and
  depends on receiver proof: node/binding/parameter `Domain`, `SequenceSurface`,
  imported namespace or unshadowed-global `Symbol`, or a nested admitted
  `LibraryApi` result such as a collection/map factory, map-key view, iterator
  adapter, HOF, pack-proven map `get`, or pack-proven map get-default.
  First-party lowering seeds these records when the receiver proof already exists; normalize refreshes and upserts first-party
  records after immutable binding-domain inference and again after final
  CFG/dataflow/algebra rewrites, so bindings such as
  `VALUES = List.of(...); VALUES.contains(x)` keep the same semantic fingerprint
  as direct factory receivers without reopening selector-only fallbacks.
  Normalized HOF receivers keep their same-span admitted `MethodCall(HoF(...))`
  occurrence as protocol evidence, so downstream adapters such as Rust
  `.collect()` can consume a canonicalized `filter_map` receiver without trusting
  the `collect` selector alone; for Rust iterator HOFs this same-span occurrence
  is admitted under `nose.protocols.sequence_hof_adapters`. Swift
  `map`/`filter`/`flatMap` same-span occurrences use the same pack only when
  their receiver proof is Array/Collection rather than arbitrary `Sequence`,
  `Set`, or `Dictionary`, so chained Swift HOFs can reuse pack-backed receiver
  proof without opening one-shot or ordering assumptions. The Swift corpus audit [`swift-stdlib-partial-audit-2026-06-28.v2.json`](../bench/recall_loss/swift-stdlib-partial-audit-2026-06-28.v2.json)
  processes the 5,000+ cardinality receiver-proof group as existing-contract
  coverage, keeping selector-only `count`/`isEmpty` closed without
  ExactCollection proof. Ruby
  `map`/`collect`/`select`/`filter`/`reject` same-span occurrences use the same
  pack only when their receiver proof is Array/Collection rather than Hash,
  Set, lazy Enumerator, or framework relation; chained Ruby HOFs can reuse
  pack-backed receiver proof, and `reject` carries `Not(predicate)` in the value
  graph. Value-graph filter
  consumers such as `len(filter(...))`, explicit reductions over a filter, and
  static literal membership shortcuts reuse HOF admission as well, so raw
  `HoF(Filter)` cannot bypass the source/API HOF gate by appearing under another
  operation.
- Type/domain evidence now has vocabulary for arrays, collections, iterables,
  iterators, sets, maps, records, options, results, promise/future-like values,
  strings, booleans, integer/float/number distinctions, byte arrays, and hashed
  nominal domains. `Type(NominalDomain)` rows can connect provider-proven
  nominal type identities to domains, while raw type names still do not prove
  semantics. The `ValueDomain` bridge remains deliberately narrow:
  integer/float/number, boolean, string, and array/collection/set facts can seed
  current value laws; iterable, iterator, record, option/result, future-like, and
  nominal facts stay separate until a consumer names those obligations.
- Java empty collection constructor contracts cover `new ArrayList<>()` and
  `new LinkedList<>()` through `LibraryApiContract` rows only for the Java
  `java.util` list types. Simple names require exact `java.util` import proof or
  earlier `java.util.*` wildcard import proof, plus no local type declaration
  with the same simple name. A `java.util.*` wildcard import is not enough when
  another package explicitly imports the same simple type; fully-qualified
  `java.util.*List` names carry the namespace proof in the selector itself.
  Builtin Java lowering preserves these supported constructors as construct
  `Call` nodes and emits admitted `LibraryApi` occurrence evidence with
  `nose.java.stdlib.collection_constructors` pack provenance. Value-graph
  collection canonicalization and result `Domain(Collection)` evidence require
  that occurrence proof, so source/import facts alone do not reopen the exact
  path.
- Builder append contracts are separate from arbitrary method calls. A selector
  such as `push`, `append`, or `add` is not proof by itself. First-party
  frontend/normalize paths must prove the receiver or active-builder contract,
  lower the call to canonical `Builtin::Append`, and attach
  language-core `Effect(BuilderAppendCall)` through explicit same-span
  language/API evidence
  before exact fragments can treat it as an append effect. Value-graph active
  list builders require emitted effect evidence, an admitted same-span
  `LibraryApi(MethodCall(Builtin(Append)))` occurrence, or the first-party
  builder-append method-effect row, always under active-builder receiver context;
  selectors outside those rows never reopen the path by themselves.
  Active map-builder recognition similarly consumes an index-write contract row:
  Python `d[k] = v` requires `Effect(BindingWrite)` plus an active map-builder
  receiver seeded by an explicit map surface, while other languages need their
  own row or the separate non-overloadable-index evidence path. Raw selectors,
  raw index assignment, raw tuple values, and untagged sequence values no longer
  reopen collection/map builder semantics by themselves.
- Exact fragment production is now contract-first: the collector admits
  statement fragments through `fragment::recognize::recognize_contract`, while
  the older predicate matrix remains as a debug/differential guard. Surface
  proofs for Java `this.field`, Java `return this`, non-overloadable C/Go/Java
  index assignment, and single-item builder append calls are shared through
  `nose-semantics`, and contract recognizers consume the same IL-level proof
  helpers. Raw selector-only append calls stay exact-closed as append effects,
  though they may still participate in the separate opaque-call policy as
  generic `Other` effect context.
- Value-graph and oracle same-unit field state are evidence-gated. A cached
  write/readback/final field sink is admitted only for the current builtin
  language-core self-field substrate: Java `this.field` proven by
  `Place(SelfReceiver)`, `Place(SelfField)`, and `Effect(SelfFieldWrite)`. Raw
  dynamic attribute or property spellings, including Python `self.x`, do not
  prove exact field state; they remain ordered effects or unsupported until a
  pack supplies explicit place/effect evidence.
- Exact-fragment place/effect gates now have builtin language-core evidence
  provenance. Frontend lowering and normalize refreshes emit
  `Place(SelfReceiver)` and `Place(SelfField)` for Java `this`/`this.field`,
  plus `Effect` evidence for canonical builder append calls, C/Go/Java
  non-overloadable index writes, and Java self-field writes. Fragment
  recognizers require these records; missing, conflicting, ambiguous, or
  dependency-broken place/effect evidence closes the exact path instead of
  reopening a language/shape fallback. Self-field place/write records depend on
  the matching receiver/place records.
- `SeqSurfaceContract` now centralizes first-party lowered sequence tags and
  keeps separate axes for exact-tree safety, membership-collection admission,
  map-entry-list admission, imported-literal eligibility, and value-graph
  canonical tags. Strict exact gates, value-graph sequence lowering, and
  sibling-module literal export checks consume this contract only through
  matching `SequenceSurface` evidence with builtin language-core provenance
  rather than raw tag spelling or local string allowlists. Missing surface
  evidence now lowers to the untagged sequence value in the value graph, not a
  spelling-derived raw hash. Untagged `Seq` remains an internal grouping surface
  and does not itself prove exact collection semantics; the older Python empty
  sequence collection case is handled only by the explicit collection profile
  path. Rust struct literals use `SequenceSurface(RustStructExpression)`, which
  is exact-tree-safe but stays separate from collection, map, membership,
  map-entry, and imported-literal contracts.
- Collection reductions such as Rust `Iterator::count()` and Java
  `Stream.count()` are admitted through library method contracts plus exact
  protocol receiver proof, not through a bare method-name check.
- Selected value-graph library consumers now call shared admitted occurrence
  resolvers in `nose-semantics` for method, imported-namespace function,
  iterator-adapter, Rust Option/`Vec::new`, direct factory/constructor eval,
  node-level property builtins, Rust `Some` callee-node checks, static
  index-membership, Rust scalar integer method calls under
  `nose.rust.stdlib.integer_methods`, Java Math scalar integer calls under
  `nose.java.stdlib.math`, and builder append API admission instead of
  recombining raw selector parsing with evidence admission locally. Normalize
  idiom canonicalization uses the same resolver layer for
  supported free-function builtins, pack-proven Python iterator builtin HOFs,
  pack-proven builtin method contracts, HOF receiver proof, pack-proven map
  `get`, pack-proven map get-default, pack-proven map-key views,
  iterator/static collection adapters, Rust `Some(...)`, Rust map factory
  receiver proof, Promise `resolve`, and Promise `.then` contract lookup.
  Promise continuation reduction remains fail-closed
  unless a supported settled value can be recovered and the final value remains
  behind a Promise boundary. Value-level CSE paths that query
  by call span now use span-query resolvers for free-name/imported collection
  factories, Java/Ruby/Rust collection factories, free-name/Java map factories,
  Java map entries, pack-proven map `get`, pack-proven map get-default, and
  pack-proven map-key view/wrapper calls.
- The Python iterator-builtin protocol pack now owns lazy builtin iterator
  producer evidence for `map`, `filter`, `zip`, and `enumerate`, plus terminal
  occurrence evidence for `any` and `all`. `map`/`filter` become normalized
  HOFs only when the occurrence has `nose.protocols.iterator_builtins`
  provenance, unshadowed builtin proof, iterable-source proof, and a lambda
  callback shape. `list`/`tuple`/`set` materializers consume lazy iterator
  producers only when both the collection factory proof and producer/source
  proof are present. `list(map(...))` keeps the existing list-comprehension
  convergence, while `tuple`, `set`, and `frozenset` materializers keep distinct
  terminal identity. Shadowed builtins, wildcard-import ambiguity, missing source
  proof, callable-but-not-lambda callbacks, missing materializer proof, invalid
  nested producer evidence, multi-iterable `map`, and `sorted`/`reversed` remain
  closed. The corpus audit [`python-hof-runtime-audit-2026-06-28.v3.json`](../bench/recall_loss/python-hof-runtime-audit-2026-06-28.v3.json)
  processes the 5,000+ materializer-domain group as a boundary split, keeping
  `list`/`set`/`tuple`/`frozenset` gated by existing LibraryApi occurrence,
  unshadowed builtin proof, source-iterator provenance, and result-domain proof.
- Opaque exact callee identity remains separate from library/API admission. A
  parameter callee or proof-backed immutable/imported callee may keep an exact
  same-callee call comparable as an opaque value operation. Same-spelled
  file-local functions still require `CallTarget::DirectFunction` evidence,
  imported function/member calls can enter opaque identity only through explicit
  `CallTarget::ImportedFunction` or `CallTarget::ImportedMember` records. The
  normalize call-target producer now emits these records and their call-site
  imported symbol occurrence dependencies with the file language's builtin
  language-core pack provenance. Public node-anchored `UnshadowedGlobal` and
  `ImportedNamespace` identity helpers consume only matching builtin
  language-core, asserted, dependency-valid occurrence proof. `LibraryApi`
  occurrence admission now applies the same gate to `UnshadowedGlobal` and
  `ImportedNamespace` symbol prerequisites, so broad or wrong-provenance symbol
  rows cannot license free-name, namespace, or receiver-method API evidence;
  import-binding prerequisites remain on the import-backed binding path. The
  shared call-target resolver and value-DAG referent extraction consume only
  matching builtin language-core, asserted, dependency-closed,
  selector-matching evidence, while library semantics still require admitted API
  occurrence evidence.
- Java stream source adapters are split by proof through library API contracts:
  `receiver.stream()` requires an exact iterable receiver, while
  `Arrays.stream(xs)` requires the `java.util.Arrays` import binding and no local
  `Arrays` type shadow.
- Cross-file immutable import replacement now copies the provider's closed
  evidence subgraph for the exported literal expression, so a Java static import
  of `LOOKUP = Map.of(...)` carries the provider's `java.util.Map` proof into
  the importing file only when the provider emitted that import proof. Copied
  provider nodes and evidence anchors keep provider source-origin spans, while
  copied dependency ids are rewired inside the importer IL; this prevents
  importer-local scopes or same-named classes from shadowing provider-proven API
  occurrences. The replacement records `ImportedLiteralSnapshot` provenance
  depending on the importer static import proof plus copied provider evidence.
  Provider-side literal export safety now consumes a shared `nose-semantics`
  helper that admits concrete root literals, requires sequence-surface proof for
  literal containers, uses the Go zero-map literal/entry contracts for imported
  Go map values, and uses shared admitted occurrence resolvers for Java/Rust map
  factory calls plus JS/TS `new Map(...)` and `new Set(...)` constructor calls;
  raw import-coordinate sequences remain rejected as provider literal children.
  Go namespace-member consumers such as `tables.Lookup` can be
  replaced with a provider snapshot only when the namespace import proof is
  asserted, the provider export is unique and immutable, the consumer namespace
  is not rebound or parameter-shadowed, and the selected member is not written,
  receiver-mutated, or passed to an opaque escaping call.
  Provider and importer module-binding mutation proof now consumes shared
  mutation-risk `Effect` evidence and rejects direct binding mutations, direct
  place writes such as `LOOKUP.clear()`, `LOOKUP.push(...)`, and
  `LOOKUP[key] = value`, and provider-side opaque argument escapes such as
  `mutate(LOOKUP)`, before imported literal provenance can enter exact
  matching.
- Membership and map-key membership selectors now consume language-scoped
  library method contracts before normalize/detect treat them as semantic
  containment. A method named `contains` is Java/Rust collection membership
  only; JavaScript `.contains(...)` is not accepted as array membership. Map-key
  examples include Java `Map.containsKey`, Java `keySet().contains`, Rust
  `contains_key`, Rust `get(key).is_some()`, Ruby `key?`/`has_key?`, Python
  `__contains__`, and TypeScript `Array.from(map.keys()).includes(key)` when the
  receiver is a typed/proven map.
- Map key-view library contracts distinguish collection views from iterator views:
  Python/Ruby `keys` and Java `keySet` are collection views, while JS-like
  `Map.keys()` is an iterator view. JS/TS `Object.keys(obj)` is a collection
  view only when `Object.keys` has qualified-global proof and `obj` is a proven
  static object literal or unique unescaped local binding to one. Those key-view
  occurrences report `nose.protocols.map_key_views` provenance and need either
  exact-map receiver proof or the static object-argument proof for
  `Object.keys`.
  JS-like iterator views still need the `Array.from(...)` wrapper contract plus
  `QualifiedGlobal("Array.from")` symbol evidence before they can feed exact
  membership. That qualified-global record must depend on same-span source proof
  that the `Array` root is unshadowed.
- Map lookup surfaces that return a value/option are now explicit library API
  contracts for Java/Rust/JS-like `get(key)` plus an exact-map receiver
  requirement. Those `MapGet` occurrences report `nose.protocols.map_get`
  provenance. Python `dict.get(key, default)`, Java `getOrDefault`, and Ruby
  `fetch` still use the `GetOrDefault` method contract. Rust
  `get(key).unwrap_or(default)` is modeled as `GetOrDefault` only through the
  nested pack-proven `MapGet` dependency on the `unwrap_or` occurrence. Ruby
  `fetch(key) { fallback }` carries a separate zero-arg-lambda fallback argument
  contract, so block fallback demand is not inferred from the selector name in
  normalize/detect.
- JS-like static array `indexOf`/`findIndex` membership surfaces are explicit
  `LibraryApi` occurrence contracts, including the static non-float literal
  collection requirement and accepted `-1`/`0` threshold comparisons through
  `OperatorSemantics`. The occurrence record depends on
  `SequenceSurface(Collection)` evidence for the exact receiver, and value-graph
  and strict exact consumers require that admitted call occurrence before
  treating a threshold comparison as membership. Callback membership variants
  also require source operator facts: JS-like strict equality/inequality can
  enter exact matching, while loose equality, `instanceof`, and non-JS equality
  surfaces stay closed for these contracts. Callers still prove the receiver and
  lambda equality shape before exact normalization/detection accepts them.
- Source `Op::In` is not proof by itself. Strict exact collection/map
  membership currently admits Python `in` only through a language-scoped
  membership-operator contract plus receiver evidence. JS `in` remains
  exact-closed for collection membership because it means property/key existence,
  not array element membership.
- Imported namespace function contracts now cover Python `math.prod` as a product
  reduction only when the receiver is proven to be the imported `math` namespace.
  Bare globals named `math` and overwritten module bindings stay exact-closed.
- Java integer `Math.abs`/`Math.min`/`Math.max` now lower through scalar-integer
  method contracts with `nose.java.stdlib.math` provenance, an unshadowed
  `Math` receiver requirement, and integer-domain proof for value arguments
  instead of frontend text-only builtin lowering. JS-like
  `Math.abs`/`Math.min`/`Math.max` stay exact-closed until a signed-zero and
  NaN-aware numeric model exists; Go `math.Abs`/`math.Min`/`math.Max` and Java
  floating `Math.abs`/`Math.min`/`Math.max` stay closed for the same reason.
- Two-argument free `min(...)`/`max(...)` normalization consumes the Python
  free-function builtin `LibraryApi` occurrence contract plus integer-domain
  proof. Same-named functions from other languages, including JS `min(...)`,
  locally shadowed Python names, manually constructed calls without admitted
  occurrence evidence, and float/NaN-sensitive operands stay exact-closed. Python
  free `abs(...)` and sign-test absolute-value ternaries also require
  integer-domain proof before they use the modeled Abs node, so untyped and
  element-derived operands keep the signed-zero boundary closed.
- User-defined and imported opaque call identity now consume `CallTarget`
  evidence. The builtin language-core producer admits `DirectFunction` records
  for unique top-level in-file function targets with no current or enclosing
  lexical shadowing, and imported function/member records only from
  dependency-backed imported binding or imported namespace symbol proof at the
  call occurrence. Same raw callee spelling, same field selector spelling, stale
  broad-provenance rows, wrong-language provenance, external rows, rebinding,
  ambiguous import proof, conflicting symbol evidence, dependency-broken proof,
  and locally visible same-name function units stay closed. Recursion
  normalization, the interpreter oracle, value-graph pure helper inlining,
  value-DAG referents, and strict exact direct-function callee gates no longer
  treat same raw callee spelling as call-target proof. The shared resolver also
  understands `DirectMethod` and `DynamicDispatch` records, but no builtin
  producer emits them yet. Strict exact admits imported function/member identity
  only through explicit evidence, requires exact receiver identity for direct
  methods, treats dynamic-dispatch records as non-concrete by themselves, and
  closes on selector mismatch, dependency-broken records, wrong provenance, or
  conflicting target evidence.
- JS-like `typeof` exact-safety now consumes a language- and arity-constrained
  operator contract plus `Source::Operator(Typeof)` evidence at the call span.
  A raw `Call(Var("typeof"), arg)` shape, same-named function from another
  language, or unresolved provider is not treated as the JS operator.
- JS-like `Array.isArray(...)` exact-safety now consumes a static-global method
  contract and requires the `Array` global to be unshadowed through the
  qualified-global record's root dependency.
- JS-like record-shape guards that use `Boolean(value)` as the non-null/truthy
  clause consume the pack-owned static-global function contract and require the
  `Boolean` global to be unshadowed. `value !== null` and `!!value` remain
  available when their own clauses prove the same record shape. The collapsed
  `Seq("record_guard")` is no longer admitted by tag spelling alone: strict
  exact and value-graph paths require matching `SequenceSurface(RecordGuard)` and
  `Guard::JsRecordShape` evidence, including subject identity, null/truthiness
  form, comparison form, and asserted API dependencies for `Array.isArray` plus
  optional `Boolean`.
- JS/TS own-property guards are also evidence-backed. The frontend emits
  `Guard::JsOwnProperty` for admitted `Object.hasOwn(obj, key)` and
  `Object.prototype.hasOwnProperty.call(obj, key)` surfaces, with a dependency
  on the corresponding `QualifiedGlobal` proof, which in turn depends on
  same-span unshadowed `Object` root evidence. Strict exact and value-graph
  map-default paths require both `SequenceSurface(OwnPropertyGuard)` and that
  dedicated guard evidence; raw `Seq("own_property_guard")`, object method
  spellings, detached API evidence, and shadowed `Object` roots stay closed.
- JS-like `undefined` is no longer frontend-collapsed to null unconditionally.
  It is preserved as a name and only treated as the nullish sentinel through an
  unshadowed-global contract. Value-graph nullish-value evaluation now requires
  asserted `Symbol(UnshadowedGlobal("undefined"))` evidence instead of falling
  back to raw spelling plus a file-scope scan; strict exact-safe gates consume
  the same proof, so temp-bound `Map.get(...)` defaulting can stay open without
  admitting shadowed `undefined` bindings.
- Go literal map default lookup is represented by shared contracts for both the
  outer `composite_literal` and per-entry `keyed_element` sequence surfaces plus
  the supported zero-default payload classes. Normalize and strict exact paths
  require matching `SequenceSurface(GoCompositeMapLiteral)` and
  `SequenceSurface(GoMapEntry)` evidence with Go language-core provenance, so
  raw tag spelling alone is not enough. Go `composite_literal` no longer falls
  back to a generic collection sequence tag; it is consumed only by the Go map
  contract or left as a distinct surface.
- Static JS-like `indexOf`/`findIndex` membership requires a call occurrence
  with `nose.javascript.builtins.static_index_membership` provenance whose
  receiver sequence surface has membership-collection admission. Untagged
  sequence expressions, destructuring surfaces, float literals, and other
  positional groupings do not become static array membership merely because
  their children are literals.
- JS/TS object literals preserve static property keys in exact map/object
  semantics, but computed property names are exact-closed until a future
  contract can prove key evaluation, coercion, order, and side-effect behavior.
  The `Object.keys` key-view slice consumes only object literals whose keys are
  static strings and whose lowered object surface has `SequenceSurface(Map)`
  evidence. For local bindings it also requires the initializer's
  `BindingWrite` effect and rejects intervening mutation or argument escape
  before the `Object.keys` use, including JS `delete` property mutation and
  `for...in` / `for...of` loop target writes. Nested local function
  declarations that could close over the object, direct `eval`, and `with`
  scopes over or enclosing the object use also close the proof.
  Object-literal `__proto__` prototype syntax is exact-closed because it is not
  an enumerable own key; escaped identifier keys and numeric literal keys whose
  runtime property names need JS canonicalization are also exact-closed.
- JS/TS `new Map(...)` and `new Set(...)` now require construct-syntax source
  facts distinct from ordinary calls, `UnshadowedGlobal` symbol proof for the
  `Map`/`Set` constructor, and `nose.javascript.builtins.collection_constructors`
  `LibraryApi` provenance. With exact-safe static collection or entry arguments
  they can enter exact matching, including supported immutable module-level
  Set/Map bindings. Plain `Set(...)`/`Map(...)` calls and locally shadowed
  constructor names remain exact-closed.
- JS/TS regex literal `.test(...)` now requires regex-literal source fact proof
  plus `nose.javascript.builtins.regex` `LibraryApi` provenance. String
  receivers or unsupported arities remain exact-closed even when the selector is
  named `test`.
- Static import proof facts now have a typed `ImportFactKind`/`ImportFact`
  facade in `nose-semantics`. First-party frontends emit import binding and
  namespace facts through that contract. The lowered RHS keeps only structural
  coordinate literals; proof lives in `EvidenceRecord::Import` and binding
  `Symbol` evidence. Value-graph import identity consumes sequence `Import`
  evidence into dedicated `ImportNamespace`/`ImportBinding` value ops, so raw
  import coordinate sequences can no longer become proof-bearing value nodes by
  tag shape. Imported literal replacement also consumes evidence-only import
  facts; missing or ambiguous `Import` evidence no longer proves a cross-file
  replacement. Rust unrestricted `pub use` and crate-visible `pub(crate) use`
  re-exports, including nested static brace aliases, now emit
  `Import(ReExportBinding)` as alias proof, not value proof; imported literal
  replacement may follow one such same-corpus hop only to an already
  literal-safe provider export.
- Symbol identity evidence now covers static imported binding/namespace aliases
  and JS/TS static-global value occurrences such as `Math`, `console`, `Array`,
  `Map`, `Set`, and `undefined` when the frontend proves no local shadow.
  Normalize idiom admission, value-graph namespace fallbacks, and strict exact
  gates consume `nose-semantics` symbol-proof helpers; imported binding/namespace
  symbol helpers no longer fall back to raw import assignment RHS parsing.
  Selected JS/TS qualified static global paths now emit `QualifiedGlobal`
  evidence as well: `Object.hasOwn` and
  `Object.prototype.hasOwnProperty.call` gate own-property guards, while
  `Array.from` gates JS-like map-key iterator wrappers and `Object.keys` gates
  static-object map-key views. The path evidence is not enough by itself:
  consumers require its dependency on same-span
  `UnshadowedGlobal` root proof. This does not cover all qualified members or
  namespace exports.
  A spelling such as `Math`, `fmt`, or `deque` is still only a selector; exact
  consumers need symbol identity proof plus the language/API contract. Binding
  evidence does not prove later uses if the alias is rebound or ambiguous.
- TypeScript `import type ...` and type-only named import specifiers are erased
  for runtime import proof; they remain unavailable to exact semantic library
  contracts.
- Strict exact collection-membership gates no longer treat any strict-safe
  expression as collection evidence. Non-literal receivers must now be proven by
  `Domain` evidence from exact receiver nodes, immutable local/module binding
  anchors, scoped parameter annotations, or selected admitted API result records.

## Scattered semantic knowledge

Semantic knowledge still appears in several forms outside the facade:

- direct `Lang` checks and local recognizers in strict exact gates and value-graph
  rules that have not yet been expressed as shared contracts;
- source provenance now exists for selected JS/TS and Python equality-shaped
  surfaces, JS-like unary `typeof`, Python comprehension surfaces, and C
  unsigned-cast syntax. Consumption is still limited to narrow contracts such as
  JS-like static membership callbacks, the strict `typeof` exact gate, Python
  HOF/comprehension admission, and C byte-pack casts. General equality
  dispatch and producer-executed external pack influence remain open;
- language-specific import, symbol, or module proof mechanics that are still
  local to frontend, normalize, detect, or value-graph callers;
- C quote-include and typedef alias proof now has `Import`/`Type` evidence for
  the current byte-pack alias forms, but broader type-system evidence and
  external C pack manifests remain open;
- JS/TS record-shape and own-property guards now have dedicated `Guard` evidence
  records consumed by strict exact and value-graph paths. The recognizers are
  still first-party JS/TS lowering code, and broader guard families, richer
  source/API dependency records, and pack-facing dependency validation remain
  open;
- IL no longer stores import proof as `Seq("import_binding")` /
  `Seq("import_namespace")` payloads. Frontends keep an assignment plus
  untagged coordinate literals for structural similarity and nearby syntax, but
  import identity is proven only by `EvidenceRecord::Import` and associated
  `Symbol` evidence. Corpus-level module/export matching and snapshot stitching
  are still local to `nose-frontend`;
- module/import proof logic for immutable sibling-module literal bindings still
  has frontend-local corpus orchestration, but provider literal export safety is
  now a shared `nose-semantics` policy. Replacement copies the provider's closed
  evidence subgraph into the importer, preserves provider source-origin spans,
  rewires dependency ids, and records `ImportedLiteralSnapshot` provenance tied
  to the importer static import proof. The current positive product slice covers
  imported map-default values for Python, Java, Go, and JS/TS constructor-backed
  Maps, imported immutable collection membership for JS/TS constructor-backed
  Sets plus TypeScript/Rust literals, and imported string-affix coordinates for
  TypeScript, Java, Rust, and Go, with mutation, shadowing, wrong-default, and
  re-export hard negatives;
- broader value-domain evidence and LawPack records beyond the current
  first-party `nose.value_graph.laws` pilot for factor distribution and clamp;
- named value-graph rule modules that still consume internal `Builder` facts
  without pack-facing per-family provenance;
- oracle evaluation rules for admitted eager calls, short-circuit quantifiers,
  append mutation, nullish defaulting, reductions, and HOF callback execution
  now consume internal demand profiles, but broader lazy, async, generator,
  protocol, repeated, and call-by-need demand/effect semantics are still not a
  shared external contract language;
- remaining library/API proof gates that do not yet have occurrence records.
  `LibraryApi` occurrence evidence now covers selected JS-like static/global
  APIs and static-index membership, JS/TS/Java property builtins, Python
  builtin/import-backed factories/functions, pack-proven Python/Go/Swift
  free-function builtins, pack-proven Python iterator builtins, Rust free-name/path factories,
  Rust Option/scalar APIs, Ruby `require "set"; Set.new(...)`, Java `java.util`
  static factories/adapters and selected empty constructors, JS regex literals,
  selected receiver-method families. Broader thenable assimilation, exact async/sync protocol convergence,
  ecosystem APIs, and broader protocol/API evidence paths still rely on contract
  rows plus local proof or remain exact-closed. Raw Python async-looking field names such
  as `aread` no longer rewrite to sync names without an explicit protocol/API
  evidence path, JS/TS/Python/Rust/Swift async function and `await` expressions
  no longer erase to their body or operand without async protocol proof,
  JS/TS/Python `yield` no longer erases to its yielded expression without
  generator protocol proof, and Rust `async {}`/`?` no longer erase to their body
  or operand without async/error protocol proof. Swift `try` similarly keeps the
  error channel explicit. Go `go`/`defer`/channel receive no longer erase to ordinary
  calls or operands, Go channel send no longer relies on an untyped
  `send_statement` sequence tag, and Python list/set/dict/generator
  comprehension surfaces no longer share exact semantics merely because they
  share a lowered HOF shape. Rust `0..len(collection)` recognition now requires
  the half-open range source fact in addition to admitted `len` semantics, and
  Rust `Some(_)` pattern recognition now requires both selector API proof and
  wildcard pattern source proof rather than raw names or raw pattern shape.

These are valuable, but they do not yet share one complete semantic contract
language.

## Current strengths

- Exact matching is conservative by design.
- The value graph already separates behavioral fingerprints from fuzzy candidate
  structure.
- The oracle models return values, ordered effects, evidence-admitted final
  field state, `Err` behavior, short-circuit `and`/`or`, `any`/`all`, HOFs,
  recursion, and selected interprocedural calls.
- Proof-sensitive normalization already has named rule modules and a Lean
  obligation registry.
- Raw-node coverage gives a practical measure of lowering gaps.
- Convergence tests and hard negatives catch many semantic boundary mistakes.

## Current limits

- Language semantics are not first-class. Many rules ask "which language is this?"
  instead of "which semantic capability has been proven?"
- Library semantics are still compiled into engine/builtin facade code.
  Internal `LibraryApiContract` rows exist, and v0 manifests can describe
  contract metadata, but local external packs cannot yet execute producers or
  open exact consumers.
- Evaluation strategy is only partially shared. Internal demand profiles now
  cover the currently admitted eager, short-circuit, append, nullish-default,
  reduction, eager HOF callback, pull-lazy library iterator/stream HOF, and
  selected source protocol shapes, but call-by-need, richer async/generator,
  channel/protocol, observable-effect, exact-size/materialization, and
  callback-effect behavior are not represented by a common pack-facing
  demand/effect abstraction.
- The #594 obligation vocabulary makes scheduling, success/error/rejection
  channels, callback demand/effect, lifecycle/materialization, receiver
  mutation, and ambiguous selectors reportable across JS/TS, Python, Rust, Go,
  Java, Swift, Ruby, and C. This is a reporting and kernel-obligation layer:
  broad async, channel, callback, aggregate-result, cancellation, and lifetime
  convergence remain fail-closed until producers can prove dependency-closed
  obligations.
- External producer execution does not exist. New languages and libraries that
  affect analysis must still be added inside the main crates.
- Query JSON now exposes the active builtin/local pack set at top level, but
  family/member-level pack provenance is still limited. Selected findings can
  expose internal law provenance; local external packs remain metadata-only.
- Builtin and external responsibility boundaries are documented and represented
  in the internal facade as provenance/trust policy. Loaded external manifests
  remain metadata-only until a producer runtime and executable fixture/oracle
  workflow exist.

## Current fail-closed choices

Several older convergence expectations are intentionally disabled or narrowed in
this worktree because the required evidence is not yet modeled:

- JS-like `.then(lambda)` does not converge with `await` code yet. Supported
  `Promise.resolve(...).then(...)` chains can reduce behind a Promise boundary,
  but await scheduling, exception, and effect equivalence are not modeled as the
  same async protocol. The 2026-06-30 JS/TS audit counts `29,305` `await`
  occurrences and `14,491` async-function surfaces in this closed boundary.
- JS/TS, Python, Rust, and Swift async/sync protocol twins can converge only in
  the near/graded channel as `async-mirror` transformation leads. Exact
  `await value`/async-function payload equivalence with plain `value` remains
  closed until language/runtime-specific async protocol, demand, scheduling,
  exception, and effect obligations are modeled. Rust `?` is similarly closed
  until error protocol obligations are modeled. JS/TS and Python `yield value`
  remains closed against plain `value` until generator demand and suspension
  semantics are modeled.
- Python `asyncio.create_task`/`sleep`/`gather`/`wait` plus `run`,
  `wait_for`, `shield`, `run_coroutine_threadsafe`, and `to_thread`, Rust
  `tokio`/`async-std`
  spawn, qualified/imported `tokio`/`futures`/`futures_util`
  `join!`/`select!`, qualified/import-backed Rust `tokio_test::block_on`
  calls plus proof-backed Rust runtime `.block_on` receiver chains, and
  Swift `Task` creation and continuation bridges report scheduler, lifecycle,
  cancellation, callback, exception, and result-channel obligations but do not
  yet converge with synchronous calls, direct payload values, `await`, or each
  other. These reports require import-backed Python `asyncio` namespace or
  binding proof with no path-visible local module,
  qualified or imported Rust runtime proof whose root is not locally defined in
  the same file, proof-backed `tokio::runtime` receiver construction for
  Rust `.block_on`, proof-backed local runtime variables whose last visible
  assignment preserves that construction, nominal scope-visible Rust
  `tokio::runtime` parameter receiver evidence, unshadowed Swift `Task` roots
  with no corpus-visible Swift `Task` definition, and unshadowed Swift
  continuation free functions with no same-file or corpus-visible shim.
  Python function-local `asyncio`
  imports remain closed until scope proof exists, the imported-occurrence
  producer keeps Rust block-scoped or other-module runtime imports from proving
  calls outside that module scope, and `self.rt.block_on(...)`,
  wildcard/relative imports, and type aliases stay closed until stronger
  receiver/type evidence exists.
- Java `CompletableFuture.supplyAsync`/`runAsync`, settled factories,
  `allOf`/`anyOf`, and exact-import-backed CompletionStage-style receiver
  continuations now report future settled-value, continuation, callback
  demand/effect, task scheduling, aggregate, and exception obligations when the
  type or receiver identity is proven. Exact Future/CompletionStage recovery
  remains closed: executor timing, callback identity/effects,
  cancellation/liveness, exceptional completion, result channels, and
  constructor semantics still need dependency-closed contracts before Java
  future calls can converge with synchronous values or each other.
- Go `go f(x)`, `defer f(x)`, `<-ch`, `ch <- x`, and `select` do not converge
  with ordinary calls, values, sends, or sequential control-flow variants until
  channel/goroutine/defer/select contracts can prove scheduling, blocking,
  close/zero-value, case-selection, and effect obligations.
- Python returned generator and set comprehensions do not converge with returned
  list comprehensions. `len(generator)` and `len(set_comprehension)` stay closed
  against list cardinality/count reductions until generator demand and set
  deduplication obligations are modeled. Supported list/generator terminal
  reductions remain open only where the consumer immediately demands the stream.
- Plain JS/TS `Map(...)` and `Set(...)` calls do not enter exact matching because
  constructor-only contracts require construct-syntax proof.
- Ordinary JS/TS string `.test(...)` calls do not enter regex-test exact matching
  because the receiver must have regex-literal provenance.
- Untyped JS/TS array method chains do not enter exact higher-order contracts
  unless the receiver is a literal/proven collection surface.
- Nested element method chains such as `xs.map(...)` inside a flat-map callback
  stay closed unless the nested element collection proof is available. Explicit
  nested builder loops can still converge with identity flat-map when their loop
  structure proves the emitted elements.
- Ruby untyped `Enumerable` methods, including block loop surfaces such as
  `.each` and `.each_with_index`, plus Ruby scalar/array `abs`/`min`/`max` and
  C `fmin`/`fmax`, remain closed until the relevant receiver, stdlib, and
  overload facts are modeled as contracts.
- Rust scalar `.abs`, `.min`, `.max`, and `.clamp` are admitted only for the
  current first-party primitive-integer domain. Rust float methods need a separate
  float/NaN contract and proof before they can enter exact matching.

These reduce recall in affected cases, but they are the correct precision trade
until packs can emit the missing facts.

Remaining migration targets are tracked in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). The post-PR #147
classification snapshot is in
[semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md), and
the completed #109 foundation and follow-up tranche is in [semantic-kernel-tranche-closeout-2026-06-09](semantic-kernel-tranche-closeout-2026-06-09.md).

## See also

Back to [semantic-kernel](semantic-kernel.md). This page records the current
implementation shape; planned work and decision history live in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). The internal evidence
record substrate is described in [evidence-records](evidence-records.md). The
post-PR #147 raw/local pocket audit is recorded in
[semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md). The
v0 provider-facing extension API is defined in
[semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md), and the
local provider/user conformance workflow is in
[semantic-pack-conformance](semantic-pack-conformance.md). The closeout for the
#109 semantic-kernel migration is in the [semantic-kernel-tranche-closeout-2026-06-09](semantic-kernel-tranche-closeout-2026-06-09.md)
tranche note.
