# Demand and effect semantics

Demand/effect contracts describe how an already-admitted semantic operation
evaluates its children, invokes callbacks, and exposes effects. They do not
admit a source API by name. API admission still requires source, symbol, import,
receiver, domain, and `LibraryApi` evidence.

## Current substrate

`nose-semantics::demand` now exposes a shared `DemandEffectProfile` with these
axes:

- operation class: eager, fold reduction, short-circuit quantifier, append
  mutation, nullish default, per-element HOF, pull-lazy HOF, call-by-need thunk,
  async continuation, generator suspension, source-order callback invocation,
  scheduled/deferred callback invocation, channel operation, or protocol
  boundary;
- evaluation order: source order, short-circuit, per-element source order,
  deferred until observation, runtime scheduled, or protocol-defined;
- child demand: always, never, conditional, short-circuit-until-known,
  per-element-pull, maybe repeated, call-by-need memoized, suspended until
  observed, async continuation, channel boundary, or protocol boundary;
- callback demand, when present: per-element callback, fold step, async
  continuation, source callback invocation, scheduled callback, or deferred
  callback, with argument/result roles;
- effect visibility: immediate, only-if-demanded, delayed-until-pull,
  memoized-first-demand, async boundary, yield boundary, channel boundary, or
  protocol boundary.

This is a contract model for admitted operations, not an evidence record family.
Source protocol facts such as `Source::Protocol(Await)`,
`Source::Protocol(Yield)`, and `Source::Protocol(BlockYield)` are proof
anchors. The demand/effect profile says what a contract would need to prove
before exact consumers may use that anchor.

## Implemented profiles

Builtins have demand/effect profiles for:

- eager operations such as `len`, `sum`, `min`, `max`, `range`, `zip`, `keys`,
  and `get-or-default` after their API occurrence is admitted;
- explicit fold reduction;
- `any`/`all` short-circuit quantifiers;
- append mutation;
- nullish/default fallback, where the fallback child is conditional.

Swift `Dictionary`'s default subscript has a lazy autoclosure fallback rather
than the eager argument demand of Python `dict.get` or Java `getOrDefault`.
The controlled cross-language slice therefore admits only a direct plain
immutable fallback parameter read, whose demand timing is unobservable. Calls,
operators, property-wrapped coordinates, derived expressions, and other
potentially effectful or trapping defaults stay closed until a separate
demand/effect proof can preserve that distinction. A rejected Swift
default-subscript is made source-salted opaque before value-graph child
evaluation, so its autoclosure is not accidentally modeled as an eager call or
merged with a hoisted eager fallback.

Higher-order forms have per-element callback profiles for `map`, `flat_map`,
`filter_map`, `filter`, and `reduce`, but a raw HOF kind does not choose eager
or lazy timing. Timing comes from an explicit demand source. Python
list/dict-comprehension surfaces use eager per-element demand where modeled.
Python generator-expression surfaces use pull-lazy demand: callback errors and
effects are delayed until a terminal consumer pulls an element. First-party
library/API HOF rows now carry explicit timing for the supported surfaces:
JS-like and Swift `map`/`flatMap`/`filter` rows are eager per element where
available; Ruby Enumerable `map`/`collect`/`select`/`filter`/`reject` rows are
eager per element only when an inline effect-closed block is present. Rust
iterator and Java Stream `map`/`flatMap`/`filter` rows are pull-lazy. Python
builtin `map`/`filter` rows are also pull-lazy, but only when they are admitted
through `nose.protocols.iterator_builtins` with unshadowed builtin proof,
iterable-source proof, and a lambda callback shape. Rust
iterator HOF rows require
`nose.protocols.sequence_hof_adapters` occurrence provenance on a proven
protocol receiver; `count` terminal rows use the same receiver provenance
boundary, while Rust `any`/`all` terminals additionally require inline
effect-closed callback proof. Swift `map`/`filter` rows use the same pack only on
proven Array/Collection receivers with inline effect-closed callbacks. Swift
`flatMap` additionally has an eager one-level controlled slice: the outer source
must be a direct, attribute- and modifier-free function parameter with
language-core bracket-array (`[T]`) evidence, and its unary effect-closed callback
must return either a direct bracket-array parameter or exactly one admitted inner
`map` over one. The controlled aggregate slice may place one admitted pure
`filter` over either direct outer or inner source, retaining both predicate
coordinates before an eager unary `allSatisfy` terminal. Imports, type aliases,
macros, visible `flatMap`/`filter`/`map` methods or
callable properties, property-wrapped or parser-recovered callback parameters,
lexically unrelated parameter evidence, derived or repeated filters, scalar
results, and recursive `flatMap` output close the proof before normalization.
Visible unary `allSatisfy` overloads close terminal admission, while a proven
two-argument callback overload is disjoint from the standard unary call. Any raw
Swift `flatMap` selector that survives normalization also stays outside opaque
same-callee exact identity. Swift
`compactMap` additionally has an eager-per-element controlled slice: the callback must run
over a direct, attribute- and modifier-free function parameter with language-core bracket-array (`[T]`) source evidence, have one plain parameter used as
both the `Bool` condition and emitted value, have exactly one `nil` branch, and run in
a closed nil-literal namespace without imports, type aliases, macros, or visible
`ExpressibleByNilLiteral` conformances. Ruby
Enumerable rows use the same pack only on proven Array/Collection receivers with
inline effect-closed blocks; no-block Enumerator returns, lazy enumerators,
framework relations, Hash/Set receivers, and `flat_map` remain closed until
their demand, receiver, ordering, or flattening semantics are represented.
`collect` remains in the Rust iterator identity/materialization adapter slice.
Admitted HOF identity alone is still not enough; consumers resolve the
node-level demand/effect profile before opening exact behavior.

Library-API admission now represents transform-callback purity separately from
boolean-predicate purity through one shared callback-obligation resolver. For
already-admitted JS/TS Array, Swift Array/Collection, and Ruby Array/Collection
HOF rows, a transform callback must be a unary inline function whose body stays
inside immutable local or captured parameter projection, language-core-proven
collection/tuple construction,
proof-backed non-dispatching/non-trapping JS/TS primitive operators, and recursively
admitted HOF calls. Unary value coordinates require exactly one plain childless
parameter; default, rest, destructured, optional, and other non-plain forms stay
closed. Ruby trailing-comma destructuring and block-local declarations remain
container markers rather than yielded-value parameters; dynamic regex interpolation,
explicit return, and nested method/class definitions remain evaluation, nonlocal-control,
or runtime-definition effects. A nested HOF's eager receiver/source is still walked under
the enclosing transform obligation after recursive admission. Its callback is rechecked when
the nested obligation is weaker, so a predicate rule cannot hide coercive operator dispatch;
an identical or stronger nested obligation is reused without a duplicate subtree walk. Observed
calls, free/global/unresolved reads, captured assignment, extra index/source coordinates, implicit
`arguments`/dynamic-`this` context, operator or custom dispatch, throwing or
unsupported sinks, and unproven property/field reads close admission.
Pre-alpha name lookup stops at every intervening parameter, assignment/destructuring,
or foreach binder, and post-alpha canonical ids cross Lambdas but never a fresh Func
namespace. This prevents a same-spelling or same-number local from borrowing an outer
parameter's domain/purity proof. Ruby value-transform operators, array splat, and
map-key hashing stay closed. JS array
spread remains an unproven nested sequence, and `instanceof` remains closed
because source-level operand checks can throw. Equality-shaped JS/TS operators require a
unique admitted source-operator identity, so missing, ambiguous, or broken evidence cannot
reinterpret `instanceof` as equality. Runtime class-heritage expressions also remain visible
and closed. Swift value-transform literals and
collection construction stay closed because literal protocols and hashing are
contextual dispatch. Every Swift value-transform operator also stays closed until
stdlib nominal identity can distinguish a builtin primitive from a qualified user
type with the same final name; force unwrap, casts/type checks, consume expressions, and
interpolation remain explicit closed
surfaces. Swift closure capture lists also remain closed because initializers run at
closure formation and capture ownership affects lifetimes. JS abstract integer literals are not
Number proof because the lowering also represents BigInt with that class.
Quantifier/filter predicates use the separate pure-predicate
obligation even though both obligations share the same effect-closure walker.
This callback fact does not prove receiver/source,
emitted-value, optional-channel, flatten-depth, aggregate, or timing facts.

The Swift `compactMap` slice combines those independent facts rather than treating
the selector as proof. Its `FilterMap` value keeps the drop condition and emitted
value as separate coordinates and preserves the Optional absence channel. Changed
conditions or emitted values, a different source, `.map` returning Optional payloads,
captured or Optional emissions, custom nil-literal channels, effectful callbacks,
same-file/corpus-visible custom `compactMap`/`map`/`filter` methods or callable properties, parameter attributes/modifiers/property wrappers, nominal/custom
receivers, derived or aliased sources, and imports, type aliases, macros, overloadable or
derived expressions remain split or fail closed. Exact-channel consumers
revalidate the admitted HOF with the same interner used for source/API identity; ambiguous
corpus evidence remains a tombstone across repeated normalization, and any surviving raw
Swift `compactMap` selector stays outside opaque exact method identity.

Promise `.then` now carries an async-continuation demand/effect profile in its
contract row. That does not open exact beta-reduction by itself. The value-graph
rule requires an admitted Promise-like receiver plus a recoverable supported
settled value. Today that means JS-like `Promise.resolve(value)` with
unshadowed `Promise.resolve` proof and a non-thenable-safe value, JS-like
`Promise.reject(reason)` as a rejected channel, or a chain of admitted
`.then(lambda)`/`.catch(lambda)` calls over those supported boundaries. Safe
`.finally(lambda)` passthrough is also modeled when the receiver is admitted and
the handler is absent or a zero-argument lambda returning a non-thenable-safe
value, a fulfilled Promise boundary, or a rejected Promise boundary.
Handler-returned `Promise.resolve` is flattened only when its value is
non-thenable-safe after local callback substitution, handler-returned
`Promise.reject` stays in the rejected channel, and a rejecting `.finally`
handler overrides the original settlement with that rejected channel. Arbitrary
selector-only `.then(...)`/`.catch(...)`/`.finally(...)`, custom thenables,
shadowed Promise roots, unsafe `Promise.resolve(obj)` arguments, unsafe or
parameterized `.finally` handlers, unsupported aggregate combinators, and
missing receiver proof remain closed. Literal Promise aggregates can recover
only after the aggregate call is admitted and every element either has supported
Promise settlement evidence or proves the same non-thenable-safe raw-input
condition used by `Promise.resolve`. `Promise.all` and `Promise.allSettled`
recover all-fulfilled/all-settled channels, while `Promise.race` and
`Promise.any` recover only the first-settled/first-fulfilled subsets whose
literal inputs are fully closed. Dynamic iterables, possible thenables,
all-rejected `Promise.any` AggregateError payloads, executor timing, and sync
aggregate equivalence stay closed.

Source protocol boundaries have internal profiles for future contracts:

- `await` and Promise continuations are async boundaries;
- JS/TS, Python, Rust, and Swift runtime-body async functions are async
  boundaries even when their body has no `await`; JS/TS Promise producer proof
  remains a separate Promise-specific recovery obligation;
- `async {}` is suspended until observation;
- JS/TS and Python generator `yield` is a generator suspension boundary;
- Ruby block `yield` is a source-order callback invocation boundary;
- Go channel/select surfaces are channel boundaries;
- Go goroutine surfaces are scheduled callback protocol boundaries, while
  `defer` surfaces are scope-exit deferred callback protocol boundaries;
- Rust `?` is a conditional short-circuit boundary.

## Current consumers

The interpreter oracle consumes builtin demand/effect profiles for admitted
builtin calls instead of branching on local demand enums. This preserves current
behavior while giving the oracle a single semantic contract source.

The value graph consumes node-level HOF demand/effect profiles for source and
admitted library HOFs. A Python list comprehension or admitted eager JS-like/Ruby
HOF with a statically failing callback can trigger the surrounding handler when
the collection is known non-empty. A Python generator expression or admitted
Rust/Java pull-lazy HOF with the same callback does not, because construction is
pull-lazy and the callback is not demanded until observation. `len` over a HOF is
opened only for materialized/eager profiles; terminal reductions may consume
profile-backed pull-lazy iterator HOFs. Raw HOF payloads, selector-only calls,
unsupported source HOFs, and broken `LibraryApi` evidence remain closed.
Python `list`/`tuple`/`set` materializers may consume an admitted lazy iterator
source from `map`/`filter` or `zip`/`enumerate`, but only when the terminal
collection factory proof and the iterator producer/source proof are both
present.

The Promise `.then` value-graph rule consumes the async-continuation contract,
PromiseLike receiver proof, and supported settled-value proof. It keeps the
result behind a Promise boundary, so a Promise continuation does not converge
with synchronous code that computes the same payload.

## Exact-channel policy

Demand/effect profiles are necessary but not sufficient for exact semantics.
Exact consumers must still prove:

- the API occurrence or source protocol surface is admitted;
- receiver/protocol/domain obligations are satisfied;
- callback purity/effect obligations are satisfied where the law requires them;
- missing, ambiguous, conflicting, or dependency-broken evidence closes the
  exact path.

Selectors, raw `Payload::Builtin`, raw `Payload::HoF`, and source protocol facts
do not prove demand behavior by themselves.

## Remaining gaps

The substrate is intentionally broader than today's exact consumers. Remaining
work includes:

- broader thenable assimilation and async/await convergence contracts;
- pack-facing schema names for demand/effect rows (coordinated with issue #151);
- conformance fixtures that let pack authors prove demand/effect behavior
  without giving packs exact-clone authority (issue #157);
- richer iterator, generator, channel, call-by-need, observable, scheduling,
  exact-size/materialization, and callback-effect contracts;
- report-level provenance for which demand/effect contract influenced an exact
  result.

## See also

- [semantic-kernel](semantic-kernel.md) defines the exact-admission boundary that
  consumes demand and effect evidence.
- [scheduling-channel-callback-obligations-594](scheduling-channel-callback-obligations-594.md) defines the #594 cross-language obligation vocabulary that maps scheduling, channel, callback, lifecycle, and mutation surfaces onto this substrate.
- The maintained kernel contract is summarized in
  [semantic kernel](semantic-kernel.md); active migration and pricing work is
  routed from [development and evidence](development-and-evidence.md#planning-and-pricing).
