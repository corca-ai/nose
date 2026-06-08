# Semantic evidence records

Back to [semantic-kernel](semantic-kernel.md). Current implementation status is
recorded in [semantic-kernel-snapshot](semantic-kernel-snapshot.md); history and
remaining work are tracked in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). Source-origin facts are
covered in [source-facts](source-facts.md).

Evidence records are the internal substrate that lets current first-party
frontends, and future language/library packs, emit proof facts without giving
those producers authority to approve exact clones. They are facts, not verdicts.
Contracts in `nose-semantics` decide whether a fact can satisfy an exact-channel
precondition.

## Goal

- Give source, domain, import, symbol-identity, type-alias, guard,
  place/effect, selected library API occurrence, and sequence-surface proof
  facts one shared shape.
- Make facts carry stable ids, anchors, provenance, dependencies, and status.
- Keep exact matching fail-closed when evidence is missing, ambiguous, or
  conflicting.
- Preserve existing behavior while first-party frontends emit source,
  parameter-domain, import, symbol, guard, place/effect, library API, and
  sequence-surface facts directly into the record shape.
- Make the future external pack schema a narrowing of an implemented internal
  boundary, not a speculative document-only API.

## Non-goals

- Do not let evidence records mint value fingerprints, bypass laws, or mark clone
  pairs exact.
- Do not expose this internal Rust record as the final external pack manifest or
  scan JSON schema.
- Do not certify external pack claims. nose validates record shape and fails
  closed; providers own their claims, and users own opt-in decisions.
- Do not model demand evidence or every place/effect family in this slice. The
  current place/effect records cover the first exact-fragment substrate plus
  conservative binding-write, receiver-mutation, and opaque-argument-escape
  risk facts needed to keep binding/import safety fail-closed.

## Record Shape

`nose-il` now carries `Il::evidence: Vec<EvidenceRecord>`.

An evidence record has:

- `id`: stable within the IL file, used by dependencies;
- `anchor`: the source subject the fact is about, such as a source span,
  parameter span, binding span plus local-name hash, or sequence span;
- `kind`: the kernel-defined fact kind;
- `provenance`: emitter class, pack hash, and rule hash;
- `dependencies`: other evidence ids this fact depends on;
- `status`: currently `Asserted` or `Ambiguous`.

The current implemented kinds are:

| kind | purpose |
|---|---|
| `Source` | construct syntax, Rust macro invocation syntax, async/generator/error and Go concurrency/channel protocol boundary syntax, Python comprehension surface provenance, C unsigned-cast syntax, regex literal provenance, and source operator family including JS-like unary `typeof` |
| `Domain` | parameter, receiver-expression, or value/binding domain such as collection, map, option, string, integer, or byte array |
| `Import` | static import binding/namespace proof, Python wildcard-import ambiguity proof, Java wildcard import proof, C quote-include proof, Ruby `require` module proof, and imported-literal snapshot provenance |
| `Symbol` | resolved or proven symbol identity, with record kinds for unshadowed globals, static imported binding/namespace aliases, and selected qualified global API paths |
| `Type` | type alias proof, currently exact-spelling C aliases to unsigned 8-bit and supported unsigned 32-bit integer forms |
| `Guard` | multi-obligation guard proof facts such as JS/TS record-shape and own-property guard contracts |
| `Place` | fixed receiver/place facts currently covering `SelfReceiver` and `SelfField` |
| `Effect` | observable effect and mutation-risk facts currently covering canonical builder append calls, non-overloadable index writes, fixed self-field writes, binding writes, receiver-mutating calls, and opaque argument escapes |
| `LibraryApi` | proof that a specific API occurrence matches a language/API contract coordinate, currently for selected call, property, and sentinel occurrences across JS-like static/global/static-index APIs, selected Python/Rust/Ruby/Java/regex APIs, generic Python/Go free-function builtins, and selected receiver-method families |
| `CallTarget` | proof that a specific user call occurrence resolves to a direct in-file callable unit target |
| `SequenceSurface` | lowered aggregate surface such as collection, tuple, map, pair, import proof, guard surfaces, Go composite map literals, or Go map entries |

`LibraryApi` evidence is an occurrence fact, not the whole contract. It records
the contract id, callee coordinate, arity, and dependencies for a specific
`Call`, `Field`, or `Var` node, depending on the contract surface.
`LibraryApiContract` rows in `nose-semantics` still name result semantics and
the remaining obligations. Existing evidence kinds such as `Symbol`,
`Import`, `Source`, `Domain`, and `SequenceSurface` prove those obligations; the
contract decides whether the facts are enough to admit an exact or value-graph
path. A future external pack schema may expose library API contracts, but
providers still emit facts and contracts rather than exact-clone verdicts.

`CallTarget` evidence is the occurrence proof for user-defined calls. A raw
callee spelling such as `f(...)` is only a selector that a producer may inspect;
recursion normalization, the interpreter oracle, value-graph pure helper
inlining, and strict exact direct-function callee gates consume only
`CallTarget::DirectFunction` records anchored to the exact `Call` node and
matching the target function span. The first-party producer currently emits this
fact only for unique top-level in-file `Function` units when neither the current
lexical scope nor any enclosing lexical scope has a parameter, assignment
target, loop-pattern binding, or nested function definition that shadows the
callee name. Duplicate function names, lexical shadowing, nested/non-top-level
functions, methods without an explicit target proof, computed callees, and
missing or conflicting evidence stay closed.

Current first-party `LibraryApi` callee coordinates are intentionally specific:

- `QualifiedGlobal`-backed coordinates, such as `StaticGlobalMethod` and
  `StaticGlobalFunction`, name an allowed language/API path and depend on
  matching symbol evidence for the anchored call occurrence.
- `ImportedBinding` and `ImportedNamespaceFunction` name a module/export or
  namespace/function identity and depend on import-backed call-site symbol
  evidence. They do not infer semantics from the local alias spelling.
- `FreeName` names a language-scoped free identifier, such as Python `list`,
  Python `len`, Go `append`, or Rust `Vec`, and depends on
  `Symbol(UnshadowedGlobal)` evidence at the callee anchor plus the contract's
  shadow policy.
- `JavaUtilStaticMember` names selected Java `java.util` static factory/adaptor
  calls and depends on matching Java import-binding evidence plus source-origin
  local type shadow checks.
- Python wildcard imports emit `Import::Wildcard` evidence. For current
  first-party Python free-name API producers, any asserted wildcard import keeps
  unqualified builtin/free-name library occurrence evidence closed because a
  provider module may supply a same-named binding.
- `JavaUtilConstructor` names selected Java `java.util` constructors and depends
  on construct-syntax evidence plus exact import-binding evidence or earlier
  Java wildcard import evidence. Wildcard proof is constrained by Java name
  resolution: a local same-name type or explicit same-name import from another
  package keeps the occurrence closed.
- `RustMacro` names a Rust macro invocation, currently `vec!`, and depends on
  both `Source::Call(MacroInvocation)` at the call span and
  `Symbol(UnshadowedGlobal)` evidence at the macro callee anchor.
- `RubyRequireStaticMember` names a Ruby receiver/method pair, currently
  `Set.new`, and depends on both an unshadowed receiver symbol and a matching
  earlier `Import::Require` module fact whose own dependency proves the
  `require` callee identity.
- `JsGlobalConstructor` and `RegexLiteralMethod` name APIs whose identity
  includes source provenance, such as construct syntax or regex-literal receiver
  proof.
- `StaticIndexMembershipMethod` names JS-like `indexOf`/`findIndex` membership
  calls and depends on `SequenceSurface(Collection)` evidence for the exact
  receiver plus the static non-float literal receiver shape required by the
  contract.
- `Property` names a language-scoped property surface such as JS/TS/Java
  `length`. It is anchored to the `Field` node and depends on receiver proof
  such as `Domain`, `SequenceSurface`, or nested admitted `LibraryApi`
  evidence. It does not infer semantics from a property spelling alone.
- `Method` and `IteratorAdapterMethod` name language-scoped receiver methods by
  exact method string and arity. They depend on receiver proof such as
  `Domain`, `SequenceSurface`, imported-namespace or unshadowed-global `Symbol`,
  or nested admitted `LibraryApi` evidence for factory/result calls. They do not
  infer semantics from a selector spelling alone.

## Consumption Rules

Consumers should go through `nose-semantics` helpers rather than scanning raw IL
side tables.

- A lookup succeeds only when asserted evidence resolves to exactly one relevant
  fact.
- Conflicting asserted evidence makes the lookup fail.
- Dependency-broken evidence makes the lookup fail.
- `Ambiguous` evidence makes the lookup fail.
- If any relevant ambiguous, conflicting, or dependency-broken evidence exists,
  compatibility fallback must not reopen the exact path.
- Compatibility fallback is allowed only when no relevant evidence record exists,
  and only for explicitly legacy compatibility helpers.

This is stricter than a name or tag check. For example, static import lowering
keeps an assignment RHS with only untagged module/export coordinate literals;
the coordinates are not a proof channel. Import contracts consume only
`Import` evidence. If that evidence is missing or ambiguous, exact import proof
stays closed instead of falling back to the raw sequence shape. Value-graph
import identity likewise consumes only sequence `Import` evidence and
materializes dedicated internal import values, never raw `ValOp::Seq` proof
objects.

Symbol identity follows the same rule. A method selector such as `abs` or a
receiver spelling such as `Math` is not proof. Exact consumers must require a
language-scoped contract plus symbol evidence. Imported binding/namespace symbol
helpers no longer accept a raw import assignment RHS as proof. Binding-level
import evidence does not by itself prove every use of the same local name; if the
alias is rebound or ambiguous, the exact path stays closed until a node-level
symbol fact or stronger scope-resolution evidence exists.

Domain evidence follows the same fail-closed rule. First-party parameter
annotations emit `Domain` evidence on `Param` anchors only through
language-scoped type-domain contracts in `nose-semantics`, not through a shared
substring fallback over arbitrary parameter text. The frontend owns evidence
emission and alias lifecycle: for example, Python `typing`/`collections.abc`
aliases record dependency-backed parameter `Domain` evidence that depends on the
corresponding `ImportedBinding` symbol evidence, and a rebound alias closes that
path. `nose-semantics` resolves `Domain` evidence on exact receiver-expression
node anchors, then binding anchors for immutable local/module variables, then
parameter anchors. A conflicting, ambiguous, or dependency-broken
receiver-domain record closes that receiver proof and must not fall back to
side-table mirrors or selector spelling. Binding-anchor lookup matches both
source span and `local_hash`, and a binding proof is applied to a receiver only
when the assignment is visible before that receiver use. When a receiver is an
alpha-renamed parameter or local binding reference, lookup is constrained to the
nearest function/lambda scope where appropriate so same-numbered parameter/local
ids from other units do not prove the current receiver. Method receiver
contracts expose their domain-backed obligations through `DomainRequirement`;
obligations such as imported namespace, unshadowed global, exact map literal, and
future demand/effect constraints remain separate checks.

Parameter `Domain` evidence also seeds the semantic-kernel `ValueDomain`
contract used by value-graph and recursion laws. That bridge is intentionally
narrow: integer/number domains seed `Number`, string seeds `String`, and
array/collection/set domains seed `Sequence`. The value graph may additionally
infer a domain from strict operator use, literal result domains, modeled builtin
result domains, and subexpression result domains, but the law itself is admitted
only through a `ValueLaw` contract in `nose-semantics`. This keeps string and
sequence concatenation ordered when evidence proves those domains, while numeric
and boolean laws still require positive domain proof. Today that contract records
the law id and domain requirement; pack-facing per-use value-law provenance and
conformance status remain future work rather than an emitted evidence family.

Selected `LibraryApi` result-domain evidence follows the same model. A
first-party factory call result may carry `Domain(Collection)`, `Domain(Set)`,
`Domain(Map)`, or `Domain(Array)` only after the call occurrence has admitted
`LibraryApi` evidence. The `Domain` record depends on that `LibraryApi` record,
so broken import, source, shadowing, or symbol proof closes the receiver-domain
claim as well. The result-domain record proves only the container/protocol shape
of the call result; exact consumers still prove argument safety, entry shape,
mutation, receiver requirements, and demand/effect obligations separately.

Sequence-surface evidence is also authoritative for exact/value-graph aggregate
semantics. A lowered `Seq("array")`, `Seq("object")`, `Seq("tuple")`, or
language-specific tag does not by itself prove exact-tree safety, collection
membership, map-entry-list shape, imported-literal eligibility, or a canonical
value-graph tag. Consumers resolve the tag only when a matching
`SequenceSurface` record exists at the same sequence anchor and its dependencies
remain asserted. Missing, conflicting, ambiguous, or wrong-kind surface evidence
keeps the exact/value-graph path closed. In the value graph, a missing surface
record produces the untagged sequence value rather than a raw spelling-derived
hash, so a payload name cannot become a semantic proof channel by coincidence.

Qualified global identity is also evidence, not a selector guess. The current
first-party JS/TS producer emits `QualifiedGlobal` only for selected static paths
whose root is proven unshadowed, such as `Object.hasOwn`,
`Object.prototype.hasOwnProperty.call`, `Array.from`, and `Array.isArray`. The
`QualifiedGlobal` record itself depends on same-span source evidence for the
unshadowed root (`Object` or `Array` today), and exact/value consumers reject the
path when that dependency is missing, ambiguous, or broken.

Guard identity is separate from sequence shape. A raw `Seq("record_guard")` or a
matching `SequenceSurface(RecordGuard)` fact proves only that the frontend
lowered a guard-shaped surface. Exact consumers additionally require a
dedicated `Guard::JsRecordShape` record whose subject, null/truthiness form,
comparison form, and asserted `Array.isArray`/optional `Boolean` dependencies
match the lowered sequence. Generic `SequenceSurface(RecordGuard)` is therefore
not `exact_tree_safe`; missing, ambiguous, conflicting, wrong-kind, wrong-anchor,
or dependency-broken guard evidence keeps the exact path closed.

JS/TS own-property guards follow the same rule. `Seq("own_property_guard")` and
`SequenceSurface(OwnPropertyGuard)` are only the lowered shape; exact and
value-graph consumers require `Guard::JsOwnProperty` with an asserted dependency
on one supported qualified-global API path, currently `Object.hasOwn` or
`Object.prototype.hasOwnProperty.call`. Object method spellings such as
`value.hasOwnProperty(...)`, shadowed `Object` roots, missing dependencies, or
ambiguous guard evidence remain closed.

Place and effect evidence are authoritative for the exact-fragment substrate
and for conservative mutation invalidation. Raw method selectors such as `push`,
`append`, or `add` do not prove an append effect; exact consumers need
`Effect(BuilderAppendCall)`, even when the call has already been lowered to
canonical `Builtin::Append`. Likewise, non-overloadable index writes and fixed
self-field writes are admitted only through exact `Effect` records, with
`Place(SelfReceiver)` and `Place(SelfField)` proving the receiver/place side.
First-party `Place(SelfField)` depends on the matching `Place(SelfReceiver)`,
and `Effect(SelfFieldWrite)` depends on the matching `Place(SelfField)`.
Missing, conflicting, ambiguous, or dependency-broken place/effect evidence
closes exact fragments instead of reopening a legacy language/shape fallback.

Mutation-risk effects are intentionally separate from exact effect proofs.
`Effect(BindingWrite)` says an assignment node writes its syntactic target;
`Effect(ReceiverMutation)` says a call is covered by a first-party
language-scoped receiver-mutating method-effect contract row; and
`Effect(OpaqueArgumentEscape)` says an ordinary call argument may escape to
unknown code. These records can invalidate immutable binding, module export,
imported literal, value-graph, or exact-fragment context assumptions, but they
do not prove collection membership, builder append, map update, or any other
exact semantic law. Known admitted `LibraryApi` occurrences suppress the opaque
escape helper for that call, so a modeled API such as an imported stdlib
predicate is not treated as arbitrary mutation risk merely because it takes an
argument.

Builder and map-builder value laws consume separate contract rows. A
method-effect row for list builders names the language, selector, arity,
required `Effect(BuilderAppendCall)`, and active-builder receiver obligation.
An index-write row for map builders names the language, required write evidence,
and active-map-builder receiver obligation. `BindingWrite` alone remains a
mutation-risk fact; it admits a map-builder contribution only when a matching
index-write contract and explicit active map seed are also present.

Library API evidence follows the same fail-closed rule. If a call carries
`LibraryApi` evidence for a selected API occurrence, consumers must validate the
contract id, callee coordinate, arity, dependencies, dependency anchors, and call
shape. A conflicting, ambiguous, wrong-callee-anchor, wrong-dependency-anchor, or
dependency-broken API record on the queried call closes that API path. A record
anchored to another call is irrelevant to this lookup and leaves compatibility
policy to the queried surface. For selected surfaces that already have
first-party occurrence producers, missing `LibraryApi` evidence is also closed:
older symbol/source/import facts remain dependencies of the occurrence proof,
not alternate API-identity proofs. Compatibility fallback is reserved for
contract rows whose occurrence producer is not modeled yet. The record proves
only API identity; exact consumers still separately prove receiver/domain facts,
source-surface requirements, argument safety, result shape, mutation safety, and
demand/effect obligations.

Canonical `Payload::Builtin` calls are not proof by themselves. They are the
shared operation shape after syntax/lowering/desugaring has erased a concrete
callee, so value-graph, strict-exact, function-binding, mutation-risk,
value-domain inference, and interpreter-oracle consumers must first ask
`nose-semantics` whether that call is admitted for the specific builtin.
Admission currently comes from either a same-span admitted
`LibraryApi` occurrence record whose contract id maps to that builtin, or a
narrow first-party language-core lowering: Go map lookup-ok `Contains`, Go
`range` `Enumerate`, Python dict-comprehension `DictEntry`, JS-like `for-in`
`Keys`, C `UnsignedCast32` with `Source(Cast(CUnsigned32))`, or canonical
`Append` with `Effect(BuilderAppendCall)`. Raw or unadmitted builtin payloads
stay opaque in the value graph and closed in exact/oracle consumers.
When a receiver obligation makes an API result more specific than the selector
alone, the dependency chain must prove that specialization. For example, Rust
`unwrap_or` is an option defaulting API in isolation, but the canonical
`GetOrDefault(map, key, default)` builtin is admitted only when the same
`unwrap_or` occurrence has the Rust `RustMapGetOrExactOption` receiver contract
and depends on an admitted `MapGet` occurrence for the exact receiver.

Imported API occurrence evidence is not a broad name guess. A call-site
`Symbol(ImportedBinding)` or `Symbol(ImportedNamespace)` dependency must itself
depend on the matching binding-anchor symbol, pass rebinding and local/parameter
shadow checks, and match the current receiver/callee span when that span is
available. If normalization erases an import occurrence into a seeded import
value, consumers pass no occurrence span and rely on that validated dependency
instead of accepting an unrelated imported symbol elsewhere in the file.

## Current Producers

First-party frontends now emit these facts as `EvidenceRecord`:

- parameter semantic annotations become `Domain` evidence. Selected first-party
  library/API factory calls now also emit receiver-expression `Domain` evidence
  at the exact call node after their `LibraryApi` occurrence has been admitted.
  Normalize also emits binding-anchored `Domain` evidence for single-assignment
  local/module bindings whose initializer has asserted sequence or result-domain
  evidence and whose binding has no direct binding-write, receiver-mutation, or
  opaque-argument-escape risk under the current first-party evidence producers.
  Future packs and inference producers should use node or binding anchors for
  receiver-domain proof instead of selector spelling;
- source-origin facts become `Source` evidence. JS/TS, Python, and Rust `await`
  expressions preserve a raw async boundary and emit
  `Source::Protocol(Await)` at that source span. JS/TS and Python `yield`
  expressions emit `Source::Protocol(Yield)`. Rust `async {}` and `?` emit
  `Source::Protocol(AsyncBlock)` and `Source::Protocol(TryPropagation)`. These
  are future protocol/demand proof anchors, not evidence that the source
  operation is equivalent to its operand or body. Go `go`, `defer`, channel
  send/receive, receive-status projection, `select`, and select cases/defaults
  likewise preserve source-backed protocol anchors instead of ordinary calls,
  values, or generic sequences. Python list/set/dict comprehensions and generator
  expressions emit source-comprehension facts so exact consumers can distinguish
  eager materialized lists, lazy generators, set deduplication, and dict
  materialization even when the lowered HOF body shape is similar;
- import binding and namespace lowering emits `Import` evidence for the proof RHS
  and `Symbol` evidence for the local alias identity;
- selected top-level Ruby literal `require "module"` calls that occur before a
  selected library API use emit `Import::Require` evidence for the required
  module, with an asserted `UnshadowedGlobal("require")` dependency;
- JS/TS static-global value occurrences that remain as `Var` nodes, such as
  member receivers, call callees, constructors, and `undefined`, emit
  `UnshadowedGlobal` symbol evidence when the frontend proves no local shadow;
- selected JS/TS qualified static global paths emit `QualifiedGlobal` symbol
  evidence at the lowered node anchor: own-property guards at their
  `Seq("own_property_guard")` node, and static member expressions such as
  `Array.from` and `Array.isArray` at their `Field` node. These records depend
  on a same-span source `UnshadowedGlobal` proof for the qualified root;
- JS/TS own-property guard lowering emits `Guard::JsOwnProperty` evidence for
  the lowered `Seq("own_property_guard")`, with an asserted `QualifiedGlobal`
  dependency for the admitted API path and that API record's root proof;
- JS/TS record-shape guard lowering emits `Guard::JsRecordShape` evidence for
  the lowered `Seq("record_guard")`, including the shared subject hash, the
  null/truthiness clause kind, whether JS loose equality was admitted, and
  asserted dependencies for the required `Array.isArray` API proof plus optional
  `Boolean` proof;
- first-party lowering and normalize refreshes emit `Place(SelfReceiver)` for
  Java `this`, `Place(SelfField)` for Java `this.field`,
  `Effect(SelfFieldWrite)` for Java `this.field = ...`,
  `Effect(NonOverloadableIndexWrite)` for C/Go/Java index writes, and
  `Effect(BuilderAppendCall)` for canonical `Builtin::Append`. They also emit
  mutation-risk effects: `Effect(BindingWrite)` for assignment nodes,
  `Effect(ReceiverMutation)` for calls admitted by the first-party
  language-scoped mutating-method policy, and
  `Effect(OpaqueArgumentEscape)` for ordinary calls with arguments. The shared
  semantic helper treats opaque escape as active mutation risk only when the
  same call does not also have admitted `LibraryApi` evidence. Self-field
  place/write evidence records include dependencies that link the write proof
  back to the receiver proof;
- C lowering emits `Import(CQuoteInclude)` for admitted direct quote includes
  that provide byte-pack aliases, and `Type(CTypeAlias)` for local or included
  exact-spelling `unsigned char` and supported unsigned 32-bit aliases used by
  current byte-pack surfaces. Included alias `Type` evidence depends on the
  quote-include proof.
  Alias-based `Domain(ByteArray)` parameter evidence and
  `Source(Cast(CUnsigned32))` cast evidence depend on the relevant `Type`
  evidence; direct `unsigned char *` parameters and direct unsigned 32-bit casts
  remain dependency-free;
- first-party lowering emits `LibraryApi` evidence for selected API occurrences
  that remain as raw nodes: JS-like `Array.from(...)`, `Array.isArray(...)`,
  `Boolean(...)`, `new Map(...)`, `new Set(...)`, and static
  `indexOf`/`findIndex` membership calls whose receiver has collection
  sequence-surface proof; Python builtin collection factories such as
  `list(...)` when the callee has an unshadowed free-name proof; Python
  `collections.deque(...)` through imported binding/namespace proof; Python
  `math.prod(...)` through imported namespace proof; Rust
  `vec!(...)` when macro-invocation source syntax and macro-name shadow policy
  are proven, `Vec::new()`, `Some(...)`, `Some(_)` pattern selectors, bare
  `None`, and selected `std::collections::*::from(...)` factory paths when
  their root-shadow policy is proven; JS/TS/Java `length` property reads whose
  receiver proof is satisfied; Ruby
  earlier top-level `require "set"; Set.new(...)` through `Import::Require`
  plus unshadowed `require` and `Set` proof; Java `java.util` static
  factories/adapters including `List.of`, `Set.of`, `Arrays.asList`, `Map.of`,
  `Map.ofEntries`, `Map.entry`, and `Arrays.stream`, plus selected empty
  `new ArrayList<>()`/`new LinkedList<>()` constructors through exact or
  wildcard import proof; and JS-like regex-literal `.test(...)`. These records
  depend on the relevant `QualifiedGlobal`, `UnshadowedGlobal`,
  import-backed call-site `Symbol`, `Import::Require`, construct-syntax
  `Source`, `SequenceSurface`, or regex-literal `Source` evidence. Calls
  collapsed into specialized guard surfaces emit their guard evidence instead;
  strict exact operators such as JS/TS `typeof` consume source-operator evidence
  at their exact-fragment gate instead of emitting `LibraryApi` evidence.
  Shadowed roots, unsupported arities, unsupported static paths, unresolved
  free-name/path factories, and Ruby require-backed APIs without require
  evidence do not emit API occurrence evidence;
- first-party lowering plus post-binding and final-normalization refresh passes
  emit `LibraryApi`
  occurrence evidence for selected receiver methods that remain as raw call
  nodes: map `get`, map-key views, iterator identity adapters, and the
  language-scoped method-call contracts currently used for collection/map
  membership, map defaulting, count, predicates, Rust scalar integer methods,
  Rust `Option::and_then`, Rust `zip`, HOF, and reduction methods. Property
  cardinality such as JS/TS `length` is modeled as `Property`, not as a method
  call. The post-binding refresh exists because immutable
  binding-domain evidence is inferred after lowering; the final refresh exists
  because CFG/dataflow/algebra rewrites can replace receiver expressions with
  equivalent sequence or result values. Refreshing upserts first-party occurrence
  records so `VALUES = List.of(...); VALUES.contains(x)` depends on the current
  binding or sequence-domain proof rather than falling back to selector
  matching;
- selected `LibraryApi` producer-covered result calls emit dependent
  receiver-expression `Domain` evidence: Python `list`/`tuple` and
  `collections.deque`, Rust `Vec::new`, `vec!`, and
  `std::collections::VecDeque::from`, Java `List.of` and zero- or multi-argument
  `Arrays.asList`, and selected empty `new ArrayList<>()`/`new LinkedList<>()`
  as `Collection`; Python `set`/`frozenset`, Rust
  `std::collections::{HashSet,BTreeSet}::from`, Java `Set.of`, Ruby `Set.new`,
  and JS-like `new Set` as `Set`; Rust
  `std::collections::{HashMap,BTreeMap}::from`, Java `Map.of`/`Map.ofEntries`,
  and JS-like `new Map` as `Map`; and JS-like one-argument `Array.from` as
  `Array`. `Map.entry`, `Array.isArray`, `Boolean`, regex `.test`,
  `math.prod`, `Arrays.stream`, map `get`, iterator adapters, promise `.then`,
  and generic method contracts do not emit `Domain` records because their
  results are not simple container receiver domains under the current
  vocabulary;
- lowered `Seq` surfaces emit `SequenceSurface` evidence, including Go map
  literal and Go map-entry surfaces where those tags carry first-party meaning.

Source-origin and parameter-domain proof no longer has side-table mirror storage:
frontends emit `Source` and `Domain` records directly, and semantic lookups are
evidence-only for those facts. First-party JS/TS record-shape guards now have
dedicated guard evidence, exact-fragment append/index/self-field gates now have
the first place/effect evidence substrate, and binding/module/import mutation
safety consumes mutation-risk `Effect` records instead of re-scanning raw call
selectors and assignment shapes. Broader guard families, richer source-clause
dependencies, richer receiver/place families, demand-aware effect contracts,
and general evidence validation remain open.

## Current Consumers

The first migrated consumers are the shared semantic helpers and their direct
callers:

- source-fact lookup for construct syntax, async/generator/error and Go
  concurrency/channel protocol boundaries, Python comprehension surfaces, C
  unsigned-cast syntax, regex literal, and operator provenance;
- receiver-domain lookup used by desugaring/idiom canonicalization,
  post-desugar semantic/value-graph membership/property/map/integer gates, and
  strict exact receiver gates. Consumers ask `nose-semantics` whether a receiver
  satisfies a `DomainRequirement`, using the shared
  `ReceiverDomainEvidenceIndex` cache when they inspect many receivers. That
  keeps node-anchored receiver evidence, immutable local/module binding
  evidence, scoped parameter evidence, selected API result-domain evidence,
  ambiguity handling, and compatibility fallback under one kernel-owned policy
  instead of reimplementing those rules per consumer. Desugaring and early idiom
  canonicalization still run before normalize emits additional immutable
  binding-domain evidence, so they only consume the domain evidence already
  present at that point. Strict exact receiver gates use this resolver directly
  and do not maintain raw collection/map name or CID side tables. Coarse API
  result-domain and binding-domain evidence is not exact-tree proof: value-graph
  consumers prefer concrete factory/result-shape canonicalization before using a
  domain-shaped fallback, and strict exact consumers may use domain evidence only
  for receiver-domain obligations. They do not promote an opaque binding with a
  collection/map domain into a generally exact-safe variable value;
- import proof parsing for compatibility helpers, with value-graph import
  identity and imported literal replacement consuming evidence-only facts;
- cross-file imported literal replacement copies the provider's closed evidence
  subgraph into the importer while preserving provider source-origin spans and
  rewiring dependency ids, then records `Import(ImportedLiteralSnapshot)`
  provenance that depends on the importer import proof and copied provider
  evidence;
- imported namespace/binding symbol proof for normalize idiom admission,
  value-graph namespace fallbacks, and strict exact gates, without raw assignment
  fallback;
- value-graph internal import identity now uses dedicated
  `ImportNamespace`/`ImportBinding` value ops derived from `Import` evidence, so
  raw import `Seq` payloads cannot hash-cons with proof-bearing import values;
- unshadowed-global symbol proof for JS/TS `Math.*` method contracts,
  `new Map(...)`/`new Set(...)` constructor contracts, static `Array.isArray`
  exact gates, and `undefined` nullish-default handling. Value-graph nullish
  value semantics are evidence-only for `undefined`; raw spelling plus a
  scope scan no longer reopens that exact path;
- qualified-global symbol proof for selected JS/TS API paths: own-property
  guard evidence depends on `Object.hasOwn` or
  `Object.prototype.hasOwnProperty.call`, and map-key view wrappers require
  evidence for `Array.from`. Those `QualifiedGlobal` records themselves depend
  on the appropriate unshadowed root proof, so path-shaped text or a detached API
  record does not prove identity;
- selected `LibraryApiContract` consumers now consult `LibraryApi` occurrence
  evidence first for the migrated JS-like, Python builtin/imported, Rust
  free-name/path/Option/scalar, Ruby require-backed, Java static/property,
  regex-literal, property, and receiver-method surfaces;
  conflicting or dependency-broken API evidence keeps
  the value-graph, idiom, and strict exact paths closed. Missing API evidence is
  now also closed for those producer-covered surfaces; older symbol/source proof
  helpers remain dependency inputs to `LibraryApi` evidence, not fallback API
  proofs. Other factory and constructor contract rows still name API
  identity/result semantics while local consumers prove their current `Symbol`,
  `Import`, `Source`, `Domain`, and `SequenceSurface` obligations;
- value-graph consumers that query by source span re-check the original source
  `Call` node shape and its evidence dependencies when that call can be
  recovered. This preserves receiver-method precision when value-graph CSE has
  collapsed a parameter receiver into a spanless input, and it still fails closed
  if the source span does not identify a matching call occurrence;
- normalized `HoF` nodes produced from admitted receiver-method calls preserve
  the same-span `LibraryApi(MethodCall(HoF(...)))` occurrence record as protocol
  receiver evidence. Downstream calls such as Rust `.collect()` can therefore
  depend on the admitted `filter_map`/`map`/`filter` occurrence after IL
  canonicalization, without reopening selector-only proof;
- JS/TS record-shape guard exact admission and value-graph tagging require both
  `SequenceSurface(RecordGuard)` and `Guard::JsRecordShape`; raw
  `Seq("record_guard")` cannot enter the proof-bearing exact/value-graph path by
  tag spelling alone;
- JS/TS own-property guard exact admission and value-graph map-default
  normalization require both `SequenceSurface(OwnPropertyGuard)` and
  `Guard::JsOwnProperty`; raw `Seq("own_property_guard")` plus a path-shaped
  spelling, or `QualifiedGlobal` evidence without the required root proof, is not
  proof by itself;
- sequence-surface admission for normalize/value-graph/detect exact paths where
  the surface contract is independently exact-safe; guard surfaces use their
  dedicated guard helper instead. Go zero-map literal lookup also requires
  `SequenceSurface(GoCompositeMapLiteral)` and `SequenceSurface(GoMapEntry)`,
  so `composite_literal`/`keyed_element` tag spelling alone no longer admits the
  exact map-default path;
- C byte-pack value-graph laws consume the first-party C byte-pack contract,
  `Domain(ByteArray)` base evidence, and source-cast evidence for the unsigned
  high lane where required. Raw `UnsignedCast32` builtin payloads without
  `Source(Cast(CUnsigned32))` evidence stay opaque;
- exact-fragment append, non-overloadable index-write, and self-field-write
  gates now require exact `Effect`/`Place` evidence. Missing, ambiguous,
  conflicting, or dependency-broken evidence keeps the exact path closed;
- immutable binding-domain inference, module facts, imported literal
  replacement, value-graph binding safety, and imported binding use indexing
  consume shared mutation-risk helpers for `BindingWrite`, `ReceiverMutation`,
  and `OpaqueArgumentEscape` instead of each re-scanning raw method selectors
  or call arguments independently. Exact-fragment context blocking still has a
  local assignment-shape invalidation slice; call mutation risk in that path
  uses the shared helpers.

Broader field/place/effect facts, promise receiver proof, async/sync and
Go-channel protocol convergence, unmodeled stdlib/ecosystem APIs, broader
inferred receiver-expression domain evidence, field/property/setter/proxy place
facts, full protocol/demand/effect receiver obligations for lazy generators,
set/dict materialization, channels, and async, full scope-resolution and
namespace-member evidence, broader guard evidence, general cross-module
dependency manifests, report-level provenance, and external manifest loading are
still open work.
