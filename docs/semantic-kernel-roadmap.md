# Semantic kernel roadmap

Back to [semantic-kernel](semantic-kernel.md). Current code shape is recorded in
[semantic-kernel-snapshot](semantic-kernel-snapshot.md). The post-PR #147 audit
of remaining raw/local semantic pockets is in
[semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md). The
provider-facing v0 extension API is in
[semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md). The
closeout for the #109 semantic-kernel migration is in
[semantic-kernel-tranche-closeout-2026-06-09](semantic-kernel-tranche-closeout-2026-06-09.md).

This page tracks decisions, history, and remaining work for the semantic kernel
and pack ecosystem.

## Decisions

1. **All language and library semantics should eventually enter through packs.**
   First-party languages are not special at the API boundary; they may be compiled
   into nose, but they should use the same pack contracts as external languages.

2. **nose certifies only first-party packs.** External pack providers own their
   semantic claims. Users own the decision to enable them. nose owns the
   extension contract, schema and structural validation, fail-closed execution,
   and provenance reporting. Semantic correctness, conformance evidence, and
   enablement risk for external packs stay with the provider and user, except
   for first-party/default packs that nose ships and tests.

3. **Packs emit evidence, not verdicts.** A pack can emit facts, contracts, and
   protocol operations. It cannot mint fingerprints, bypass laws, or approve exact
   clones.

4. **Language core and library semantics are separate layers.** Evaluation order,
   truthiness, overloadability, and exception behavior belong to language core.
   `sum`, `Iterator::map`, `Array.prototype.map`, RxJS `Observable.map`, and NumPy
   vector operations belong to stdlib/library packs mapped onto protocols.

5. **Demand and effect are first-class.** Lazy evaluation, short-circuiting,
   iterators, async/futures, and observables cannot be accurately modeled with a
   simple purity flag. Exact laws need demand and effect preconditions.

6. **Unknown stays fail-closed.** Missing type, receiver, symbol, version,
   shadowing, or effect evidence must block exact semantic acceptance. It may
   still inform `near` scoring.

7. **Selectors are not proof.** A function or method name is only a selector.
   Exact contracts must also declare and check the language, symbol/namespace,
   arity, receiver/protocol, shadowing, import, version, overload, demand, and
   effect obligations that make that selector mean the claimed operation.

8. **Source facts are evidence, not semantics.** Source-origin facts preserve
   distinctions that the shared IL erases, such as construct syntax, literal
   surface, and equality/operator family. They can feed exact contracts only
   through kernel-defined fact kinds and contract preconditions; they do not mint
   fingerprints or approve equivalence directly.

## History

- The original architecture lowered every supported language into one shared IL,
  then normalized toward common fingerprints.
- The value graph became the behavioral fingerprint substrate, separating exact
  semantic matching from fuzzy structural candidate generation.
- The independent interpreter oracle was added to test fingerprint-equal units
  against concrete behavior and catch behavior-changing canonicalizations.
- Lean proof obligations were added for proof-sensitive rules.
- Exact fragments gained explicit contracts, effect classifications, and
  fail-closed receiver/place boundaries.
- Dogfooding surfaced repeated per-language frontend shapes; safe common helpers
  moved into `lower.rs`, while grammar-specific parallelism remained explicit.
- Documentation review in PR #89 clarified the current limits: exact Type-4 is a
  modeled subset, not arbitrary semantic equivalence.
- The semantic-kernel direction was chosen to make language and library semantics
  an explicit extension boundary rather than scattered engine code.
- The first internal facade landed as `nose-semantics`, wrapping first-party
  language/profile predicates and API contracts while the rest of the pipeline is
  migrated.
- Name-only contracts were narrowed: JS/TS `Map`/`Set` constructors, JS-like
  `.then`, untyped JS collection methods, and Rust iterator adapters now require
  explicit proof or remain exact-closed.
- Additional call surfaces moved behind proof-gated contracts: JS/TS
  `filter(...).length`, Rust `get(key).is_some()`, Java `keySet().contains`,
  and Java `Stream.count()` require receiver/protocol or map proof. Ruby untyped
  `Enumerable` surfaces, including `.each`/`.each_with_index` block loops, and
  scalar/array numeric helpers remain closed until comparable proof facts exist.
- New value-graph rewrites began moving into named `rules/*` modules with
  mechanical formal-obligation pairing; `clamp` is the current proof-backed
  example.
- The common parameter-type substring recognizer moved behind first-party,
  language-scoped type-domain contracts in `nose-semantics`. Frontends now emit
  parameter `Domain` evidence from those contracts, while imported Python
  `typing`, `collections.abc`, and `asyncio` aliases carry `ImportedBinding`
  dependencies and rebound aliases close the path. `ParamSemantic` remains a
  compatibility vocabulary in tests and lower-level helpers, not the producer
  boundary for newly emitted parameter-domain facts.
- The first first-party pack pilot moved Python stdlib type-domain aliases from
  a raw helper table into a pack-shaped contract row set. Imported
  `typing`, `collections.abc`, and `asyncio` alias-derived `Domain` evidence now
  reports `nose.python.stdlib.type_domain` provenance while preserving
  shadow/rebind hard negatives and metadata-only behavior for local external
  manifests.
- Rust scalar integer methods (`abs`, `min`, `max`, `clamp`) now consume a
  language-, signature-, and integer-domain-constrained first-party contract
  instead of a bare method-name recognizer. Float/NaN-sensitive methods remain a
  separate future contract.
- Exact fragment IL-surface proofs for Java `this.field`, Java `return this`,
  non-overloadable C/Go/Java index assignment, and single-item builder append
  calls moved into `nose-semantics`, so predicate and contract paths no longer
  duplicate those language/API gates.
- The first receiver-domain evidence facade landed as `DomainEvidence`.
  Parameter type domains, selected library/API result domains, and inferred
  immutable binding domains now feed the same kernel-facing domain vocabulary so
  pack-provided evidence can replace first-party producers later without adding
  new consumer paths.
- Rust stdlib path contracts for `Some`/`Option::Some`,
  `None`/`Option::None`, `Option::and_then`, and `Vec::new` moved into the
  kernel facade with explicit shadow-root obligations. The caller still proves
  local shadow safety, and the Rust frontend preserves `if let` pattern tests
  instead of lowering `Some`/`None` directly to null/not-null before that proof.
- Java collection/map factory selectors, Python free-name/imported collection
  factories, Rust std collection/map factory paths, Ruby `Set.new`, and JS-like
  `new Map`/`new Set` moved behind internal `LibraryApiContract` rows in
  `nose-semantics`. Normalize and strict exact gates now consume the same API
  identity/result source while keeping local import, require, shadow, mutation,
  constructor-syntax, and entry-shape proof at the caller.
- Java empty `ArrayList`/`LinkedList` constructor lowering now consumes a
  `LibraryApiContract` `java.util` constructor row instead of a raw simple-name
  check. Simple names need import proof and no local type shadow before they can
  seed exact builder-loop equivalence.
- Membership and map-key membership recognition now uses language-scoped method
  contracts before normalization or strict exact matching assigns containment
  semantics. This intentionally closes old name-only paths such as JavaScript
  `.contains(...)`, which had no first-party JS membership contract.
- Java stream source adapters are now proof-gated: receiver `.stream()` requires
  exact iterable evidence, and static `Arrays.stream(xs)` requires the
  `java.util.Arrays` import binding with no local `Arrays` type shadow.
- Cross-file immutable import replacement now copies the provider's closed
  evidence subgraph required by the exported literal expression, preserving
  provider-side stdlib proofs such as `java.util.Map` for Java static imports
  only when that provider evidence exists. Copied provider nodes/evidence keep
  provider source-origin spans while dependency ids are rewired in the importer,
  so importer-local declarations do not shadow provider-proven API occurrences.
  Replacement records `ImportedLiteralSnapshot` provenance depending on the
  importer static import proof and copied provider evidence. Static import
  identity now requires `EvidenceRecord::Import`; frontends keep only untagged
  coordinate sequences in the assignment carrier, and raw sequence spelling no
  longer proves cross-file replacement or value-graph import identity.
- JS-like `Map`/`Set` constructor contracts now require construct-syntax proof.
  They were initially closed while construct-vs-call evidence was missing; the
  source-fact slice reopened proof-backed `new Map(...)`/`new Set(...)` while
  plain `Map(...)`/`Set(...)` calls stayed closed.
- Map key-view recognition moved behind contracts that distinguish collection
  views from iterator views. JS-like `Map.keys()` now requires an
  `Array.from(...)` wrapper before exact membership can consume it.
- Go composite map literal/default-zero lookup recognition moved behind shared
  contracts for the outer literal surface, per-entry surface, and supported
  zero-default payload classes.
- Map `get(key)` lookup surfaces for Java, Rust, and JS-like typed/proven maps
  moved behind an explicit map-get contract. Defaulting surfaces continue through
  the existing `GetOrDefault` method contract.
- JS-like static array `indexOf`/`findIndex` membership and their accepted
  threshold comparisons moved behind shared semantic contracts.
- Channel eligibility and pack trust were split: first-party/default status is
  provenance and enablement policy, not a semantic channel.
- Newly migrated selector contracts started carrying explicit receiver/proof
  requirements so extension APIs do not look like name-only semantic guesses.
- Python `math.prod` product-reduction recognition moved behind an imported
  namespace function contract with missing-import and overwritten-binding hard
  negatives.
- Java `Math.abs`/`Math.min`/`Math.max` moved out of frontend text-only lowering
  and into method contracts that require an unshadowed `Math` receiver.
- JS-like `undefined` moved from unconditional frontend null lowering to an
  unshadowed-global nullish contract, preserving shadowed binding hard negatives.
- Strict exact gates now consume the same nullish-global proof, so temp-bound
  JS/TS `Map.get(...)` defaulting remains exact-eligible only when `undefined`
  is the unshadowed JS-like sentinel.
- Strict exact call gates for JS-like `typeof` and `Array.isArray(...)` moved
  behind language/arity/global-shadow contracts. Regex literal `.test(...)` now
  consumes regex-literal source provenance, while ordinary string `.test(...)`
  and same-named method calls remain closed. This closes raw-name bypasses found
  after PR #101.
- Normalize idiom receiver admission for iterator identity adapters and Rust
  `zip` now consumes the same semantic contracts as value-graph/detect paths,
  closing language-blind `iter`/`zip` selector bypasses.
- JS-like `Math.abs`/`Math.min`/`Math.max` now consume method contracts with an
  unshadowed `Math` receiver, and JS record-shape guards using `Boolean(...)`
  consume a static-global function contract with an unshadowed `Boolean`
  requirement.
- Generic Python/Go free-function builtins now have `LibraryApi` occurrence
  rows. Early idiom canonicalization and value-graph two-argument
  `min(...)`/`max(...)` require admitted occurrence evidence instead of raw
  callee spelling, closing unqualified JS `min(...)`, local-shadowing, and
  missing-producer bypasses.
- Ruby `fetch(key) { fallback }` map-default handling now consumes an explicit
  zero-arg-lambda fallback argument contract, and Go `slices.Contains` value-graph
  membership proof consumes the imported namespace carried by the method contract
  instead of spelling the namespace locally.
- Imported immutable literal replacement and exact module-binding gates now share
  stronger mutation evidence for top-level place writes such as
  `LOOKUP[key] = value`, closing importer-side direct-write false exact cases.
- Value-graph and oracle field state no longer treat raw field spelling as place
  proof. The admitted same-unit substrate is Java `this.field`, backed by
  `Place(SelfReceiver)`, `Place(SelfField)`, and `Effect(SelfFieldWrite)`.
  Raw dynamic attribute/property writes remain ordered or unsupported until a
  pack supplies explicit place/effect evidence.
- Lowered `Seq` surface admission now goes through `SeqSurfaceContract` instead
  of local raw-string allowlists. The contract separates exact-tree safety,
  membership collection admission, map-entry-list admission, imported-literal
  eligibility, and value-graph tags, so Go `composite_literal` map surfaces no
  longer leak into generic collection semantics. Untagged `Seq` is now
  non-semantic by default; static membership and idiom receiver gates consume
  explicit membership-collection surface proof.
- JS/TS object surfaces were narrowed: static property keys remain exact
  map/object entries, computed property names are exact-closed until key
  evaluation semantics are contracted, and object `.length` no longer lowers to
  collection `Len` merely because the receiver is a `Seq`.
- Java `java.util.*` wildcard proof for empty `ArrayList`/`LinkedList`
  constructors now closes when another package explicitly imports the same
  simple type, matching Java import resolution before the constructor surface can
  enter the collection builder contract.
- Same-unit value-graph and oracle field readback/final field state now consume
  the self-field place/effect evidence boundary rather than arbitrary evaluated
  receiver shape, so aliases, Python-style dynamic attributes, property
  setters, and computed call-result receivers stay exact-closed until
  pack-facing place evidence exists.
- Import binding and namespace proof interpretation now goes through a typed
  `ImportFactKind`/`ImportFact` facade in `nose-semantics`. Frontend emitters,
  imported immutable literal replacement, normalize idiom gates, value-graph
  import proof, and strict exact gates initially moved behind that shared facade
  instead of parsing raw import `Seq` tags locally.
- Imported immutable literal replacement now consumes evidence-only import facts,
  copies provider evidence with preserved source-origin anchors and rewired
  dependency ids, and records `ImportedLiteralSnapshot` provenance. This closes
  raw import-tag fallback and missing-provider-proof cases such as Java
  `Map.of(...)` without `import java.util.Map`.
- TypeScript type-only imports no longer emit runtime import facts: whole
  `import type ...` declarations and type-only named specifiers stay outside
  exact library/API proof.
- Imported literal provenance now treats provider-side opaque argument escapes
  such as `mutate(LOOKUP)` as mutation risk, so exported bindings must be direct,
  unescaped immutable values before cross-file replacement can copy them.
- Strict exact collection-membership receiver proof no longer falls back from
  "not a known collection surface" to "any strict-safe tree." Top-level immutable
  collection and map bindings are tracked separately from generic immutable names,
  preserving supported module-level collection cases while closing unproven
  receiver expressions.
- Exact fragment append-effect recognition now consumes canonical append evidence
  instead of raw method selectors. Untyped `push`/`append`/`add` calls no longer
  prove append fragments by name; first-party language/library paths must first
  prove the receiver or active-builder contract and lower the call to
  `Builtin::Append`.
- The first demand/effect contract module now names the currently supported
  builtin, HOF, source protocol, and Promise-continuation profiles: eager builtin
  calls, explicit reductions, short-circuit quantifiers, append mutation,
  nullish defaulting, per-element callback demand for
  map/flat-map/filter-map/filter/reduce, pull-lazy Python generator expressions,
  eager JS-like/Ruby library HOFs, pull-lazy Rust iterator/Java Stream HOFs,
  async continuation boundaries, generator suspension, channel boundaries, and
  non-channel protocol boundaries. The oracle consumes those profiles for
  admitted builtins instead of matching local demand enums; value-graph HOF
  callback exception timing, HOF materialization gates, strict-exact HOF gates,
  and Promise `.then` beta-reduction also read shared profiles. API admission
  and receiver/protocol proof remain evidence/contract-row gated; raw HOF
  payloads, unsupported source HOFs, selector-only calls, and broken API evidence
  stay closed.
- Primitive operator gates now enter through `OperatorSemantics` contracts for
  comparison transforms, comparison laws, cardinality thresholds, static
  `indexOf`/`findIndex` thresholds, and source membership operators. Algebra,
  CFG normalization, value-graph comparison/count rewrites, and strict exact
  static-membership gates consume the shared contract vocabulary. JS `in` no
  longer inherits collection-membership exact safety from the shared `Op::In`
  token; only Python `in` currently has a first-party membership-operator
  contract.
- Source facts landed for construct syntax, regex literals, and selected
  equality/operator provenance. Exact consumers now reopen proof-backed JS-like
  `new Map(...)`/`new Set(...)`, regex literal `.test(...)`, and strict JS-like
  static membership callbacks while closing plain constructor calls, string
  `.test(...)`, loose equality, and `instanceof` for those exact contracts.
- The first shared `EvidenceRecord` substrate landed for source, domain, import,
  and sequence-surface facts. First-party frontends now mirror compatibility
  `SourceFact`, `ParamTypeFact`, raw import `Seq`, and lowered `Seq` surface
  facts into records with ids, anchors, provenance, dependencies, and status.
  `nose-semantics` lookups fail closed on ambiguous/conflicting evidence before
  falling back to compatibility storage.
- Source-origin and parameter-domain proof later became evidence-only: the
  `SourceFact` and `ParamTypeFact` side-table mirrors were removed from IL
  storage, first-party frontends emit `Source` and `Domain` records directly, and
  semantic lookups no longer reopen those proof paths from compatibility mirrors
  when evidence is missing.
- Symbol-identity evidence now represents static imported binding/namespace
  aliases. Normalize idiom admission, value-graph namespace fallbacks, and strict
  exact gates have started consuming this helper layer instead of each re-scanning
  raw import assignment shapes. Provider/imported immutable literal replacement
  also now rejects direct module-binding mutations such as `LOOKUP.push(...)`.
- JS/TS static-global value occurrences now emit `UnshadowedGlobal` evidence for
  first-party globals such as `Math`, `console`, `Array`, `Map`, `Set`, and
  `undefined` when no local shadow is proven. JS/TS `Math.*` no longer lowers
  directly to builtins in the frontend; normalize consumes the preserved
  `Field(Var(global), method)` shape through symbol-proof contracts instead.
- Selected JS/TS qualified static global paths now emit `QualifiedGlobal`
  evidence. `Object.hasOwn` and
  `Object.prototype.hasOwnProperty.call` are dependencies of own-property guard
  evidence, while `Array.from` gates JS-like map-key iterator wrappers.
  `Array.isArray` emits the same path evidence for strict exact call gates. These
  qualified path records now depend on same-span `UnshadowedGlobal` root proof,
  so consumers no longer accept detached path evidence without the root identity
  proof. Full namespace-member resolution remains open.
- Value-graph import identity now consumes sequence `Import` evidence into
  dedicated internal `ImportNamespace`/`ImportBinding` value ops instead of
  treating raw `ValOp::Seq(import_*)` shapes as proof objects. Imported
  binding/namespace symbol helpers also no longer accept raw import assignment
  RHS parsing as an exact proof fallback.
- JS/TS record-shape guards now emit dedicated `Guard::JsRecordShape` evidence
  with subject, null/truthiness, equality-form, and API-dependency obligations.
  Strict exact and value-graph paths require that evidence plus
  `SequenceSurface(RecordGuard)`, so raw `Seq("record_guard")` no longer acts as
  a proof object by tag spelling.
- JS/TS own-property guards now emit dedicated `Guard::JsOwnProperty` evidence
  with an asserted supported `QualifiedGlobal` API dependency. Strict exact and
  value-graph map-default paths require that evidence plus
  `SequenceSurface(OwnPropertyGuard)`, so raw `Seq("own_property_guard")` no
  longer acts as proof by tag spelling or API-looking text.
- Go zero-map literal/default lookup now requires evidence for both
  `SequenceSurface(GoCompositeMapLiteral)` and `SequenceSurface(GoMapEntry)`.
  The compatibility tags still exist as lowered surfaces, but exact admission no
  longer comes from raw `composite_literal`/`keyed_element` strings alone.
- Non-factory library/API surfaces started moving into `LibraryApiContract`
  identity/result rows. Map-key views and wrappers, map `get`/defaulting method
  calls, selected static JS-like helpers, regex-literal `.test`, Python
  `math.prod`, promise `.then`, iterator identity adapters, Java
  `Arrays.stream`, and existing language-scoped method-call gates now share the
  same API-contract source across normalize, value-graph, and strict exact
  consumers.
- The first `LibraryApi` occurrence evidence vertical landed for selected
  JS-like static/global APIs. First-party lowering emits dependency-backed call
  evidence for `Array.from`, `Array.isArray`, `Boolean`, `new Map`, and
  `new Set`; value-graph and strict exact consumers for those surfaces consult
  it first and close legacy fallback on conflicting, ambiguous, or
  dependency-broken records.
- The next `LibraryApi` occurrence evidence slice extended the same
  dependency-backed path to selected import/source-backed APIs: Python
  `collections.deque`, Python `math.prod`, Java `java.util` static
  factories/adapters (`List.of`, `Set.of`, `Arrays.asList`, `Map.of`,
  `Map.ofEntries`, `Map.entry`, `Arrays.stream`), and JS-like regex-literal
  `.test`. Producers emit call-site `Symbol` dependencies for imported
  binding/namespace occurrences or `Source` dependencies for regex literals;
  value-graph, idiom, and strict exact consumers consult these records first and
  close fallback on rejected records. Imported occurrence symbols now require
  binding-anchor dependencies, rebinding/local-shadow validation, span-matched
  dependencies when spans survive normalization, and Java map provider proofs no
  longer replace current receiver identity except for imported literal snapshots
  already validated in the provider module.
- The follow-up LibraryApi fallback-closure slice made those producer-covered
  surfaces require admitted occurrence evidence. Missing `LibraryApi` evidence
  now closes value-graph, idiom, strict exact, and Java map provider snapshot
  paths for JS-like static/global APIs, Python imported `collections.deque`,
  Python `math.prod`, Java `java.util` static factories/adapters, and JS-like
  regex-literal `.test`. The older import/symbol/source facts remain
  dependencies, not fallback API-identity proofs. Python aliased imports such as
  `from collections import deque as Values; Values(...)` are preserved by
  resolving the occurrence through imported-binding evidence rather than by
  comparing the local name to the exported API name.
- The same fallback-closure slice extended occurrence evidence to selected
  free-name and require-backed factories: Python builtin collection factories,
  Rust `vec!`, `Vec::new`, and selected `std::collections::*::from` factories,
  plus Ruby `require "set"; Set.new(...)`. First-party lowering now emits
  `UnshadowedGlobal`, macro-invocation `Source`, or earlier top-level
  `Import::Require` dependencies for those occurrences, and value-graph, idiom,
  strict exact, and provider snapshot consumers require admitted `LibraryApi`
  evidence instead of raw selector/path/require scans.
- Receiver-domain proof consumption moved behind a shared `DomainRequirement`
  resolver in `nose-semantics`. `Domain` evidence can now be consumed at exact
  receiver node anchors before scoped parameter compatibility evidence, and
  ambiguous/conflicting/dependency-broken receiver facts close fallback.
  Desugaring, normalize idiom canonicalization, value-graph membership,
  property, map, and integer gates, and strict exact receiver gates now share
  this resolver instead of each re-scanning parameter ids or names locally.
  `MethodReceiverContract` exposes the subset of receiver obligations that are
  domain-backed, while imported namespace, unshadowed global, map-literal,
  demand, and effect obligations remain separate checks.
- The type/domain expansion slice broadened `DomainEvidence` and
  `DomainRequirement` beyond the initial container/scalar set. The vocabulary now
  includes iterable/iterator, record, result, future-like, boolean, float, and
  nominal domains; `Type(NominalDomain)` rows can tie provider-proven nominal
  type identities to domains without letting raw type names prove semantics.
  The value-law bridge remains narrow, so these richer facts do not automatically
  become sequence or exact algebraic proof.
- Selected first-party library/API factory result domains now produce
  node-anchored `Domain` evidence after the call occurrence has admitted
  `LibraryApi` evidence. This covers Python builtin/imported collection
  factories, Rust `Vec::new`/`vec!`/selected `std::collections::*::from`
  factories, Ruby `Set.new`, Java `List.of`/`Set.of`/zero- or multi-argument
  `Arrays.asList`/`Map.of`/`Map.ofEntries`, JS-like `new Set`/`new Map`, and
  JS-like one-argument `Array.from`. The mapping is contract-scoped and
  deliberately excludes lookalikes, Java single-argument `Arrays.asList(x)`
  without element-provenance proof, and non-container results such as
  `Map.entry`, `Array.isArray`, `Boolean`, regex `.test`, `math.prod`,
  `Arrays.stream`, map `get`, promise `.then`, iterator adapters, and generic
  method contracts.
- Immutable local/module binding domains now produce binding-anchored `Domain`
  evidence during normalization when the initializer has asserted sequence or
  result-domain evidence, the binding is single-assignment in the current scope,
  and the first-party mutation scan finds no direct binding/place mutation.
  `nose-semantics` resolves receiver-domain proof from exact receiver nodes,
  binding anchors, and scoped parameters through the same `DomainRequirement`
  helper, so value-graph and strict exact gates no longer maintain separate
  receiver-domain scanners.
- Strict exact receiver proof now consumes the shared
  `ReceiverDomainEvidenceIndex` instead of raw collection/map name and CID
  side tables. Binding-domain evidence remains receiver-domain proof only: it
  no longer promotes an opaque initializer into an exact-safe variable value, and
  binding proofs apply only when visible at the receiver use site.
- The receiver-method `LibraryApi` occurrence slice moved broad method-family
  consumers behind dependency-backed call occurrence records. First-party
  lowering now emits occurrence evidence for map `get`, map-key views, iterator
  identity adapters, and language-scoped method-call contracts only when the
  exact language/method/arity row and receiver proof are present. Normalize runs
  receiver-method refresh passes after immutable binding-domain inference and
  after final CFG/dataflow/algebra rewrites, so binding receivers such as
  `VALUES.contains(x)` can depend on the current binding or sequence-domain
  proof produced from `VALUES = List.of(...)`. Source-span evidence lookup
  re-checks the recovered source `Call` node when value-graph CSE has collapsed
  parameter receivers into spanless values, which keeps Java/TS/Python map
  defaulting aligned without accepting selector-only proof. Normalize idioms,
  value-graph rewrites, and strict exact gates for collection/map membership,
  map defaulting, map-key views, iterator adapters, Rust `zip`, and
  HOF/reduction methods now require admitted occurrence evidence instead of raw
  selector plus receiver-domain scans. Normalized `HoF` nodes produced from
  admitted method calls also remain admissible protocol receivers through their
  same-span `MethodCall(HoF(...))` occurrence record, so downstream adapters can
  consume canonicalized HOFs without trusting selector spelling alone.
- The static API occurrence slice moved Java empty collection constructors and
  JS-like static `indexOf`/`findIndex` membership behind the same
  dependency-backed occurrence boundary. `new ArrayList<>()`/
  `new LinkedList<>()` now stay as construct `Call` nodes until exact or
  wildcard `java.util` import proof admits the `LibraryApi` record; explicit
  same-name imports and local type declarations close wildcard proof. Static
  index membership now emits `LibraryApi` evidence that depends on the exact
  receiver `SequenceSurface(Collection)` fact, and value-graph/strict exact
  consumers require the admitted occurrence instead of trusting method spelling
  plus literal children. Raw `Op::In` value-graph canonicalization now also
  checks the language membership-operator contract before treating the operator
  as collection membership.
- Value-graph and structural-recursion domain gates moved from normalize-local
  `types.rs` / `Ty` inference to `nose-semantics` `ValueDomain` and `ValueLaw`
  contracts. The first contract set covers add non-concat ordering,
  numeric/boolean law preconditions, factor distribution, large formula
  compaction, and structural numeric folds. Parameter `Domain` evidence now
  feeds the shared value-domain seed for direct functions, class/container
  method fingerprints, and structural-recursion recognition, so typed
  string/sequence concatenation no longer inherits optimistic numeric add
  ordering.
- An experimental `abstraction` scan mode landed as a weak sibling claim over a
  narrow `near` subset. It emits typed literal-hole witnesses and caveats for
  refactoring-template candidates, but does not feed `semantic`, `verify`, or exact
  kernel admission.
- The abstraction witness policy is now separated from unit feature extraction as
  a small internal witness kernel. The current accepted hole remains literal-only,
  but the model records claim class, family evidence basis, checked member count,
  template format, hole role, template index, and observed literal classes so future
  type/domain/operator witnesses have a single owner.
- Abstraction scan output now requires family-wide hole agreement: every reported
  family member must fit the same normalized IL template with the same literal-leaf
  hole position. Mixed connected components are not given a weak witness merely
  because one representative pair looked actionable.
- Exact-fragment place/effect gates became evidence-authoritative for the
  producer-covered substrate. First-party normalize refreshes now upsert
  `Effect(BuilderAppendCall)` for canonical append calls only when a same-span
  append `LibraryApi` proof licenses the canonical form,
  `Effect(NonOverloadableIndexWrite)` for C/Go/Java index assignments, and Java
  self receiver/field/write `Place`/`Effect` records after canonical rewrites.
  Exact fragment consumers no longer reopen append/index/self-field admission
  through language/shape fallback when `Effect`/`Place` evidence is missing.
- Sequence-surface exact/value consumers became evidence-only. Raw
  `Seq("array")`, `Seq("object")`, `Seq("tuple")`, Go `composite_literal`, and
  similar lowered tags no longer prove exact-tree safety, membership collection
  admission, map-entry-list shape, or value-graph sequence tags without matching
  `SequenceSurface` evidence. JS/TS `filter(...).length` also now requires the
  inner HOF call's admitted `LibraryApi` occurrence instead of a raw method
  selector. Raw Python async-looking field names no longer rewrite to sync names
  until an explicit async/sync protocol evidence path exists.
- JS/TS, Python, and Rust `await` expressions now preserve a raw async protocol
  boundary and emit `Source::Protocol(Await)` evidence instead of lowering
  directly to the operand. JS/TS and Python `yield` expressions preserve raw
  generator protocol boundaries with `Source::Protocol(Yield)`. Rust `async {}`
  and `?` also preserve raw protocol boundaries with
  `Source::Protocol(AsyncBlock)` and
  `Source::Protocol(TryPropagation)`. This closes the old exact async/sync and
  error-propagation convergence paths, plus generator/body erasure, until
  language/runtime-specific protocol contracts can prove receiver, demand,
  scheduling, suspension, exception, and effect obligations.
- Go concurrency/channel surfaces now preserve source-backed protocol
  boundaries. `go`, `defer`, channel send, channel receive, receive-status
  projection, `select`, and select cases/defaults no longer erase to ordinary
  calls, operands, or ad hoc sequence tags. Exact/value consumers stay closed
  until channel/goroutine/defer/select contracts can prove scheduling, blocking,
  close/zero-value, case-selection, demand, and effect obligations.
- Python comprehension lowering now emits source facts for list/set/dict
  comprehensions and generator expressions. Exact HOF admission consumes those
  facts: list/dict materialized surfaces preserve existing positive recall where
  modeled, returned generator/set surfaces stay closed, `len(generator)` and
  set-comprehension cardinality stay closed, and supported terminal reductions
  reopen generator/list streams only under immediate consumer demand.
- The protocol/API occurrence closure slice extended `LibraryApi` beyond
  call-only APIs. JS/TS/Java `length` property reads now require a
  `PropertyBuiltin` occurrence anchored to the `Field` node, JS-like `length()`
  is no longer a cardinality method contract, Rust `Some(...)`, `Some(_)`
  pattern selectors, and bare `None` now emit contract-backed Option occurrence
  evidence, Rust `Option::and_then` and scalar integer methods require admitted
  receiver-method occurrences, and value-graph/desugar/idiom consumers fail
  closed when those occurrence records are missing, rejected, or
  dependency-broken.
- Rust range and Option-pattern recognition moved off raw IL shapes. Rust
  half-open/inclusive range expressions and tuple-struct single-wildcard
  patterns are now `Source` evidence. The `0..len(collection)` full-index range
  path requires the half-open source fact plus admitted `len` semantics, and
  `Some(_)` presence predicates require both the admitted `Some` selector
  occurrence and Rust wildcard-pattern source proof.
- Builder and mutation safety moved further onto the evidence substrate.
  First-party producers now emit exact append/index/self-field effect evidence
  separately from conservative binding-write, receiver-mutation, and
  opaque-argument-escape risk evidence. Module facts, binding-domain inference,
  imported literal replacement, imported binding use indexing, value-graph
  mutation safety, and exact-fragment context blocking consume shared
  `nose-semantics` helpers instead of each re-scanning raw assignment shapes,
  method selectors, or call arguments. Receiver-mutation production is scoped by
  language-specific first-party mutator policy, and those records only close
  risky exact paths; they do not prove a same-named API's exact semantics.
- Active aggregate builders now require contract-backed append/write proof plus
  surface shape. Exact append evidence is still required for exact-fragment
  append effects. Value-graph list-builder contributions require either exact
  append evidence or admitted same-span append API occurrence evidence plus the
  language-scoped builder-append method-effect row; a row-only path is also
  allowed, but only under active-builder context. Map-builder recognition
  requires write evidence plus an explicit map seed. Raw selectors outside those
  rows, raw index assignment, untagged sequences, and tuple values no longer
  prove builder semantics by themselves. Python set literals now emit
  `SequenceSurface(Collection)` so supported module-set membership remains
  covered, while direct/module tuple literals stay closed until a factory, typed
  receiver, or other contract supplies membership-collection evidence.
- First-party method-effect policy moved from bool selector helpers into
  explicit contract rows. Receiver-mutation and builder-append producers now
  consume language/method/arity/effect rows, value-graph list builders consume
  effect evidence, admitted append API occurrence evidence, or active-builder
  row evidence under those rows, and Python dict-builder loops consume a
  separate map-builder index-write row plus `Effect(BindingWrite)` and an
  explicit map seed. JS-like `undefined`
  value-graph nullish evaluation is now evidence-only: the frontend-proven
  `Symbol(UnshadowedGlobal("undefined"))` record is required, and raw spelling
  plus file-scope fallback no longer opens the exact nullish value path.
- JS-like `typeof` strict exact safety now requires source-operator evidence at
  the call span in addition to the language/arity/name contract, so raw
  `Call(Var("typeof"), arg)` shapes no longer prove the JS unary operator.
  Python wildcard imports now emit `Import::Wildcard` evidence; post-lower
  free-name API evidence uses that record as the ambiguity boundary instead of
  scanning a raw `python_wildcard_import` marker.
- Raw HOF value-graph admission now requires either source-comprehension proof
  for the supported Python comprehension surfaces or admitted HOF library/API
  occurrence evidence. Set comprehensions and synthetic raw `HoF` payloads stay
  closed; source-proven comprehension internals can still compose their filter
  HOFs within the proven surface. Value-graph count, reduction, and static
  membership shortcuts now reuse the same filter admission, so raw
  `HoF(Filter)` payloads cannot bypass the HOF gate by sitting under `len(...)`
  or a reduction.
- Raw canonical `Payload::Builtin` admission now goes through
  `admitted_builtin_semantics_at_call` before value-graph folding, builtin
  fallback tagging, range/len/zip/enumerate loop patterns, strict-exact builtin
  calls, function-binding safety, mutation-risk blocking, value-domain builtin
  result inference, or interpreter-oracle builtin execution can consume builtin
  semantics. Same-span `LibraryApi` occurrence evidence admits post-desugar
  library builtins, while only narrow syntax-owned language-core lowerings such
  as Go map lookup-ok `Contains`, Go `Enumerate`, Python dict-comprehension
  `DictEntry`, JS-like `Keys`, C source-proven `UnsignedCast32`, and
  effect-proven append remain raw-payload eligible.
- Rust `get(key).unwrap_or(default)` now admits the canonical map
  `GetOrDefault` builtin through the exact `unwrap_or` `LibraryApi` occurrence
  plus its admitted nested `MapGet` dependency, instead of treating
  `ValueOrDefault` selector semantics as sufficient for map defaulting.
- Raw `Seq` spelling no longer feeds value-graph sequence tags. Missing
  `SequenceSurface` or guard evidence now produces the untagged value instead of
  a spelling hash, so internal-looking payload names such as `record_guard` or
  `own_property_guard` cannot become semantic proof channels.
- Raw user-call spelling no longer proves direct recursion or in-file call
  execution. `nose-il` now has `CallTarget` evidence, the first-party normalize
  producer emits `DirectFunction` only for unique top-level in-file function
  targets with no current or enclosing lexical shadowing, and recursion,
  interpreter, value-graph pure-inline, and strict exact direct-function callee
  consumers require that occurrence proof. The follow-up call-target producer
  slice emits `ImportedFunction` and `ImportedMember` only from
  dependency-backed imported binding or imported namespace symbol proof, added a
  shared fail-closed resolver, and taught strict exact to admit imported
  function/member opaque identity only through explicit evidence. The vocabulary
  also includes `DirectMethod` and `DynamicDispatch`, but no first-party producer
  emits those records yet; direct methods still require exact receiver identity,
  and dynamic-dispatch records do not by themselves prove one concrete target.
- C byte-pack proof moved onto evidence-backed alias and cast records. Local
  typedefs and direct quote includes emit `Type(CTypeAlias)` evidence, included
  aliases depend on `Import(CQuoteInclude)`, alias-based `Domain(ByteArray)` and
  `Source(Cast(CUnsigned32))` facts depend on the type proof, and value-graph
  byte-pack laws consume a first-party C byte-pack contract instead of a bare
  language bool.
- Exact-fragment production is now contract-first: the collector uses
  `fragment::recognize::recognize_contract` as the production authority, while
  the old predicate matrix remains as a debug/differential guard until it can be
  deleted or reduced.
- Large semantic test modules were split out of the production implementation
  files while continuing this migration. `nose-semantics/src/lib.rs` and
  `nose-normalize/src/value_graph.rs` are both back under 10k lines, with their
  moved tests kept adjacent as Rust test modules. The follow-up range/pattern
  slice also split source-fact and value-graph proof/admission tests into
  focused adjacent modules so raw-shape regression cases are easier to audit.
  The LibraryApi resolver slice then split the remaining large
  `nose-semantics` test root into domain/symbol evidence, LibraryApi evidence,
  LibraryApi contract, and effect/place test modules before adding more
  occurrence-admission coverage.
- The idiom/value-graph resolver cleanup moved supported normalize idiom
  canonicalization and direct value-graph API consumers behind shared
  `nose-semantics` admitted occurrence resolvers. This covers free-function
  builtins, generic receiver-method contracts, map `get`, map-key views,
  iterator identity adapters, Java static collection adapters, Rust `Some(...)`,
  Rust map factory receiver proof, static index-membership, and Rust scalar
  integer methods where the source `Call` node is still available.
- The value-graph span-query resolver cleanup moved value-level CSE consumers
  that no longer carry a source `Call` node behind dedicated `nose-semantics`
  admitted span resolvers. Free-name/imported collection factories,
  Java/Ruby/Rust collection factories, free-name/Java map factories, Java map
  entries, map `get`, and map-key view/wrapper calls now resolve contract
  identity and `LibraryApi` occurrence evidence in one place.
- The node-level/API resolver cleanup moved property builtin field admission,
  Rust `Some` callee-node admission, HOF receiver proof in desugaring, and
  promise `.then` contract lookup behind shared admitted occurrence resolvers.
  Promise continuation semantics remained fail-closed until Promise-like
  receiver proof existed. The same cleanup preserved the separate opaque callee
  identity policy: parameter callees and proof-backed immutable/imported callees
  may be exact as opaque value calls, but they do not gain library/API semantics
  without admitted occurrence evidence.
- The Promise receiver-proof slice added `Domain(PromiseLike)` and JS-like
  `Promise.resolve` as first-party contract evidence. Admitted `.then(lambda)`
  calls can now reduce only when receiver proof is present and the settled value
  is recoverable from `Promise.resolve(non-thenable-safe value)` or a supported
  admitted `.then` chain. The value graph keeps the result behind a Promise
  boundary, so Promise-returning code does not merge with synchronous payloads.
  Arbitrary `.then` methods, custom thenables, shadowed `Promise`, unsafe
  `Promise.resolve(obj)` assimilation, and missing or ambiguous proof remain
  exact-closed.
- The detect strict exact safety gate was split from unit extraction into
  `crates/nose-detect/src/strict_exact.rs`, reducing `units.rs` to extraction,
  fragment classification, and feature orchestration while keeping proof-policy
  tests next to the strict exact module.
- The `nose-semantics` production facade is now physically split as well:
  evidence/source/domain proof helpers live in `evidence.rs`, effect/place proof
  helpers live in `effects.rs`, negative API guard policy lives in
  `api_guards.rs`, library API contract identities and result wrappers live in
  `library_api/contracts.rs`, first-party library API row constructors live in
  `library_api/rows.rs`, evidence-hash-to-row registry helpers live in
  `library_api/registry.rs`, library API occurrence evidence/admission and
  dependency validation logic live in `library_api.rs`, and `lib.rs` preserves
  the existing flat public facade while shedding the mixed 9k-line
  implementation body.
- The same code-quality pass split the CLI end-to-end test target into a small
  `tests/cli.rs` harness plus topic modules, and moved the Type-4 generator's
  axis metadata/model/aggregate helpers under `bench/type4/type4gen/` while
  preserving `bench/type4/generate.py` as the stable CLI/import entry point.
- The post-PR #147 completion audit found and closed a high-risk list-builder
  append consumer bypass where raw active-builder method selector spelling plus
  a first-party language row could prove append semantics. Remaining raw-looking
  pockets are first-party evidence producers, test fixtures, migrated
  admitted-resolver consumers, intentionally opaque call identity policy, or
  future pack surfaces. The detailed classification is in
  [semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md).
- The v0 pack extension API defined the first provider-facing manifest shape for
  language/library packs, including evidence producers, contract rows, anchors,
  dependencies, channel eligibility, trust/default status, provider/user
  responsibility boundaries, examples, local metadata loading, and local
  conformance checks for manifests plus declared fixture assets.
- The #109 semantic-kernel migration closed issues #150-#157, #166, #169, #168,
  and #167. The closeout records the landed API/loading/conformance/demand/
  effect/Promise/call-target/domain foundation, the first compiled first-party
  pack pilot, broader imported call-target producer coverage, admitted library
  HOF demand/effect contracts, and the first compiled LawPack provenance pilot.
- The first first-party pack pilot moved Python stdlib type-domain aliases from
  a raw helper table into a pack-shaped contract row set with active
  `nose.python.stdlib.type_domain` provenance.

## Phase 0: documentation and vocabulary (landed)

- PR #100 defined semantic-kernel goals, non-goals, responsibility model, and
  pack kinds.
- The current implementation snapshot is recorded separately from this roadmap.
- The direction is linked from home, architecture, languages, and
  formal-soundness.
- The docs distinguish implemented facade behavior from planned external-pack
  capability.
- The v0 provider-facing pack API is documented separately from the snapshot and
  roadmap so current implementation status, history, and extension design do not
  blur together.

## Phase 1: kernel facade and fail-closed migration (foundation tranche landed)

Landed in PR #100 and PR #101:

- `nose-semantics` exists as the first compiled facade for language profiles,
  semantic facts, effect/operator/fragment predicates, stdlib/API contracts, law
  ids, and proof status.
- First-party built-in profiles now wrap many existing `Lang` matches behind
  named predicates and contracts.
- Several proof-sensitive direct `Lang`/name checks were replaced with semantic
  predicates or fail-closed contracts.
- Old name-only recognizers were narrowed when receiver, import, shadowing,
  constructor, or protocol proof was missing.
- Tests now cover language, arity, shadowing, import, receiver, and hard-negative
  obligations for the migrated facade paths.
- Parser and lowering dispatch remain unchanged.

Remaining after the #109 closeout:

- Broaden first-party compiled packs beyond the Python stdlib type-domain pilot
  and the value-graph LawPack pilot, keeping external packs metadata-only until
  an executable producer runtime and trust policy are explicitly designed.
- Continue producer coverage beyond the imported call-target slice for direct
  methods, dynamic dispatch, richer domains, guards, aggregates, and
  module/export dependencies without reopening raw selector/name/type/tag
  fallbacks.
- Expand demand/effect contracts beyond the first admitted library HOF timing
  slice into lazy, iterator, generator, async, channel, repeated, and
  call-by-need semantics before ecosystem APIs can enter exact matching.
- Expand the first pack-facing value-law provenance pilot beyond
  `factor_distribute` and `clamp` to reduction laws, parity/toggle laws,
  low-level byte-pack laws, structural recursion, ecosystem law packs, and
  external LawPack producer execution.
- Keep behavior-changing recall reductions documented when missing evidence
  blocks exact convergence, and preserve the current precision gates while more
  first-party surfaces move behind shared contracts.

## Phase 2: shared contracts for duplicated gates

- Continue moving primitive operator gates behind `OperatorSemantics`. The first
  larger slice covers comparison transforms/laws, cardinality thresholds, static
  index-membership thresholds, and Python source `in` membership exact-safety.
  A later source-fact slice preserves selected JS/TS and Python equality-like
  source operators, but broader operator dispatch, overload semantics, and
  pack-facing consumers remain open.
- Continue migrating compatibility storage onto `EvidenceRecord` consumers.
  Source-origin, parameter-domain, import identity, symbol identity, guard,
  selected place/effect, selected library API occurrence, and selected
  sequence-surface consumers now use evidence-only proof paths where covered.
  Remaining mirror work is concentrated in broader lowered sequence/tag surfaces
  and unmodeled module/export dependencies rather than source/domain side tables.
- Add scope, dependency, and ambiguity validation for evidence records before
  they become a stable external extension surface.
- Expand the exact fragment facade from first-party helper functions into
  versioned pack-facing effect/place evidence records. The current substrate
  covers canonical append calls, C/Go/Java non-overloadable index writes, Java
  self-receiver/self-field writes, binding writes, receiver-mutation risks, and
  opaque argument escapes through required `Effect`/`Place` evidence, including
  normalize refreshes after canonical rewrites. Exact effect proofs and
  mutation-risk effects remain separate contract families.
- Continue replacing remaining local exact-fragment proof helpers with
  versioned pack-facing evidence records, especially broader field/read/write
  place facts, setter/proxy/property-write facts, and demand-aware effect
  summaries shared with lazy/async/channel protocols.
- Continue moving library API recognition into `LibraryApiContract` rows and
  `LibraryApi` occurrence evidence. The already producer-covered occurrence
  surfaces are now fail-closed on missing evidence; remaining work is promise
  receiver proof, explicit async/sync and Go channel protocol convergence
  contracts, richer Python/Ruby/Java/Rust iterator materialization/demand
  contracts, and ecosystem APIs whose receiver/domain/demand obligations are not
  yet expressible.
  The first internal slice covers collection/map factories, selected
  constructors, Java empty collection constructors, Java `Map.entry`, and the
  shared shadow/import/result
  obligations consumed by normalize and strict exact gates. The next slice moved
  selected non-factory surfaces behind the same identity/result facade: map-key
  views and wrappers, map `get`, map defaulting method calls, static JS-like
  helpers, regex-literal `.test`, Python `math.prod`, promise `.then`, iterator
  identity adapters, Java `Arrays.stream`, and existing language-scoped method
  call contracts. Occurrence-evidence slices now cover selected JS-like
  static/global APIs, Python builtin/import-backed factories/functions, Rust
  free-name/path factories, Ruby require-backed factories, Java `java.util`
  static factories/adapters and selected empty constructors, JS regex literals,
  JS/TS static-index membership, JS/TS/Java property builtins, Rust
  Option/scalar APIs, and selected receiver-method families.
  Remaining stdlib and ecosystem APIs still need dependency-backed occurrence
  records before they become pack-facing. Producer-covered
  factory/API result calls now also emit dependent call-node `Domain` evidence
  when the current `DomainEvidence` vocabulary can represent the result.
- Keep value-graph and strict exact gates on the same contract source. Factory,
  constructor, and selected method/view/adapter gates now share
  `LibraryApiContract` identity/result rows, and selected JS-like,
  Python builtin/import-backed, Rust free-name/path, Ruby require-backed, Java
  `java.util`, and regex calls now additionally share `LibraryApi` occurrence
  evidence, as do generic Python/Go free-function builtins and selected
  receiver-method families. Selected normalize idiom, value-graph, and strict
  exact consumers now call shared `nose-semantics` admitted occurrence resolvers
  for method, free-function builtin, map-get, map-key-view, regex, JS
  static/global, static-index, iterator/static collection adapter, Rust
  Option/scalar/`Vec::new`, and first-party factory/constructor calls instead of
  locally recombining raw selector parsing with evidence admission. Value-graph
  direct factory/constructor eval and provider literal export safety now share
  those resolvers where they still operate on source call nodes; selected
  value-level span-query paths now use dedicated span resolvers for
  free-name/imported collection factories, Java/Ruby/Rust collection factories,
  free-name/Java map factories, Java map entries, map-get, and map-key
  view/wrapper calls. Node-level property builtins, Rust `Some` callee checks,
  HOF receiver proof, Promise `resolve`, and Promise `.then` contract lookup
  also go through shared resolvers. Lowered sequence-surface consumers are now
  evidence-only where covered. Remaining API work is broader thenable
  assimilation, explicit async/sync protocol convergence contracts, and
  ecosystem APIs only after demand, receiver, and effect obligations are
  expressible.
- Continue import/module proof migration beyond the removed raw import payloads
  and evidence-only import identity path. Value-graph import identity and
  imported-symbol exact proof are now evidence-only, imported literal replacement
  copies provider evidence, provider literal export safety consumes a shared
  `nose-semantics` helper, and selected JS/TS `QualifiedGlobal` paths are covered
  with same-span root dependencies, but general qualified-member resolution,
  namespace export identity, provider/export dependency manifests, richer
  scope/rebinding facts, broader producer coverage for module-defined local
  methods/dynamic dispatch, richer nested-function scope, and manifest-level
  cross-module dependency evidence are not.
- Generalize dedicated guard evidence beyond the first JS/TS record-shape and
  own-property contracts, including richer source-clause records, API dependency
  validation, subject/place identity, and truthiness/null semantics.
- Expand the first `SequenceSurface` evidence into richer sequence/aggregate
  records for factories, more nested entries, iterator views, and
  exported-literal eligibility. Current exact/value-graph consumers are
  evidence-only for covered lowered surfaces, but richer aggregate semantics
  still need versioned records beyond the first tag-kind vocabulary.
- Continue expanding domain evidence producers beyond the current first-party
  annotation/alias and selected API-result facts. The shared receiver-domain
  consumer contract now accepts exact node-anchored receiver facts,
  binding-anchored immutable local/module facts, selected admitted library/API
  factory result facts, and a broader domain vocabulary. Remaining work is
  broader inferred receiver domains, richer field/property and nominal-type
  producer coverage, Java constructor call-domain evidence if that lowering
  stops collapsing directly to sequence surfaces, and protocol-specific receiver
  facts that include demand/effect obligations.
- Turn the remaining named value-graph rule modules into LawPack-facing law
  ids/contracts while retaining formal-obligation metadata as the first-party
  proof boundary. The first compiled `nose.value_graph.laws` pilot now reports
  per-family provenance for numeric common-factor distribution and integer
  ordered min/max clamp. Reduction laws, parity/toggle laws, low-level byte-pack
  laws, structural recursion, and ecosystem law packs remain local first-party
  code or metadata-only declarations.
- Add receiver/place facts so field read/write and property contracts are not
  field-name-only.
- Add provenance fields internally before exposing them in scan JSON.

## Phase 3: first-party packs

- Convert Python, JavaScript/TypeScript, Go, Rust, Java, C, Ruby, and embedded
  JS/TS containers into first-party compiled packs.
- Split stdlib knowledge, including dependency-backed type-domain alias rows,
  into first-party `StdlibPack`s.
- Define conformance manifests for each pack: positive convergence cases, hard
  negatives, Raw coverage expectations, oracle coverage, and proof obligations.
- Ensure existing docs and capabilities are generated from or checked against pack
  metadata.

## Phase 4: external pack contract

- The first versioned pack manifest schema is defined in
  [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md). Local
  metadata loading is implemented for manifest files/directories and documented
  in [semantic-pack-loading](semantic-pack-loading.md); local structural
  conformance is implemented in
  [semantic-pack-conformance](semantic-pack-conformance.md); external packs
  remain metadata-only.
- Start with data-only external packs for simple APIs once producer execution and
  executable fixture/oracle checks exist.
- Add restricted recognizer hooks only after the manifest path is stable.
- Require pack metadata: provider, license, version range, supported analysis
  channels, evidence status, conformance commands, and semantic provenance ids.
- Keep the pack conformance checklist explicit: structural harness results,
  semantic correctness evidence, and enablement risk are provider/user
  responsibility unless the pack is first-party.
- User configuration and `--semantic-pack` can enable local manifests explicitly.
- Scan JSON reports active pack provenance and whether each pack influenced
  evidence/contracts or metadata only. Per-finding contract/law provenance and
  external pack influence on `near`/exact results remain open.

The external schema must make proof obligations first-class. For example, a pack
claiming `pkg.Foo.map` maps to the `Map` protocol must say how `pkg.Foo` is
resolved, which versions it covers, how callback demand works, whether effects
are delayed or eager, and which hard negatives distinguish it from same-named but
different APIs.

## Phase 5: demand-aware semantics

- Model child demand: always, never, conditional, per-element pull,
  short-circuit-until, maybe repeated, and call-by-need memoized.
- Model effect visibility under demand: skipped effects, delayed effects,
  per-element callback effects, async scheduling, yields, and stream emissions.
- Refactor oracle and value graph to consume demand rules instead of local
  hard-coded evaluation behavior. The oracle consumes internal builtin
  demand/effect profiles; value-graph Python generator exception timing consumes
  source-backed HOF demand/effect profiles; admitted library HOF consumers now
  resolve eager JS-like/Ruby timing versus pull-lazy Rust/Java timing before
  opening callback exception, `len`, terminal-reduction, value, or strict-exact
  paths; and supported Promise `.then` chains consume the async-continuation
  profile while preserving a Promise boundary. The pack-facing schema and most
  protocol-specific consumers remain open.
- Keep expanding lazy iterator/generator/channel hard negatives before enabling
  new exact laws. The first Python generator/list/set and Go channel/goroutine
  hard negatives are now in place, along with hard negatives for pull-lazy
  library HOF callback timing and `len` materialization. Remaining work is
  richer repeated, call-by-need, iterator exact-size/materialization,
  async/generator/channel, callback-effect, scheduling, and report-provenance
  contracts.

## Phase 6: ecosystem packs

- Add high-value first-party packs only when their contracts are narrow and
  testable.
- Keep community packs external and opt-in unless nose explicitly adopts them as
  first-party/default packs with project-owned gates.
- Candidate areas: Lodash, RxJS, NumPy, pandas, Java Streams/Guava, Rust Iterator
  ecosystem helpers, Tokio futures, Rails ActiveSupport collection helpers.
- Keep exact eligibility narrow. Many APIs should stay `near-only` because
  versioning, mutability, callback effects, or dynamic dispatch make exact
  equivalence too risky.

## Open questions

- How much of a pack should be data-only, and when is a restricted recognizer hook
  justified?
- Should external recognizers run as compiled Rust, WASM, or a sandboxed DSL?
- What is the minimum provenance that scan JSON must expose without making reports
  noisy?
- How should users pin pack versions in CI?
- How should conflicting packs or overlapping API contracts be resolved?
- What conformance score is enough for a first-party pack to enter the default
  exact channel?
- Should a pack be able to express language-specific proof-producing lowering
  extensions before the general construct/import/type fact model is complete?

## Foundation acceptance status

The first implementation slice landed through PR #100 and PR #101. It is
considered successful because it:

- introduced the semantic-kernel vocabulary and first compiled facade;
- replaced multiple proof-sensitive `Lang`/name matches with named semantic
  predicates or fail-closed contracts;
- recorded intentional old-behavior changes where missing evidence blocks exact
  convergence;
- kept tests and docs checks green after the proof-gated scan follow-up;
- documented the first-party/external responsibility boundary;
- made accepted exact matches easier to explain through explicit contracts and
  hard-negative tests.

The next implementation slices should be judged by whether they remove another
class of scattered semantic knowledge without widening exact acceptance beyond
the available evidence.
