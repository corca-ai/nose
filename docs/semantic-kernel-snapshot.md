# Semantic kernel snapshot

Back to [semantic-kernel](semantic-kernel.md). This page records the current
implementation shape; planned work and decision history live in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). The internal evidence
record substrate is described in [evidence-records](evidence-records.md).

Snapshot date: 2026-06-07, current implementation after the semantic-kernel
foundation and follow-up facade migrations through receiver-aware field state
sequence-surface contracts, proof-backed append fragment evidence, and the first
operator-law contracts, typed import facts, and source-fact gates for construct,
literal, equality/operator provenance, and the first shared evidence-record
substrate for source, domain, import, and sequence-surface facts.

## What exists today

nose now has a first internal semantic-kernel facade, but most of the engine is
still being migrated toward it.

- `nose-il` defines a compact shared IL, `Lang`, `Builtin`, `HoFKind`, operators,
  literals, source spans, units, compatibility parameter/source facts, and
  pack-facing internal `EvidenceRecord` facts.
- `nose-semantics` defines the first-party semantic profile facade: language,
  source-fact, operator, effect, fragment, module, stdlib, builtin, method-call,
  property, async, iterator-adapter, builder-append, and factory contracts.
- `nose-frontend` owns tree-sitter parsing, per-language lowering, embedded
  `<script>` extraction, import facts, source-origin fact emission, and Raw-node
  coverage.
- `nose-normalize` owns desugaring, alpha-renaming, recursion normalization,
  dataflow, CFG/algebra normalization, type-gated value-graph rules, and the
  interpreter oracle.
- proof-sensitive value-graph laws are starting to move into named rule modules
  under `crates/nose-normalize/src/value_graph/rules/`; `clamp` and
  `promise_then` are the current examples.
- `nose-detect` owns unit extraction, exact fragment contracts, effect fragments,
  value/shape features, candidate generation, clustering, and ranking.
- `formal/obligations` records proof obligations for proof-sensitive rules.

The current model already enforces the main product principle: exact semantic
matches must be fail-closed and false merges are bugs.

## Implemented facade contracts

The current facade is compiled Rust, not an external manifest schema. It is
intended to make the future pack extension boundary explicit while behavior is
migrated.

- The first-party profile exposes pack id and trust policy separately from
  channel eligibility. `ChannelEligibility` describes where a fact may be used;
  first-party/default status is pack provenance, not an analysis channel.
- `Il::evidence` is now the shared internal substrate for source, domain, import,
  and sequence-surface proof facts. Records carry ids, stable source anchors,
  kind, provenance, dependencies, and asserted/ambiguous status. Lookups in
  `nose-semantics` fail closed on ambiguous or conflicting evidence and use older
  side tables only as compatibility fallback when no relevant evidence record is
  present.
- `OperatorSemantics` now owns the first shared operator contracts:
  comparison-direction transforms, comparison negation, equality operand
  commutativity, comparison-lattice laws, abs/min/max/selection guard laws,
  static cardinality thresholds, JS-like static `indexOf`/`findIndex`
  thresholds, and source membership operators. Algebra normalization, CFG
  branch orientation, value-graph comparison/count rewrites, and strict exact
  static-index gates consume these contracts instead of local operator tables.
  The old `primitive_order_comparisons()` helper remains as a compatibility
  wrapper around the stricter lattice law contract.
- Source facts are now first-class internal evidence for source distinctions that
  the shared IL erases. JS/TS frontends emit construct syntax, regex literal,
  strict/loose equality, strict/loose inequality, and `instanceof` facts. Python
  emits value equality/inequality and identity equality/inequality facts. These
  are mirrored into `EvidenceRecord::Source`; the older `SourceFact` vector
  remains a compatibility fallback. Normalize and detect consume source facts
  only where a semantic contract requires that exact source surface.
- Free-function builtin contracts are language- and arity-constrained and require
  unshadowed builtin/global proof before exact lowering.
- Method contracts carry receiver obligations such as exact collection, exact
  protocol, exact option, exact string, exact primitive integer, exact map literal,
  imported namespace, or unshadowed global.
- Source-level `ParamSemantic` facts are mirrored into
  `EvidenceRecord::Domain`, and normalize/detect receiver-domain gates consume
  domain evidence through `nose-semantics` helpers. This preserves the current
  Array/Collection/Set/Map/Option/String/Integer/Number/ByteArray distinctions
  while moving the proof storage toward the pack-facing evidence substrate.
- Property builtin contracts are language-constrained; a selector such as
  `length` is not enough without receiver proof. JS/TS `filter(...).length`
  is admitted only after the receiver has already entered a proven collection/HOF
  value. JS object `.length` remains a property read, not collection cardinality.
- Promise `.then` has a JS-like surface contract, but exact beta-reduction is
  closed until a pack/frontend can prove a Promise-like receiver.
- Rust iterator identity adapters (`iter`, `into_iter`, `collect`, `to_vec`,
  `copied`, `cloned`) are language-, arity-, and receiver-proof constrained.
  Normalize's exact protocol receiver admission consumes this same contract
  instead of accepting same-named methods from other languages.
- Rust method `zip(...)` is admitted as a protocol-pair operation only through
  the Rust `method_call_contract` and exact protocol proof for both sides.
- Rust stdlib path contracts for `Some`/`Option::Some`,
  `None`/`Option::None`, `Option::and_then`, and `Vec::new` carry the exact
  selector and shadow-root requirement through `nose-semantics`;
  normalize/detect still perform the local scope shadow check. The Rust
  frontend preserves bare `None` as a name rather than lowering it directly to
  null, so Option absence is admitted only through the contract.
- Collection and map factory contracts have started moving into the facade.
  Shared rows now cover Python free-name factories (`list`, `set`,
  `frozenset`, `tuple`), Python imported `collections.deque`, Rust
  `std::collections::{HashSet,BTreeSet,VecDeque,HashMap,BTreeMap}::from`,
  Java `List.of`/`Set.of`/`Arrays.asList`, Java `Map.of`/`Map.ofEntries`/
  `Map.entry`, and Ruby `require "set"; Set.new(...)`. Callers still prove the
  local import, require, shadowing, entry-shape, mutation, and exact-safety
  obligations.
- Java empty collection constructor contracts cover `new ArrayList<>()` and
  `new LinkedList<>()` only for the Java `java.util` list types. Simple names
  require `java.util` import proof and no local type declaration with the same
  simple name. A `java.util.*` wildcard import is not enough when another
  package explicitly imports the same simple type; fully-qualified
  `java.util.*List` names carry the namespace proof in the selector itself.
- Builder append contracts are separate from arbitrary method calls. A selector
  such as `push`, `append`, or `add` is not proof by itself. First-party
  frontend/normalize paths must prove the receiver or active-builder contract and
  lower the call to canonical `Builtin::Append` before exact fragments can treat
  it as an append effect. Value-graph active list-builder paths still consume the
  method selector only after a local builder seed is active.
- Exact fragment surface proofs for Java `this.field`, Java `return this`,
  non-overloadable C/Go/Java index assignment, and single-item builder append
  calls are now shared through `nose-semantics`; predicate and contract paths
  consume the same IL-level proof helpers. Raw selector-only append calls stay
  exact-closed as append effects, though they may still participate in the
  separate opaque-call policy as generic `Other` effect context.
- Value-graph and oracle same-unit field state are receiver-aware: a cached write
  is keyed by receiver/place plus field name, so `a.x = v` can satisfy `a.x`
  but not `b.x`, and final field-write sinks preserve the receiver identity.
  Same-unit value-graph readback uses syntactic receiver/place evidence only; it
  does not assume aliasing or computed call-result receivers.
- `SeqSurfaceContract` now centralizes first-party lowered sequence tags and
  keeps separate axes for exact-tree safety, membership-collection admission,
  map-entry-list admission, imported-literal eligibility, and value-graph
  canonical tags. Strict exact gates, value-graph sequence lowering, and
  sibling-module literal export checks consume this contract instead of local
  string allowlists. Untagged `Seq` remains an internal grouping surface and
  does not itself prove exact collection semantics; the older Python empty
  sequence collection case is handled only by the explicit collection profile
  path.
- Collection reductions such as Rust `Iterator::count()` and Java
  `Stream.count()` are admitted through exact protocol receiver contracts, not
  through a bare method-name check.
- Java stream source adapters are split by proof: `receiver.stream()` requires
  an exact iterable receiver, while `Arrays.stream(xs)` requires the
  `java.util.Arrays` import binding and no local `Arrays` type shadow.
- Cross-file immutable import replacement now preserves import-binding
  dependencies used by the exported literal expression, so a Java static import
  of `LOOKUP = Map.of(...)` carries the provider's `java.util.Map` proof into
  the importing file. Provider and importer module-binding mutation proof now
  rejects direct binding mutations and direct place writes such as
  `LOOKUP.clear()` and `LOOKUP[key] = value`, and provider-side opaque argument
  escapes such as `mutate(LOOKUP)`, before imported literal provenance can enter
  exact matching.
- Membership and map-key membership selectors now consume language-scoped method
  contracts before normalize/detect treat them as semantic containment. A method
  named `contains` is Java/Rust collection membership only; JavaScript
  `.contains(...)` is not accepted as array membership. Map-key examples include
  Java `Map.containsKey`, Java `keySet().contains`, Rust `contains_key`, Rust
  `get(key).is_some()`, Ruby `key?`/`has_key?`, Python `__contains__`, and
  TypeScript `Array.from(map.keys()).includes(key)` when the receiver is a
  typed/proven map.
- Map key-view contracts distinguish collection views from iterator views:
  Python/Ruby `keys` and Java `keySet` are collection views, while JS-like
  `Map.keys()` is an iterator view and needs the `Array.from(...)` wrapper
  contract before it can feed exact membership.
- Map lookup surfaces that return a value/option are now explicit contracts for
  Java/Rust/JS-like `get(key)` plus an exact-map receiver requirement. Python
  `dict.get(key, default)`, Java `getOrDefault`, and Ruby `fetch` still use the
  `GetOrDefault` method contract. Ruby `fetch(key) { fallback }` carries a
  separate zero-arg-lambda fallback argument contract, so block fallback demand
  is not inferred from the selector name in normalize/detect.
- JS-like static array `indexOf`/`findIndex` membership surfaces are explicit
  contracts, including the static non-float literal collection requirement and
  accepted `-1`/`0` threshold comparisons through `OperatorSemantics`. Callback
  membership variants also require source operator facts: JS-like strict
  equality/inequality can enter exact matching, while loose equality,
  `instanceof`, and non-JS equality surfaces stay closed for these contracts.
  Callers still prove the receiver and lambda equality shape before exact
  normalization/detection accepts them.
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
  free-function builtin contract. Same-named functions from other languages,
  including JS `min(...)`, and locally shadowed Python names stay exact-closed.
- JS-like `typeof` exact-safety now consumes a language- and arity-constrained
  operator contract. A same-named function from another language or unresolved
  provider is not treated as the JS operator.
- JS-like `Array.isArray(...)` exact-safety now consumes a static-global method
  contract and requires the `Array` global to be unshadowed.
- JS-like record-shape guards that use `Boolean(value)` as the non-null/truthy
  clause consume a static-global function contract and require the `Boolean`
  global to be unshadowed. `value !== null` and `!!value` remain available when
  their own clauses prove the same record shape.
- JS-like `undefined` is no longer frontend-collapsed to null unconditionally.
  It is preserved as a name and only treated as the nullish sentinel through an
  unshadowed-global contract. Value-graph defaulting and strict exact-safe gates
  consume that same proof, so temp-bound `Map.get(...)` defaulting can stay open
  without admitting shadowed `undefined` bindings.
- Go literal map default lookup is represented by a shared contract for the
  `composite_literal`/`keyed_element` surface and the supported zero-default
  payload classes. Go `composite_literal` no longer falls back to a generic
  collection sequence tag; it is consumed only by the Go map contract or left as
  a distinct surface.
- Static JS-like `indexOf`/`findIndex` membership requires a receiver whose
  sequence surface has membership-collection admission. Untagged sequence
  expressions, destructuring surfaces, and other positional groupings do not
  become static array membership merely because their children are literals.
- JS/TS object literals preserve static property keys in exact map/object
  semantics, but computed property names are exact-closed until a future
  contract can prove key evaluation, coercion, order, and side-effect behavior.
- JS/TS `new Map(...)` and `new Set(...)` now require construct-syntax source
  facts distinct from ordinary calls plus an unshadowed `Map`/`Set` global. With
  exact-safe static collection or entry arguments they can enter exact matching,
  including supported immutable module-level Set/Map bindings. Plain
  `Set(...)`/`Map(...)` calls and locally shadowed constructor names remain
  exact-closed.
- Static import proof facts now have a typed `ImportFactKind`/`ImportFact`
  facade in `nose-semantics`. First-party frontends emit import binding and
  namespace facts through that contract, and imported literal replacement,
  normalize idiom admission, value-graph import proof, and strict exact gates
  parse import proof RHS nodes through the shared helper instead of local raw
  tag checks.
- TypeScript `import type ...` and type-only named import specifiers are erased
  for runtime import proof; they remain unavailable to exact semantic library
  contracts.
- Strict exact collection-membership gates no longer treat any strict-safe
  expression as collection evidence. Non-literal receivers must now be proven by
  typed parameter facts, local collection/map binding facts, or module-level
  immutable collection/map binding facts.

## Scattered semantic knowledge

Semantic knowledge still appears in several forms outside the facade:

- direct `Lang` checks and local recognizers in strict exact gates and value-graph
  rules that have not yet been expressed as shared contracts;
- source operator provenance now exists for selected JS/TS and Python
  equality-shaped surfaces, but consumption is limited to narrow contracts such
  as JS-like static membership callbacks. General equality dispatch, report
  provenance, and external pack manifests remain open;
- language-specific import or module proof mechanics that are still local to
  frontend, normalize, or detect callers;
- IL still stores import facts as `Seq("import_binding")` /
  `Seq("import_namespace")` payloads for compatibility. Frontends also emit
  `EvidenceRecord::Import`, and semantic interpretation flows through typed
  `ImportFact` helpers in `nose-semantics`, but the raw IL storage shape has not
  been removed;
- module/import proof logic for immutable sibling-module literal bindings;
- type facts and coarse type inference used to gate numeric and collection laws;
- named value-graph rule modules that still consume internal `Builder` facts
  instead of versioned `LawPack` records;
- hard-coded oracle evaluation rules for eager calls, short-circuit operators,
  HOFs, nullish defaulting, recursion, and effect traces;
- duplicated receiver/domain and library/API proof gates in desugaring,
  idiom lowering, value-graph, and strict exact paths.

These are valuable, but they do not yet share one complete semantic contract
language.

## Current strengths

- Exact matching is conservative by design.
- The value graph already separates behavioral fingerprints from fuzzy candidate
  structure.
- The oracle models return values, ordered effects, final field state, `Err`
  behavior, short-circuit `and`/`or`, `any`/`all`, HOFs, recursion, and selected
  interprocedural calls.
- Proof-sensitive normalization already has named rule modules and a Lean
  obligation registry.
- Raw-node coverage gives a practical measure of lowering gaps.
- Convergence tests and hard negatives catch many semantic boundary mistakes.

## Current limits

- Language semantics are not first-class. Many rules ask "which language is this?"
  instead of "which semantic capability has been proven?"
- Library semantics are embedded in engine code rather than declared as versioned
  API contracts.
- Evaluation strategy is not a shared model. Eager, short-circuit, pull-lazy,
  call-by-need, async, and observable behavior are not represented by a common
  demand/effect abstraction.
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

## Known migration targets

The first high-value targets for semantic-kernel extraction are:

- pack-facing field/place evidence for all field reads and writes, building on
  the receiver-aware value-graph field state now used for same-unit caching;
- full import/module fact migration to remove the remaining raw
  `Seq("import_binding")` and `Seq("import_namespace")` compatibility payloads;
- richer sequence/aggregate evidence for factories, nested entries, iterator
  views, and exported-literal eligibility beyond the current first
  `SequenceSurface` substrate;
- dependency, scope, and ambiguity validation before evidence records become a
  stable external extension surface;
- resolved symbol facts for Java/Rust stdlib factories instead of the current
  path/name plus shadow-proof contracts;
- nested collection element proofs for iterator chains and builder convergence;
- Promise/future/thenable receiver facts;
- receiver/protocol evidence records beyond parameter-domain facts, including
  exact collection/map/set/option/string/integer receiver proofs, immutable
  local/module bindings, and mutation exclusion;
- demand/protocol contracts that distinguish eager arrays, lazy iterators,
  streams, callbacks, futures/promises, and call-by-need thunks;
- demand/error contracts for language-core oracle behavior such as non-iterable
  `for`/`foreach` evaluation;
- LawPack-facing ids for named value-graph rules, with the existing formal
  obligation metadata kept as the first-party proof boundary;
- module export visibility, path resolution, and mutation proof;
- provenance fields in scan JSON for pack id, contract id, law id, and evidence
  status.
