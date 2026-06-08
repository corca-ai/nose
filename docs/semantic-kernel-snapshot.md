# Semantic kernel snapshot

Back to [semantic-kernel](semantic-kernel.md). This page records the current
implementation shape; planned work and decision history live in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). The internal evidence
record substrate is described in [evidence-records](evidence-records.md).

Snapshot date: 2026-06-09. The current implementation has an internal
semantic-kernel facade, evidence-gated field state, sequence-surface contracts,
proof-backed append fragment evidence, operator-law contracts, typed import
facts, source-fact gates for construct/macro/literal/operator provenance,
receiver-domain evidence resolution, and a shared evidence-record substrate for
source, domain, import, symbol-identity, type-alias, guard,
place/effect, mutation-risk effect, selected library API occurrence,
value-domain/law contracts, and sequence-surface facts.
JS/TS, Python, and Rust `await` expressions are preserved as raw async protocol
boundaries with `Source::Protocol(Await)` evidence instead of being erased into
their operand. JS/TS and Python `yield` expressions are preserved as generator
protocol boundaries with `Source::Protocol(Yield)`. Rust `async {}` and `?` are
likewise preserved as protocol boundaries with `Source::Protocol(AsyncBlock)` and
`Source::Protocol(TryPropagation)`. Go goroutine spawn, deferred calls, channel
send/receive, receive-status projections, and `select` boundaries are also
preserved as raw source-backed protocol anchors rather than ordinary calls,
values, or sequence tags. Python comprehension lowering now records whether a
HOF came from a list comprehension, set comprehension, dict comprehension, or
generator expression, and exact/value consumers use that surface evidence before
applying materialization or demand-sensitive laws. Admitted builtin and HOF
operations now also have internal demand profiles for the currently supported
eager, short-circuit, append, nullish-default, reduction, and per-element
callback evaluation shapes; these profiles describe how an already-admitted
operation is consumed, not which source API is admitted.
Library/API identity is consolidated through internal `LibraryApiContract` rows
for factory, constructor, selected property/non-factory method/view surfaces,
and selected non-call sentinels, with occurrence evidence covering selected
JS-like static/global APIs and static index-membership calls, JS/TS/Java
`length` property reads, Python builtin/import-backed APIs, Rust free-name/path
APIs including `Option::Some`/`Option::None`, Ruby require-backed APIs, Java
`java.util` APIs including selected empty constructors, JS regex API calls, and
selected language-scoped receiver-method APIs such as collection membership,
map lookup/defaulting, map-key views, iterator identity adapters, Rust scalar
integer methods, Rust `Option::and_then`, Rust `zip`, and HOF/reduction methods.
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
- `nose-semantics` defines the first-party semantic profile facade: language,
  source-fact, operator, demand, effect, fragment, module, stdlib, builtin,
  method-call, property, async, iterator-adapter, builder-append, and factory
  contracts. The public crate surface remains a flat facade, while internal
  evidence/source/domain proof helpers, demand profiles, effect/place helpers,
  library API contract identities, library API row constructors, library API
  evidence-hash registry helpers, negative API guard rows, and library API
  occurrence/admission logic are split into focused modules.
- `nose-frontend` owns tree-sitter parsing, per-language lowering, embedded
  `<script>` extraction, source/domain/import/symbol/type/guard/place/effect/API/
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
  orchestration.
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

- The first-party profile exposes pack id and trust policy separately from
  channel eligibility. `ChannelEligibility` describes where a fact may be used;
  first-party/default status is pack provenance, not an analysis channel.
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
  `ValueLawContract` is still an internal law-id/requirement facade: per-use
  provenance and independent conformance status are not yet tracked as separate
  value-law evidence records.
- Source facts are now first-class internal evidence for source distinctions that
  the shared IL erases. JS/TS frontends emit construct syntax, async `await`,
  generator `yield` boundaries, regex literal, strict/loose equality,
  strict/loose inequality, and `instanceof` facts. Python emits async `await`,
  generator `yield` boundaries, list/set/dict/generator comprehension surfaces,
  value equality/inequality, and identity equality/inequality facts. Go emits
  protocol facts for `go`, `defer`, channel send/receive, receive-status
  projection, `select`, and select cases/defaults. C emits source-cast facts
  for explicit unsigned 32-bit byte-lane casts, with alias-based casts depending
  on C type-alias evidence. Rust emits macro invocation syntax for selected
  macro-backed APIs plus async/error protocol facts for `.await`, `async {}`,
  and `?`. These are stored directly as
  `EvidenceRecord::Source`; there is no source-fact side-table fallback.
  Normalize and detect consume source facts only where a semantic contract
  requires that exact source surface. Current JS/TS/Python/Rust `await` nodes,
  JS/TS/Python `yield` nodes, Rust `async`/`?` nodes, and Go concurrency/channel
  nodes remain raw exact-closed protocol anchors until such a contract exists.
  Python returned generator/set comprehensions and unsupported cardinality
  surfaces stay exact-closed; supported list/generator terminal reductions can
  still reopen only through consumer-specific demand checks.
- Free-function builtin contracts are language- and arity-constrained. Supported
  Python/Go free builtins such as `len`, `sum`, `min`, `max`, `any`, `all`, and
  Go `append` require admitted `LibraryApi(FreeFunctionBuiltin)` occurrence
  evidence whose dependencies prove the unshadowed builtin/global callee before
  exact lowering.
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
  `UnsignedCast32` with source-cast evidence, and append calls with
  `Effect(BuilderAppendCall)`. Receiver-dependent specializations also stay
  proof-chain-gated: Rust `unwrap_or` canonicalizes to map `GetOrDefault` only
  when its admitted method occurrence depends on an admitted Rust map `get`
  occurrence. Raw builtin payloads remain opaque or exact-closed.
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
  `typing`/`collections.abc` carry `ImportedBinding` symbol-evidence
  dependencies, and rebound aliases stop emitting parameter-domain evidence.
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
  facts directly for selected admitted library/API factory results, and
  normalize emits binding-anchored `Domain` evidence for single-assignment
  local/module bindings whose initializer has asserted sequence or result-domain
  evidence and whose binding has no direct binding-write, receiver-mutation, or
  opaque-argument-escape risk under first-party `Effect` evidence. Binding-domain
  lookup matches the binding `local_hash` and only applies an assignment to
  receiver uses that occur after it. Strict exact receiver gates consume this
  resolver directly instead of caching raw collection/map names or CIDs from an
  assignment scan. Domain evidence can satisfy a receiver-domain precondition,
  but it is not exact-tree proof for the binding value: an opaque initializer
  with `Domain(Collection)` still does not make the variable generally
  exact-safe. The current mutation-risk producers are conservative and
  language-scoped; they invalidate exact assumptions but do not prove exact
  library semantics.
- C byte-buffer and unsigned-cast alias proof is now evidence-backed. Local
  typedefs and direct quote includes emit `Type(CTypeAlias)` evidence for the
  currently supported exact-spelling `unsigned char` and unsigned 32-bit
  aliases; included aliases depend on `Import(CQuoteInclude)`. Alias-based
  `Domain(ByteArray)` parameter facts and `Source(Cast(CUnsigned32))` facts
  depend on those type records. The C u16/u32 byte-pack value-graph laws consume
  the first-party C byte-pack contract, byte-array domain proof, and source-cast
  proof where the u32 high lane requires it; raw `UnsignedCast32` payloads stay
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
- Promise `.then` has a JS-like library API contract, but exact beta-reduction
  is closed until a pack/frontend can prove a Promise-like receiver.
- Rust iterator identity adapters (`iter`, `into_iter`, `collect`, `to_vec`,
  `copied`, `cloned`) are language-, arity-, and receiver-proof constrained
  through `LibraryApiContract` and admitted `LibraryApi` occurrence evidence.
  Normalize's exact protocol receiver admission consumes this same contract
  instead of accepting same-named methods from other languages.
- Rust method `zip(...)` is admitted as a protocol-pair operation only through
  the Rust library method-call occurrence contract and exact protocol proof for
  both sides.
- Rust stdlib path contracts for `Some`/`Option::Some`,
  `None`/`Option::None`, `Option::and_then`, and `Vec::new` carry the exact
  selector and shadow-root requirement through `nose-semantics`. First-party
  lowering/normalization now emits admitted `LibraryApi` occurrence evidence for
  `Some(...)` calls, `Some(_)` pattern selectors, bare `None` `Var`
  occurrences, and `and_then(...)` calls only when the shadow and receiver
  obligations are satisfied. The Rust frontend preserves `if let` pattern tests
  instead of lowering them directly to null/not-null builtins, so Option
  absence/presence is admitted only through the contract-backed occurrence path.
- Collection factory, map factory, and selected constructor identity now have an
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
  collection-like factories as `Collection`, set factories/constructors as
  `Set`, map factories as `Map`, and JS-like one-argument `Array.from` as
  `Array`. Java `Arrays.asList(x)` with exactly one argument is excluded because
  array-spread versus single-element provenance is ambiguous without additional
  proof. `Map.entry`, `Array.isArray`, `Boolean`, regex `.test`, `math.prod`,
  `Arrays.stream`, map `get`, iterator adapters, promise `.then`, and generic
  method contracts do not emit result-domain evidence under the current
  vocabulary. Entry-shape, mutation, demand, and exact-safety obligations remain
  separate contract checks at the consumer.
- Selected non-factory library/API surfaces also consume `LibraryApiContract`
  rows before normalize, value-graph, or strict exact paths assign semantics.
  Current rows cover map-key views and wrappers, Java/Rust/JS-like map `get`,
  Python/Java/Ruby map defaulting through method contracts, Rust
  `get(...).is_some()`/`unwrap_or(...)`, JS-like `Array.isArray`, `Boolean(...)`,
  regex-literal `.test(...)`, Python `math.prod`, promise `.then`, Rust/Java
  iterator adapters, Java `Arrays.stream`, and the language-scoped method-call
  surfaces already admitted by `method_call_contract`. These rows carry callee
  identity and result obligations; local consumers still prove receiver domain,
  import/symbol identity, source facts, exact-safe arguments, fallback demand
  shape, and mutation safety.
- Selected API call occurrences now also have `LibraryApi` evidence records when
  they remain as raw call nodes. First-party lowering emits occurrence evidence
  for JS-like `Array.from(...)`, `Array.isArray(...)`, `Boolean(...)`,
  `new Map(...)`, `new Set(...)`, and static `indexOf`/`findIndex` membership
  calls whose receiver is a proven static non-float collection literal; Python
  builtin collection factories such as `list(...)` when the callee is proven as
  an unshadowed free name; Python
  `collections.deque(...)` when the callee is proven through
  `from collections import deque`, an alias such as
  `from collections import deque as Values`, or `import collections;
  collections.deque(...)`; Python `import math; math.prod(...)`; Rust
  `vec!(...)` when source syntax proves a macro invocation, `Vec::new()`, and selected
  `std::collections::{HashSet,BTreeSet,VecDeque,HashMap,BTreeMap}::from(...)`
  factory paths when their root-shadow policy is proven; Ruby
  `require "set"; Set.new(...)` when an earlier top-level `Import::Require("set")`
  depends on unshadowed `require` proof and unshadowed `Set` receiver proof
  exists; Java `java.util` static factories/adapters such as `List.of`,
  `Set.of`, `Arrays.asList`, `Map.of`, `Map.ofEntries`, `Map.entry`, and
  `Arrays.stream`, plus selected empty `new ArrayList<>()`/`new LinkedList<>()`
  constructors; and JS-like regex-literal `.test(...)`. These records depend on
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
  so broken API proof also closes receiver-domain proof. The `LibraryApi` record
  itself proves API identity only; source, exact-safe argument, result-shape,
  mutation, and demand/effect obligations remain separate.
- Receiver-method calls that remain as raw `Field`/`Call` nodes now emit
  `LibraryApi` occurrence evidence for the first-party method families currently
  backed by `LibraryApiContract`: map `get`, map-key views, iterator identity
  adapters, and generic language-scoped method-call contracts such as
  collection/map membership, map defaulting, count methods,
  string/collection predicates, Rust scalar integer methods, Rust
  `Option::and_then`, Rust `zip`, and HOF/reduction methods. The
  occurrence record is admitted only for the exact language/method/arity row and
  depends on receiver proof: node/binding/parameter `Domain`, `SequenceSurface`,
  imported namespace or unshadowed-global `Symbol`, or a nested admitted
  `LibraryApi` result such as a collection/map factory, map-key view, iterator
  adapter, HOF, or map `get`. First-party lowering seeds these records when the
  receiver proof already exists; normalize refreshes and upserts first-party
  records after immutable binding-domain inference and again after final
  CFG/dataflow/algebra rewrites, so bindings such as
  `VALUES = List.of(...); VALUES.contains(x)` keep the same semantic fingerprint
  as direct factory receivers without reopening selector-only fallbacks.
  Normalized HOF receivers keep their same-span admitted `MethodCall(HoF(...))`
  occurrence as protocol evidence, so downstream adapters such as Rust
  `.collect()` can consume a canonicalized `filter_map` receiver without trusting
  the `collect` selector alone.
- Java empty collection constructor contracts cover `new ArrayList<>()` and
  `new LinkedList<>()` through `LibraryApiContract` rows only for the Java
  `java.util` list types. Simple names require exact `java.util` import proof or
  earlier `java.util.*` wildcard import proof, plus no local type declaration
  with the same simple name. A `java.util.*` wildcard import is not enough when
  another package explicitly imports the same simple type; fully-qualified
  `java.util.*List` names carry the namespace proof in the selector itself.
  First-party Java lowering preserves these supported constructors as construct
  `Call` nodes and emits admitted `LibraryApi` occurrence evidence. Value-graph
  collection canonicalization and result `Domain(Collection)` evidence require
  that occurrence proof, so source/import facts alone do not reopen the exact
  path.
- Builder append contracts are separate from arbitrary method calls. A selector
  such as `push`, `append`, or `add` is not proof by itself. First-party
  frontend/normalize paths must prove the receiver or active-builder contract,
  lower the call to canonical `Builtin::Append`, and attach
  `Effect(BuilderAppendCall)` through explicit same-span language/API evidence
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
  write/readback/final field sink is admitted only for the current self-field
  substrate: Java `this.field` proven by `Place(SelfReceiver)`,
  `Place(SelfField)`, and `Effect(SelfFieldWrite)`. Raw dynamic attribute or
  property spellings, including Python `self.x`, do not prove exact field state;
  they remain ordered effects or unsupported until a pack supplies explicit
  place/effect evidence.
- Exact-fragment place/effect gates now have the first pack-facing evidence
  substrate. First-party lowering and normalize refreshes emit
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
  matching `SequenceSurface` evidence rather than raw tag spelling or local
  string allowlists. Missing surface evidence now lowers to the untagged
  sequence value in the value graph, not a spelling-derived raw hash. Untagged
  `Seq` remains an internal grouping surface and does not itself prove exact
  collection semantics; the older Python empty sequence collection case is
  handled only by the explicit collection profile path.
- Collection reductions such as Rust `Iterator::count()` and Java
  `Stream.count()` are admitted through library method contracts plus exact
  protocol receiver proof, not through a bare method-name check.
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
  `Map.keys()` is an iterator view and needs the `Array.from(...)` wrapper
  contract plus `QualifiedGlobal("Array.from")` symbol evidence before it can
  feed exact membership. That qualified-global record must depend on same-span
  source proof that the `Array` root is unshadowed.
- Map lookup surfaces that return a value/option are now explicit library API contracts for
  Java/Rust/JS-like `get(key)` plus an exact-map receiver requirement. Python
  `dict.get(key, default)`, Java `getOrDefault`, and Ruby `fetch` still use the
  `GetOrDefault` method contract. Rust `get(key).unwrap_or(default)` is modeled
  as `GetOrDefault` only through the nested `MapGet` dependency on the
  `unwrap_or` occurrence. Ruby `fetch(key) { fallback }` carries a separate
  zero-arg-lambda fallback argument contract, so block fallback demand is not
  inferred from the selector name in normalize/detect.
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
- Java and JS-like `Math.abs`/`Math.min`/`Math.max` now lower through method
  contracts with an unshadowed `Math` receiver requirement instead of frontend
  text-only builtin lowering.
- Two-argument free `min(...)`/`max(...)` normalization consumes the Python
  free-function builtin `LibraryApi` occurrence contract. Same-named functions
  from other languages, including JS `min(...)`, locally shadowed Python names,
  and manually constructed calls without admitted occurrence evidence stay
  exact-closed.
- User-defined direct calls now consume `CallTarget` evidence. The first-party
  producer admits only unique top-level in-file function targets with no
  current or enclosing lexical shadowing by parameters, assignments, loop
  patterns, or nested function definitions; recursion normalization and the
  interpreter oracle, value-graph pure helper inlining, and strict exact
  direct-function callee gates no longer treat same raw callee spelling as
  call-target proof. Method and dynamic-dispatch targets require explicit
  pack/source evidence.
- JS-like `typeof` exact-safety now consumes a language- and arity-constrained
  operator contract plus `Source::Operator(Typeof)` evidence at the call span.
  A raw `Call(Var("typeof"), arg)` shape, same-named function from another
  language, or unresolved provider is not treated as the JS operator.
- JS-like `Array.isArray(...)` exact-safety now consumes a static-global method
  contract and requires the `Array` global to be unshadowed through the
  qualified-global record's root dependency.
- JS-like record-shape guards that use `Boolean(value)` as the non-null/truthy
  clause consume a static-global function contract and require the `Boolean`
  global to be unshadowed. `value !== null` and `!!value` remain available when
  their own clauses prove the same record shape. The collapsed
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
  `SequenceSurface(GoMapEntry)` evidence, so raw tag spelling alone is not
  enough. Go `composite_literal` no longer falls back to a generic collection
  sequence tag; it is consumed only by the Go map contract or left as a distinct
  surface.
- Static JS-like `indexOf`/`findIndex` membership requires a call occurrence
  whose receiver sequence surface has membership-collection admission. Untagged
  sequence expressions, destructuring surfaces, and other positional groupings
  do not become static array membership merely because their children are
  literals.
- JS/TS object literals preserve static property keys in exact map/object
  semantics, but computed property names are exact-closed until a future
  contract can prove key evaluation, coercion, order, and side-effect behavior.
- JS/TS `new Map(...)` and `new Set(...)` now require construct-syntax source
  facts distinct from ordinary calls plus `UnshadowedGlobal` symbol proof for
  the `Map`/`Set` constructor. With exact-safe static collection or entry
  arguments they can enter exact matching, including supported immutable
  module-level Set/Map bindings. Plain `Set(...)`/`Map(...)` calls and locally
  shadowed constructor names remain exact-closed.
- Static import proof facts now have a typed `ImportFactKind`/`ImportFact`
  facade in `nose-semantics`. First-party frontends emit import binding and
  namespace facts through that contract. The lowered RHS keeps only structural
  coordinate literals; proof lives in `EvidenceRecord::Import` and binding
  `Symbol` evidence. Value-graph import identity consumes sequence `Import`
  evidence into dedicated `ImportNamespace`/`ImportBinding` value ops, so raw
  import coordinate sequences can no longer become proof-bearing value nodes by
  tag shape. Imported literal replacement also consumes evidence-only import
  facts; missing or ambiguous `Import` evidence no longer proves a cross-file
  replacement.
- Symbol identity evidence now covers static imported binding/namespace aliases
  and JS/TS static-global value occurrences such as `Math`, `console`, `Array`,
  `Map`, `Set`, and `undefined` when the frontend proves no local shadow.
  Normalize idiom admission, value-graph namespace fallbacks, and strict exact
  gates consume `nose-semantics` symbol-proof helpers; imported binding/namespace
  symbol helpers no longer fall back to raw import assignment RHS parsing.
  Selected JS/TS qualified static global paths now emit `QualifiedGlobal`
  evidence as well: `Object.hasOwn` and
  `Object.prototype.hasOwnProperty.call` gate own-property guards, while
  `Array.from` gates JS-like map-key iterator wrappers. The path evidence is not
  enough by itself: consumers require its dependency on same-span
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
  dispatch, report provenance, and external pack manifests remain open;
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
  `Symbol` evidence. Module/export dependency and provider-scope validation are
  still local to `nose-frontend`;
- module/import proof logic for immutable sibling-module literal bindings is
  still local to `nose-frontend`, although replacement now copies the provider's
  closed evidence subgraph into the importer, preserves provider source-origin
  spans, rewires dependency ids, and records `ImportedLiteralSnapshot`
  provenance tied to the importer static import proof;
- broader value-domain evidence and LawPack records beyond the first
  `ValueDomain` / `ValueLaw` contracts now used by value-graph arithmetic,
  boolean, factor, large-formula, and structural-recursion gates;
- named value-graph rule modules that still consume internal `Builder` facts
  instead of versioned `LawPack` records;
- oracle evaluation rules for admitted eager calls, short-circuit quantifiers,
  append mutation, nullish defaulting, reductions, and HOF callback execution
  now consume internal demand profiles, but broader lazy, async, generator,
  channel, repeated, and call-by-need demand/effect semantics are still not a
  shared external contract language;
- remaining library/API proof gates that do not yet have occurrence records.
  `LibraryApi` occurrence evidence now covers selected JS-like static/global
  APIs and static-index membership, JS/TS/Java property builtins, Python
  builtin/import-backed factories/functions, Rust free-name/path factories,
  Rust Option/scalar APIs, Ruby `require "set"; Set.new(...)`, Java `java.util`
  static factories/adapters and selected empty constructors, JS regex literals,
  generic Python/Go free-function builtins, and selected receiver-method
  families. Promise receiver proof, async/sync protocol convergence, ecosystem
  APIs, and broader protocol/API evidence paths still rely on contract rows plus
  local proof or remain exact-closed. Raw Python async-looking field names such
  as `aread` no longer rewrite to sync names without an explicit protocol/API
  evidence path, JS/TS/Python/Rust `await` expressions no longer erase to their
  operand without async protocol proof, JS/TS/Python `yield` no longer erases to
  its yielded expression without generator protocol proof, and Rust
  `async {}`/`?` no longer erase to their body or operand without async/error
  protocol proof. Go `go`/`defer`/channel receive no longer erase to ordinary
  calls or operands, Go channel send no longer relies on an untyped
  `send_statement` sequence tag, and Python list/set/dict/generator
  comprehension surfaces no longer share exact semantics merely because they
  lower to HOF-shaped IL.

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
- Library semantics are still compiled into engine/first-party facade code.
  Internal `LibraryApiContract` rows exist, but they are not yet versioned
  external pack manifest contracts.
- Evaluation strategy is only partially shared. Internal demand profiles now
  cover the currently admitted eager, short-circuit, append, nullish-default,
  reduction, and HOF callback shapes, but pull-lazy, call-by-need, async,
  generator, channel, and richer observable-effect behavior are not represented
  by a common pack-facing demand/effect abstraction.
- External extension points do not exist. New languages and libraries must be
  added inside the main crates.
- Report output does not yet expose semantic provenance such as pack id, contract
  id, law id, or proof status.
- First-party and external responsibility boundaries are documented and
  represented in the internal facade as provenance/trust policy, but there are no
  loadable external packs or report-level pack provenance fields yet.

## Current fail-closed choices

Several older convergence expectations are intentionally disabled or narrowed in
this worktree because the required evidence is not yet modeled:

- JS-like `.then(lambda)` does not converge with `await` code until Promise-like
  receiver proof exists.
- JS/TS, Python, and Rust `await value` does not converge with plain `value`
  until language/runtime-specific async protocol, demand, scheduling, exception,
  and effect obligations are modeled. Rust `async {}` and `?` are similarly
  closed until future/error protocol obligations are modeled. JS/TS and Python
  `yield value` remains closed against plain `value` until generator demand and
  suspension semantics are modeled.
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
[semantic-kernel-roadmap](semantic-kernel-roadmap.md).
