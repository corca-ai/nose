# Semantic evidence records

Evidence records are the internal substrate that lets current builtin
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
- Preserve existing behavior while builtin frontends emit source,
  parameter-domain, import, symbol, guard, place/effect, library API, and
  sequence-surface facts directly into the record shape.
- Make the future external pack schema a narrowing of an implemented internal
  boundary, not a speculative document-only API.

Internal Rust evidence provenance uses `EvidenceEmitter::Builtin` for shipped
nose producers and `EvidenceEmitter::External` for provider/user opt-in data.
The current serialized IL wire name for builtin evidence remains `FirstParty`
for compatibility.

## Non-goals

- Do not let evidence records mint value fingerprints, bypass laws, or mark clone
  pairs exact.
- Do not expose this internal Rust record as the final external pack manifest or
  scan JSON schema.
- Do not certify external pack claims. nose validates record shape and fails
  closed; providers own their claims, and users own opt-in decisions.
- Do not model demand as evidence records or cover every place/effect family in
  this slice. Demand/effect profiles are internal contracts for
  already-admitted operations; they are not source proof records. The current
  place/effect records cover the first exact-fragment substrate plus
  conservative binding-write, receiver-mutation, and opaque-argument-escape risk
  facts needed to keep binding/import safety fail-closed. The current
  demand/effect model is described in
  [demand-effect-semantics](demand-effect-semantics.md).

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
| `Source` | construct syntax, Rust macro invocation/range/pattern syntax, async/generator/error and Go concurrency/channel protocol boundary syntax, Python comprehension surface provenance, C unsigned-cast syntax, regex literal provenance, and source operator family including JS-like unary `typeof` |
| `Domain` | parameter, receiver-expression, or value/binding domain such as array, collection, iterable, iterator, set, map, record, option/result, promise/future-like, string, boolean, integer/float/number, byte array, or nominal type |
| `Import` | static import binding/namespace proof, Python wildcard-import ambiguity proof, Java wildcard import proof, C quote-include proof, Ruby `require` module proof, and imported-literal snapshot provenance |
| `Symbol` | resolved or proven symbol identity, with record kinds for unshadowed globals, static imported binding/namespace aliases, and selected qualified global API paths |
| `Type` | type alias or type-to-domain proof, currently exact-spelling C aliases to unsigned 8-bit and supported unsigned 32-bit integer forms plus nominal type-domain rows |
| `Guard` | multi-obligation guard proof facts such as JS/TS record-shape and own-property guard contracts |
| `Place` | fixed receiver/place facts currently covering `SelfReceiver` and `SelfField` |
| `Effect` | observable effect and mutation-risk facts currently covering canonical builder append calls, non-overloadable index writes, fixed self-field writes, binding writes, receiver-mutating calls, and opaque argument escapes |
| `LibraryApi` | proof that a specific API occurrence matches a language/API contract coordinate, currently for selected call, property, and sentinel occurrences across JS-like static/global/static-index APIs, selected Python/Rust/Ruby/Java/Swift/regex APIs, pack-proven Python/Go/Swift free-function builtins, pack-proven Python iterator builtins, pack-proven generic builtin method calls, and selected receiver-method families |
| `CallTarget` | proof that a specific call occurrence resolves to an explicit function, method, imported function/member, or dispatch-family target identity |
| `SequenceSurface` | lowered aggregate surface such as collection, tuple, map, pair, import proof, guard surfaces, Go composite map literals, or Go map entries |

`LibraryApi` evidence is an occurrence fact, not the whole contract. It records
the contract id, callee coordinate, arity, and dependencies for a specific
`Call`, `Field`, or `Var` node, depending on the contract surface.
`LibraryApiContract` rows in `nose-semantics` still name result semantics and
the remaining obligations. The implementation keeps contract identities, row
constructors, evidence-hash registry helpers, and occurrence admission separate
so this boundary can narrow into a future pack schema. Existing evidence kinds
such as `Symbol`, `Import`, `Source`, `Domain`, and `SequenceSurface` prove
those obligations; the contract decides whether the facts are enough to admit an
exact or value-graph path. A future external pack schema may expose library API
contracts, but providers still emit facts and contracts rather than exact-clone
verdicts.

`CallTarget` evidence is occurrence proof for call target identity. A raw callee
spelling such as `f(...)`, `obj.f(...)`, or `ns.f(...)` is only a selector that a
producer may inspect; consumers must require an asserted, dependency-closed,
unambiguous `CallTarget` record anchored to the exact `Call` node. The current
resolver admits only the lowered file language's builtin language-core
provenance; legacy broad-provenance, wrong-language, external, dependency-broken,
or selector-mismatched call-target rows cannot open exact call identity. The current
vocabulary separates concrete exact identity from broader dispatch facts:

- `DirectFunction` names a unique in-file function target by function span and
  selector hash. Recursion normalization, the interpreter oracle, value-graph
  pure helper inlining, and strict exact direct-function gates consume this
  variant only when the target function itself is exact-safe.
- `DirectMethod` names a concrete in-file method/function target plus receiver
  type and method selector hashes. Strict exact still separately requires the
  receiver expression to have exact identity before treating the call as opaque
  same-target identity.
- `ImportedFunction` and `ImportedMember` name imported function/member
  coordinates. They prove opaque call identity only; standard-library or
  ecosystem semantics still require `LibraryApi` occurrence evidence.
- `DynamicDispatch` names a protocol/dispatch family and method selector, but it
  does not by itself prove one concrete implementation target.

The builtin language-core call-target producer emits these records with the
matching `nose.lang.*` pack provenance for the lowered file language:

- `DirectFunction` for unique top-level in-file `Function` units when neither
  the current lexical scope nor any enclosing lexical scope has a parameter,
  assignment target, loop-pattern binding, or nested function definition that
  shadows the callee name. If the callee already carries symbol-identity
  evidence or an imported binding proof for the same local name, this direct
  raw-name path stays closed instead of overriding that proof.
- `ImportedFunction` for variable callees whose local binding has a unique
  asserted `Symbol(ImportedBinding)` proof and whose call-site occurrence can be
  materialized as dependency-backed imported-symbol evidence. Alias rebinding,
  local shadowing, ambiguous/conflicting symbol evidence, dependency-broken
  import proof, and locally visible same-name function units keep the call
  target closed.
- `ImportedMember` for field callees whose receiver has a unique asserted
  `Symbol(ImportedNamespace)` or `Symbol(ImportedBinding)` proof. Imported
  namespace receivers use the module/member coordinate; imported binding
  receivers use the module/export/member coordinate. Receiver rebinding,
  ambiguous/conflicting symbol evidence, dependency-broken import proof, and
  selector mismatches stay closed.
  Node-anchored `UnshadowedGlobal` and `ImportedNamespace` identity helpers
  admit only matching builtin language-core provenance; broad, wrong-language,
  external, ambiguous, or dependency-broken occurrence rows do not prove public
  global/imported-namespace identity.

There is not yet a builtin producer for `DirectMethod` or `DynamicDispatch`.
Duplicate function names, lexical shadowing, nested/non-top-level functions,
methods without explicit target proof, computed callees, selector mismatches,
dependency-broken records, and conflicting evidence stay closed.

Current builtin `LibraryApi` callee coordinates are intentionally specific:

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
- `LabeledFreeName` names a language-scoped free identifier plus the first
  argument label, such as Swift `Dictionary(uniqueKeysWithValues:)`, and
  depends on `Symbol(UnshadowedGlobal)` evidence at the callee anchor plus the
  contract's shadow policy. Other overload labels do not share the occurrence
  proof.
- `JavaUtilStaticMember` names selected Java `java.util` static factory/adaptor
  calls and depends on matching Java import-binding evidence plus source-origin
  local type shadow checks.
- `JavaStaticMember` names selected non-`java.util` Java static factory calls
  with an explicit module coordinate, currently Guava immutable collection
  factories under `com.google.common.collect`. It depends on matching imported
  binding evidence and local type-shadow checks; selector spelling alone stays
  closed.
- Python wildcard imports emit `Import::Wildcard` evidence. For current builtin
  Python free-name API producers, any asserted wildcard import keeps
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
- receiver-method coordinates such as string affix predicates depend on the
  receiver contract, not selector spelling alone. JS/TS `startsWith` and
  `endsWith` require primitive string receiver proof; untyped JavaScript,
  TypeScript `String` object wrappers, nullable receivers, borrowed/custom
  same-name calls, offsets, and direct `String.prototype` patching stay closed.
- `StaticIndexMembershipMethod` names JS-like `indexOf`/`findIndex` membership
  calls and depends on `SequenceSurface(Collection)` evidence for the exact
  receiver plus the static non-float literal receiver shape required by the
  contract.
- `Property` names a language-scoped property surface such as JS/TS/Java
  `length`. It is anchored to the `Field` node and depends on receiver proof
  such as `Domain`, `SequenceSurface`, or nested admitted `LibraryApi`
  evidence. It does not infer semantics from a property spelling alone.
- `Method`, `AsyncMethod`, and `IteratorAdapterMethod` name language-scoped receiver methods by
  exact method string and arity. They depend on receiver proof such as
  `Domain`, `SequenceSurface`, imported-namespace or unshadowed-global `Symbol`,
  or nested admitted `LibraryApi` evidence for factory/result calls. They do not
  infer semantics from a selector spelling alone. The current `AsyncMethod`
  row is JS-like Promise `.then`; it requires PromiseLike receiver proof.

When a `LibraryApi` occurrence depends on node-anchored
`Symbol(UnshadowedGlobal)` or `Symbol(ImportedNamespace)` proof, admission uses
the same builtin language-core provenance gate as the public symbol identity
helpers. Broad `nose.first_party`, wrong-language, external, ambiguous, or
dependency-broken symbol occurrence rows do not license free-name, namespace, or
receiver-method API evidence. `ImportedBinding` dependencies remain on the
import-backed binding path and are not reinterpreted as language-core namespace
proof.

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
substring fallback over arbitrary parameter text. The current vocabulary covers
container/protocol domains (`Array`, `Collection`, `Iterable`, `Iterator`,
`Set`, `Map`, `Record`), nullable/effect domains (`Option`, `Result`,
`PromiseLike`, `FutureLike`), scalar domains (`String`, `Boolean`, `Integer`,
`Float`, `Number`, `ByteArray`), and a hashed `Nominal` domain for
provider-proven user/library types. The frontend owns evidence emission and
alias lifecycle: for example, Python `typing`, `collections.abc`, and `asyncio`
aliases record dependency-backed parameter `Domain` evidence that depends on the
corresponding `ImportedBinding` symbol evidence, and a rebound alias closes that
path.
`Type(NominalDomain)` rows can map a scoped nominal type hash to a domain, but
type names alone are not proof; the dependent `Domain` record still needs a
valid symbol/import/scope chain. `nose-semantics` resolves `Domain` evidence on
exact receiver-expression node anchors, then binding anchors for immutable
local/module variables, then parameter anchors. A conflicting, ambiguous, or
dependency-broken receiver-domain record closes that receiver proof and must not
fall back to side-table mirrors or selector spelling. Binding-anchor lookup
matches both source span and `local_hash`, and a binding proof is applied to a
receiver only when the assignment is visible before that receiver use. When a
receiver is an alpha-renamed parameter or local binding reference, lookup is
constrained to the nearest function/lambda scope where appropriate so
same-numbered parameter/local ids from other units do not prove the current
receiver. Method receiver contracts expose their domain-backed obligations
through `DomainRequirement`; obligations such as imported namespace, unshadowed
global, exact map literal, and future demand/effect constraints remain separate
checks.

Parameter `Domain` evidence also seeds the semantic-kernel `ValueDomain`
contract used by value-graph and recursion laws. That bridge is intentionally
narrow: integer/float/number domains seed `Number`, boolean seeds `Boolean`,
string seeds `String`, and only array/collection/set domains seed `Sequence`.
Iterable, iterator, record, option/result, future-like, and nominal domains do
not automatically become sequence or exact value-law proof. The value graph may
additionally infer a domain from strict operator use, literal result domains,
modeled builtin result domains, and subexpression result domains, but the law
itself is admitted only through a `ValueLaw` contract in `nose-semantics`. This
keeps string and sequence concatenation ordered when evidence proves those
domains, while numeric and boolean laws still require positive domain proof.
The first pack-facing law pilot now reports per-family provenance for selected
compiled builtin value laws through `nose.value_graph.laws`: numeric
common-factor distribution and integer ordered min/max clamp. That provenance
records the pack id, stable law id, exact channel, proof status, and formal
obligation id only when the value graph actually applied the law. It is not a
new `EvidenceRecord` family, and external LawPack manifests remain
`metadata-only` until a future producer/runtime path can satisfy the same
kernel-owned law registry boundary.

Selected `LibraryApi` result-domain evidence follows the same model. A
first-party factory call result may carry `Domain(Collection)`, `Domain(Set)`,
`Domain(Map)`, or `Domain(Array)` only after the call occurrence has admitted
`LibraryApi` evidence. The `Domain` record depends on that `LibraryApi` record,
so broken import, source, shadowing, or symbol proof closes the result-domain
claim as well. The result-domain record proves only the container/protocol shape
of the call result; exact consumers still prove argument safety, entry shape,
mutation, receiver requirements, and demand/effect obligations separately.
Normalize-owned result-domain refreshes, helper receiver-domain facts, and
helper call-site `Symbol` occurrence facts use the lowered file language's
builtin language-core provenance, while the licensing `LibraryApi` occurrence
keeps the specific builtin pack provenance. When these refreshes encounter
older broad `nose.first_party` rows for the same asserted fact, they update the
row in place instead of minting a duplicate; if an equivalent current row
already exists, stale broad duplicates are closed as ambiguous.

Sequence-surface evidence is also authoritative for exact/value-graph aggregate
semantics. A lowered `Seq("array")`, `Seq("object")`, `Seq("tuple")`, or
language-specific tag does not by itself prove exact-tree safety, collection
membership, map-entry-list shape, imported-literal eligibility, or a canonical
value-graph tag. Consumers resolve the tag only when a matching
`SequenceSurface` record exists at the same sequence anchor with matching
builtin language-core provenance and its dependencies remain asserted. Missing,
conflicting, ambiguous, broad/wrong-language, external, or wrong-kind surface
evidence keeps the exact/value-graph path closed. In the value graph, a missing
surface record produces the untagged sequence value rather than a raw
spelling-derived hash, so a payload name cannot become a semantic proof channel
by coincidence.

Qualified global identity is also evidence, not a selector guess. The current
first-party JS/TS producer emits `QualifiedGlobal` only for selected static paths
whose root is proven unshadowed, such as `Object.hasOwn`,
`Object.prototype.hasOwnProperty.call`, `Object.keys`, `Array.from`, and
`Array.isArray`. The `QualifiedGlobal` record itself depends on same-span source
evidence for the unshadowed root (`Object` or `Array` today), and exact/value
consumers reject the path when that dependency is missing, ambiguous, or broken.

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

JS/TS `Object.keys(obj)` map-key views use the same evidence-first rule under
`nose.protocols.map_key_views`. The admitted `LibraryApi` record is a
`StaticGlobalMethod` contract for `Object.keys`, and depends on
`QualifiedGlobal("Object.keys")`, `UnshadowedGlobal("Object")`, and the object
argument proof. Inline object literals require `SequenceSurface(Map)` at the
object sequence; local bindings additionally require the initializer
`Effect(BindingWrite)` and reject intervening mutation or argument escape before
the `Object.keys` call, including JS `delete` property mutation and `for...in` /
`for...of` loop target writes. Consumers re-run that object-argument proof when
admitting the record. A detached API row, `Object.values`, `Object.entries`,
shadowed `Object`, mutation, nested local function declarations that can close over
the object, direct `eval`, `with` scopes over or enclosing the object use,
object-literal `__proto__` prototype syntax, escaped identifier keys, numeric
literal keys whose runtime property names need JS canonicalization, or missing
surface evidence does not prove a key-view.

Place and effect evidence are authoritative for the exact-fragment substrate,
value-graph field-state consumers, oracle field state, and conservative mutation
invalidation. Raw method selectors such as `push`, `append`, or `add` do not prove
an append effect; exact consumers need
`Effect(BuilderAppendCall)`, even when the call has already been lowered to
canonical `Builtin::Append`. Likewise, non-overloadable index writes and fixed
self-field writes are admitted only through exact `Effect` records, with
`Place(SelfReceiver)` and `Place(SelfField)` proving the receiver/place side.
Builtin language-core `Place(SelfField)` depends on the matching
`Place(SelfReceiver)`, and `Effect(SelfFieldWrite)` depends on the matching
`Place(SelfField)`.
Missing, conflicting, ambiguous, or dependency-broken place/effect evidence
closes exact fragments, value-graph field readback/final field sinks, and oracle
field state instead of reopening a legacy language/shape fallback.

Mutation-risk effects are intentionally separate from exact effect proofs.
`Effect(BindingWrite)` says an assignment node writes its syntactic target;
`Effect(ReceiverMutation)` says a call is covered by a builtin language-scoped
receiver-mutating method-effect contract row; and
`Effect(OpaqueArgumentEscape)` says an ordinary call argument may escape to
unknown code. These records can invalidate immutable binding, module export,
imported literal, value-graph, or exact-fragment context assumptions, but they
do not prove collection membership, builder append, map update, or any other
exact semantic law. Known admitted `LibraryApi` occurrences suppress the opaque
escape helper for that call, so a modeled API such as an imported stdlib
predicate is not treated as arbitrary mutation risk merely because it takes an
argument.

Builder and map-builder value laws consume emitted or admitted evidence, not raw
selectors. A method-effect row for list builders names the producer-side
language, selector, arity, required `Effect(BuilderAppendCall)`, and
active-builder receiver obligation. Value-graph list-builder recognition
requires that effect record, an admitted same-span append `LibraryApi`
occurrence, or the method-effect row itself under active-builder context. An
index-write row for map builders names the language, required write evidence,
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
`Keys`, C `UnsignedCast32` with `nose.lang.c` provenance-backed
`Source(Cast(CUnsigned32))`, or Python builtin collection factories with
`nose.python.builtins.collection_factories` provenance-backed `LibraryApi`
occurrence evidence, or Python imported `collections.deque` collection
factories with `nose.python.stdlib.collection_factories` provenance-backed
`LibraryApi` occurrence evidence, or Ruby `require "set"; Set.new(...)`
factories with `nose.ruby.stdlib.set` provenance-backed `LibraryApi` occurrence
evidence, or Rust `Vec::new`/`vec!` factories with `nose.rust.stdlib.vec`
provenance-backed `LibraryApi` occurrence evidence, or Rust `Ok`/`Err`
constructor channels and exact-Result `is_ok`/`is_err` predicates with
`nose.rust.stdlib.result` provenance-backed `LibraryApi` occurrence evidence
when selector and receiver proofs are not locally shadowed,
or selected Rust
`std::collections::{HashSet,BTreeSet,VecDeque}::from` factories with
`nose.rust.stdlib.collection_factories` provenance-backed `LibraryApi`
occurrence evidence, or selected Rust
`std::collections::{HashMap,BTreeMap}::from` factories with
`nose.rust.stdlib.map_factories` provenance-backed `LibraryApi` occurrence
evidence, or Swift `Array(sequence)`, `Set(sequence)`, and
`Dictionary(uniqueKeysWithValues:)` factories with
`nose.swift.stdlib.collection_factories` provenance-backed `LibraryApi`
occurrence evidence, or Java `java.util.Map.of`/`Map.ofEntries` factories with
`nose.java.stdlib.map_factories` provenance-backed `LibraryApi` occurrence
evidence, or Java `java.util.List.of`/`Set.of`/`Arrays.asList` factories with
`nose.java.stdlib.collection_factories` provenance-backed `LibraryApi` occurrence
evidence, or Java empty `new ArrayList<>()`/`new LinkedList<>()` constructors
with `nose.java.stdlib.collection_constructors` provenance-backed `LibraryApi`
occurrence evidence, or case-sensitive string prefix/suffix predicates with
`nose.protocols.string_affix_predicates` provenance-backed `LibraryApi`
occurrence evidence under exact primitive string receiver proof or Go imported
`strings` namespace proof. Canonical
`Append` still needs `Effect(BuilderAppendCall)`, and the builtin language-core
normalize producer emits that effect only when the same call also has the
same-span `LibraryApi` proof for the append API; the effect record depends on
that API record. Raw or unadmitted builtin payloads stay opaque in the value
graph and closed in exact/oracle consumers.
When a receiver obligation makes an API result more specific than the selector
alone, the dependency chain must prove that specialization. For example, Rust
`unwrap_or` is an option defaulting API in isolation, but the canonical
`GetOrDefault(map, key, default)` builtin is admitted only when the same
`unwrap_or` occurrence has the Rust `RustMapGetOrExactOption` receiver contract
and depends on an admitted pack-proven `MapGet` occurrence for the exact
receiver.

Imported API occurrence evidence is not a broad name guess. A call-site
`Symbol(ImportedBinding)` or `Symbol(ImportedNamespace)` dependency must itself
depend on the matching binding-anchor symbol, pass rebinding and local/parameter
shadow checks, and match the current receiver/callee span when that span is
available. If normalization erases an import occurrence into a seeded import
value, consumers pass no occurrence span and rely on that validated dependency
instead of accepting an unrelated imported symbol elsewhere in the file.
Recall-loss reports use these same evidence boundaries when attributing
fail-closed exact admission to callee identity, receiver domain, library API
occurrence, HOF demand/effect, source surface, mutation/effect, and unsupported
runtime buckets. See
[recall-loss-recovery-loop](recall-loss-recovery-loop.md) for the checked-in
baseline and cycle process.

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
  materialization even when the lowered HOF body shape is similar. Rust range
  expressions emit half-open or inclusive range source facts, Rust macro
  invocations emit call source facts, and Rust tuple-struct single-wildcard
  patterns such as `Some(_)` emit pattern source facts. These facts are syntax
  provenance only: the full-index range rewrite also needs an admitted `len`
  contract, Option pattern predicates also need admitted `Some`/`None` API
  occurrence evidence and receiver-domain proof, and unmodeled Rust macros stay
  in recall-loss `source-surface-proof-missing` until a macro expansion or
  library contract proves their behavior;
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
- frontend lowering and normalize refreshes emit builtin language-core
  `Place(SelfReceiver)` for Java `this`, `Place(SelfField)` for Java
  `this.field`, `Effect(SelfFieldWrite)` for Java `this.field = ...`, and
  `Effect(NonOverloadableIndexWrite)` for C/Go/Java index writes. Normalize also
  emits language-core `Effect(BuilderAppendCall)` for canonical `Builtin::Append`
  only when a same-span append `LibraryApi` record licenses that canonical form,
  and records the API evidence as a dependency. They also emit mutation-risk
  effects: `Effect(BindingWrite)` for assignment nodes, `Effect(ReceiverMutation)`
  for calls admitted by the builtin language-scoped mutating-method policy, and
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
  and exact-Array receiver `map`/`filter`/`flatMap` plus `some`/`every` with
  `nose.javascript.builtins.array` provenance; `new Map(...)` and `new Set(...)`
  with `nose.javascript.builtins.collection_constructors` provenance;
  `Boolean(...)` with `nose.javascript.builtins.boolean` provenance; regex
  literal `.test(...)` with
  `nose.javascript.builtins.regex` provenance; and static
  `indexOf`/`findIndex` membership calls whose receiver has collection
  sequence-surface proof with
  `nose.javascript.builtins.static_index_membership` provenance; Python builtin
  collection factories such as
  `list(...)` when the callee has an unshadowed free-name proof; Python
  `collections.deque(...)` through imported binding/namespace proof; Python
  `math.prod(...)` with `nose.python.stdlib.math` provenance through imported
  namespace proof; Rust
  `vec!(...)` with `nose.rust.stdlib.vec` provenance when macro-invocation
  source syntax and macro-name shadow policy are proven, `Vec::new()` with the
  same pack provenance when the root-shadow policy is proven, `Some(...)`,
  selected `Some(_)` pattern selectors, bare `None`, and `and_then(...)` with
  `nose.rust.stdlib.option` provenance when Option receiver or selector proof is
  satisfied, `Ok(...)`/`Err(...)` constructors, selected `Ok(_)`/`Err(_)`
  pattern selectors, and exact-Result `is_ok`/`is_err` predicates with
  `nose.rust.stdlib.result` provenance when Result selector or receiver proof
  is satisfied, primitive integer `abs`/`min`/`max`/`clamp` receiver methods with
  `nose.rust.stdlib.integer_methods` provenance when exact integer receiver
  proof is present, Java `Math.abs`, `Math.min`, and `Math.max` scalar integer
  APIs with `nose.java.stdlib.math` provenance when unshadowed `Math` and
  integer-domain proof are present, Java/Rust/JS-family `map.get(key)` lookups
  with `nose.protocols.map_get` provenance when exact-map receiver proof is
  present, Python `dict.get(key, default)`, Ruby `Hash#fetch(key, default)` or
  zero-arg block fallback, and Java `Map.getOrDefault(key, default)` lookups with
  `nose.protocols.map_get_default` provenance when exact-map receiver proof is
  present, receiver-method membership calls with
  `nose.protocols.receiver_membership` provenance when their map, collection, or
  set-or-map receiver proof is present, Python/Ruby `keys`, Java `keySet`, and
  JS-family `Map.keys()` views with `nose.protocols.map_key_views` provenance
  when exact-map receiver proof is present, and JS/TS `Object.keys` views with
  the same provenance when static object-argument proof is present; JS/TS/HTML-family
  and Java `.length`, plus Swift `count` and
  `isEmpty`, with `nose.protocols.property_builtins` provenance when
  receiver-domain proof is present, Rust
  `iter`/`into_iter`/`iter_mut`/`collect`/`to_vec`/`copied`/`cloned` and Java
  `.stream()` iterator identity adapters with
  `nose.protocols.iterator_identity_adapters` provenance when protocol receiver
  proof is present, Rust iterator `map`/`filter`/`filter_map`/`flat_map` HOF
  adapters and `any`/`all`/`count` terminals with
  `nose.protocols.sequence_hof_adapters` provenance when protocol receiver proof
  is present, and Swift `map`/`filter`/`flatMap` HOFs with the same provenance
  when Array/Collection receiver proof and inline effect-closed callback proof
  are present, and Ruby Enumerable `map`/`collect`/`select`/`filter`/`reject`
  HOFs with the same provenance when Array/Collection receiver proof and inline
  effect-closed block proof are present, generic method-call and namespace-call builtin semantics with
  `nose.protocols.builtin_method_calls` provenance when no narrower pack owns
  the row, Go `strings.HasPrefix`/`HasSuffix` namespace calls with
  `nose.protocols.string_affix_predicates` provenance when imported `strings`
  namespace proof is present, Go `fmt.Print*`, `strings.Contains`, and
  `slices.Contains` namespace calls with `nose.go.stdlib.namespace_calls`
  provenance when imported-namespace proof is present, and selected
  `std::collections::{HashSet,BTreeSet,VecDeque}::from(...)` factory paths with
  `nose.rust.stdlib.collection_factories` provenance when their root-shadow
  policy is proven, and selected
  `std::collections::{HashMap,BTreeMap}::from(...)` factory paths with
  `nose.rust.stdlib.map_factories` provenance when their root-shadow policy is
  proven, and Swift `Array(sequence)`, `Set(sequence)`, and
  `Dictionary(uniqueKeysWithValues:)` with
  `nose.swift.stdlib.collection_factories` provenance when unshadowed type-name
  proof and supported entry-shape proof are present. The selector occurrence
  does not by itself
  prove the pattern semantics: `Some(_)` value-graph presence predicates also
  require the Rust tuple-struct wildcard `Source::Pattern` fact, and
  `Ok(_)`/`Err(_)` channel predicates require that pattern fact plus Result
  domain evidence on the scrutinee. JS/TS/Java
  `length` property reads whose
  receiver proof is satisfied; Ruby
  earlier top-level `require "set"; Set.new(...)` through `Import::Require`
  plus unshadowed `require` and `Set` proof; Java `java.util` static
  factories/adapters including `List.of`, `Set.of`, and `Arrays.asList` with
  `nose.java.stdlib.collection_factories` provenance, `Map.of`/`Map.ofEntries`
  with `nose.java.stdlib.map_factories` provenance, Guava `ImmutableList.of`,
  `ImmutableSet.of`, and `ImmutableMap.of` with
  `nose.java.ecosystem.guava.immutable_collection_factories` provenance,
  `Map.entry` with
  `nose.java.stdlib.map_entries` provenance, `Arrays.stream` with
  `nose.java.stdlib.static_collection_adapters` provenance, plus selected empty
  `new ArrayList<>()`/`new LinkedList<>()` constructors with
  `nose.java.stdlib.collection_constructors` provenance through exact or
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
  nodes: map `get`, map get-default, map-key views, iterator identity adapters,
  and the language-scoped method-call contracts currently used for
  collection/map membership, count, predicates, pack-owned Rust scalar integer
  methods, Rust `Option::and_then`, Rust Result `is_ok`/`is_err`, Rust `zip`,
  HOF, and reduction methods. Python builtin `map`/`filter` HOFs and
  `zip`/`enumerate`/`any`/`all` occurrences live in the narrower
  `nose.protocols.iterator_builtins` pack instead of the generic
  free-function-builtin pack, and their records depend on unshadowed builtin
  symbol proof plus iterable-source proof where the contract requires it.
  Property cardinality such as JS/TS `length` is
  modeled as `Property`, not as a method call. The post-binding refresh exists
  because immutable
  binding-domain evidence is inferred after lowering; the final refresh exists
  because CFG/dataflow/algebra rewrites can replace receiver expressions with
  equivalent sequence or result values. Refreshing upserts first-party occurrence
  records so `VALUES = List.of(...); VALUES.contains(x)` depends on the current
  binding or sequence-domain proof rather than falling back to selector
  matching;
- selected `LibraryApi` producer-covered result calls emit dependent
  receiver-expression `Domain` evidence: Python `list`/`tuple` and
  `collections.deque`, Rust `Vec::new`, `vec!`, and pack-owned Rust
  `std::collections::VecDeque::from`, pack-owned Java `List.of` and zero- or
  multi-argument `Arrays.asList`, and pack-owned Java empty
  `new ArrayList<>()`/`new LinkedList<>()` as `Collection`; Python
  `set`/`frozenset`, pack-owned
  Rust `std::collections::{HashSet,BTreeSet}::from`, pack-owned Java `Set.of`,
  Ruby `Set.new`,
  and JS-like `new Set` with
  `nose.javascript.builtins.collection_constructors` provenance as `Set`; Rust
  `std::collections::{HashMap,BTreeMap}::from`, pack-owned Java
  `Map.of`/`Map.ofEntries`, and JS-like `new Map` with
  `nose.javascript.builtins.collection_constructors` provenance as `Map`;
  JS-like
  one-argument `Array.from` with `nose.javascript.builtins.array` provenance as
  `Array`; and JS-like `Promise.resolve` plus admitted Promise `.then` results
  with `nose.javascript.builtins.promise` provenance as `PromiseLike`.
  `Map.entry`, `Array.isArray`, `Boolean`, regex `.test`,
  `math.prod`, `Arrays.stream`, map `get`, iterator adapters, JS Array HOFs, and
  generic method contracts do not emit `Domain` records because their result
  domains are consumed through admitted API dependencies rather than materialized
  as standalone receiver facts;
- lowered `Seq` surfaces emit `SequenceSurface` evidence under the matching
  builtin language-core pack, including Go map literal and Go map-entry surfaces
  where those tags carry first-party meaning.

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
  keeps node-anchored receiver evidence, builtin language-core immutable
  local/module binding evidence, scoped parameter evidence, selected API
  result-domain evidence,
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
  evidence. Provider literal export safety is now a shared `nose-semantics`
  policy admitting concrete root literals, requiring sequence-surface proof for
  literal containers, using Go zero-map literal/entry contracts for Go imported
  map values, and requiring admitted `LibraryApi` proof for supported Java/Rust
  map factories, while corpus-level module/export matching remains
  frontend-owned. Go namespace-member snapshots such as `tables.Lookup` are
  consumer rewrites, not broad namespace proof: they require asserted namespace
  import evidence, a unique provider export, no provider mutation/escape, no
  consumer namespace rebinding or parameter shadowing, and no member write,
  receiver mutation, or opaque argument escape;
- imported namespace/binding symbol proof for normalize idiom admission,
  value-graph namespace fallbacks, and strict exact gates, without raw assignment
  fallback;
- value-graph internal import identity now uses dedicated
  `ImportNamespace`/`ImportBinding` value ops derived from `Import` evidence, so
  raw import `Seq` payloads cannot hash-cons with proof-bearing import values;
- unshadowed-global symbol proof for Java `Math.*` scalar integer contracts,
  JS/TS `Math.*` method boundaries,
  pack-owned `new Map(...)`/`new Set(...)` constructor contracts, regex literal
  `.test(...)` contracts, static `Array.from` and `Array.isArray` exact/API
  gates, and `undefined` nullish-default handling.
  Value-graph nullish
  value semantics are evidence-only for `undefined`; raw spelling plus a
  scope scan no longer reopens that exact path;
- qualified-global symbol proof for selected JS/TS API paths: own-property
  guard evidence depends on `Object.hasOwn` or
  `Object.prototype.hasOwnProperty.call`, static-object map-key views require
  evidence for `Object.keys`, and map-key view wrappers require evidence for
  `Array.from`; static Array gates require evidence for
  `Array.isArray`. Those `QualifiedGlobal` records themselves depend on the
  appropriate unshadowed root proof, so path-shaped text or a detached API
  record does not prove identity;
- selected `LibraryApiContract` consumers now consult `LibraryApi` occurrence
  evidence first for the migrated JS-like, Python builtin/imported, Rust
  free-name/path/Option/scalar, Ruby require-backed, Java Math/static/property,
  regex-literal, property, and receiver-method surfaces;
  conflicting or dependency-broken API evidence keeps
  the value-graph, idiom, and strict exact paths closed. Missing API evidence is
  now also closed for those producer-covered surfaces; older symbol/source proof
  helpers remain dependency inputs to `LibraryApi` evidence, not fallback API
  proofs. Factory and constructor consumers still prove their channel-specific
  argument, entry-shape, mutation, `Source`, `Domain`, and `SequenceSurface`
  obligations, but API occurrence admission itself is shared where covered;
- strict exact consumers share the same admitted occurrence resolver layer for
  selected method, pack-proven map-get, pack-proven map-get-default,
  pack-proven map-key-view, regex, JS static/global,
  static-index, iterator-adapter, Rust Option sentinel, Rust `Vec::new`, and first-party
  collection/map factory and constructor paths instead of locally recombining
  selector strings with evidence checks. Opaque same-callee exact identity
  remains separate: it can keep identical calls comparable, but it does not
  assign cross-language or library semantics;
- normalize idiom canonicalization shares the admitted occurrence resolver layer
  for pack-proven supported free-function builtins, pack-proven Python iterator
  builtin HOFs, pack-proven generic builtin method contracts, pack-proven map `get`,
  pack-proven map get-default, pack-proven map-key
  views, iterator identity adapters with
  `nose.protocols.iterator_identity_adapters` provenance, Rust sequence-HOF
  adapters, Swift Array/Collection HOFs, and Ruby Enumerable HOFs with
  `nose.protocols.sequence_hof_adapters` provenance, Java
  `Arrays.stream`, Java map entries, Rust `Some(...)`, Rust map factory receiver
  proof, and HOF receiver proof instead of locally recombining selector strings
  with `LibraryApi` evidence checks.
  Test fixtures may still use row constructors to mint synthetic evidence
  records;
- value-graph direct API eval paths, node-level API consumers, and provider
  literal export safety share the same admitted occurrence resolver layer where
  they still have the source `Call` or `Field` node. This includes direct
  factory/constructor eval, property builtins such as JS/TS/Java `.length` with
  `nose.protocols.property_builtins` provenance, Rust `Some` callee-node checks,
  static index-membership, Rust scalar integer method calls under
  `nose.rust.stdlib.integer_methods`, Java Math scalar integer calls under
  `nose.java.stdlib.math`, builder append API admission, pack-owned
  Promise `resolve`, and
  Promise `.then` contract lookup. Promise continuation reduction additionally requires a
  recoverable supported settled value and preserves a Promise boundary in the
  value graph. Value-level CSE paths that only retain source
  spans now also go through span-query resolvers for free-name/imported
  collection factories, Java/Ruby/Rust collection factories, Java collection
  constructors, free-name/Java map factories, Java map entries, pack-proven map
  `get`, pack-proven map get-default, and pack-proven map-key view/wrapper
  calls. The value graph no longer locally recombines those contract rows with `LibraryApi`
  span evidence;
- value-graph consumers that query by source span re-check the original source
  `Call` node shape and its evidence dependencies when that call can be
  recovered. This preserves receiver-method precision when value-graph CSE has
  collapsed a parameter receiver into a spanless input, and it still fails closed
  if the source span does not identify a matching call occurrence;
- normalized `HoF` nodes produced from admitted receiver-method calls preserve
  the same-span `LibraryApi(MethodCall(HoF(...)))` occurrence record as protocol
  receiver evidence. Downstream calls such as Rust `.collect()` can therefore
  depend on the admitted `filter_map`/`map`/`filter` occurrence after IL
  canonicalization, without reopening selector-only proof. For Rust iterator
  chains, Swift Array/Collection chains, and Ruby Enumerable chains, that
  same-span HOF record must carry `nose.protocols.sequence_hof_adapters`
  provenance. Ruby `reject` records carry the distinct negated-filter HOF kind,
  so `reject { p }` does not reuse the positive `filter { p }` predicate.
  Value-graph filter
  consumers such as `len(filter(...))`, explicit reductions over a filter, and
  static literal membership shortcuts reuse the same HOF admission; a raw
  `HoF(Filter)` payload no longer enters those paths by shape alone;
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
  the surface contract is independently exact-safe, now requiring matching
  builtin language-core provenance. Guard surfaces use their dedicated guard
  helper instead. Go zero-map literal lookup also requires
  `SequenceSurface(GoCompositeMapLiteral)` and `SequenceSurface(GoMapEntry)`, so
  `composite_literal`/`keyed_element` tag spelling alone no longer admits the
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

Broader field/place/effect facts, thenable assimilation, async/sync and
Go-channel protocol convergence, unmodeled stdlib/ecosystem APIs, broader
inferred receiver-expression domain evidence, field/property/setter/proxy place
facts, pack-facing demand/effect rows for lazy generators, set/dict
materialization, channels, async, call-by-need, and callback effects, full
scope-resolution and namespace-member evidence, broader guard evidence, general
cross-module dependency manifests, report-level provenance, and external
manifest loading are still open work.

## See also

Back to [semantic-kernel](semantic-kernel.md). Current implementation status is
recorded in [semantic-kernel-snapshot](semantic-kernel-snapshot.md); history and
remaining work are tracked in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). Source-origin facts are
covered in [source-facts](source-facts.md).
