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

- Give source, domain, import, symbol-identity, guard, place/effect, selected
  library API occurrence, and sequence-surface proof facts one shared shape.
- Make facts carry stable ids, anchors, provenance, dependencies, and status.
- Keep exact matching fail-closed when evidence is missing, ambiguous, or
  conflicting.
- Preserve existing behavior while first-party frontends mirror their older
  side-table facts into the new record shape.
- Make the future external pack schema a narrowing of an implemented internal
  boundary, not a speculative document-only API.

## Non-goals

- Do not let evidence records mint value fingerprints, bypass laws, or mark clone
  pairs exact.
- Do not expose this internal Rust record as the final external pack manifest or
  scan JSON schema.
- Do not remove compatibility mirrors in the first slice. `SourceFact`,
  `ParamTypeFact`, and raw import `Seq` payloads still exist while consumers are
  migrated, though new proof-bearing consumers should prefer evidence-only
  helpers.
- Do not certify external pack claims. nose validates record shape and fails
  closed; providers own their claims, and users own opt-in decisions.
- Do not model demand evidence or every place/effect family in this slice. The
  current place/effect records cover the first exact-fragment substrate only.

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
| `Source` | construct syntax, regex literal provenance, and source operator family |
| `Domain` | receiver/value domain such as collection, map, option, string, integer, or byte array |
| `Import` | static import binding/namespace proof and imported-literal snapshot provenance |
| `Symbol` | resolved or proven symbol identity, with record kinds for unshadowed globals, static imported binding/namespace aliases, and selected qualified global API paths |
| `Guard` | multi-obligation guard proof facts such as JS/TS record-shape and own-property guard contracts |
| `Place` | fixed receiver/place facts currently covering `SelfReceiver` and `SelfField` |
| `Effect` | observable effect facts currently covering canonical builder append calls, non-overloadable index writes, and fixed self-field writes |
| `LibraryApi` | proof that a specific call occurrence matches a language/API contract coordinate, currently for selected JS-like static/global APIs plus selected import/source-backed Python, Java, and regex APIs |
| `SequenceSurface` | lowered aggregate surface such as collection, tuple, map, pair, import proof, guard surfaces, Go composite map literals, or Go map entries |

`LibraryApi` evidence is an occurrence fact, not the whole contract. It records
the contract id, callee coordinate, arity, and dependencies for a specific call
node. `LibraryApiContract` rows in `nose-semantics` still name result semantics
and the remaining obligations. Existing evidence kinds such as `Symbol`,
`Import`, `Source`, `Domain`, and `SequenceSurface` prove those obligations; the
contract decides whether the facts are enough to admit an exact or value-graph
path. A future external pack schema may expose library API contracts, but
providers still emit facts and contracts rather than exact-clone verdicts.

## Consumption Rules

Consumers should go through `nose-semantics` helpers rather than scanning raw IL
side tables.

- A lookup succeeds only when asserted evidence resolves to exactly one relevant
  fact.
- Conflicting asserted evidence makes the lookup fail.
- `Ambiguous` evidence makes the lookup fail.
- If any relevant ambiguous/conflicting evidence exists, compatibility fallback
  must not reopen the exact path.
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

Qualified global identity is also evidence, not a selector guess. The current
first-party JS/TS producer emits `QualifiedGlobal` only for selected static paths
whose root is proven unshadowed, such as `Object.hasOwn`,
`Object.prototype.hasOwnProperty.call`, `Array.from`, and `Array.isArray`.

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

Place and effect evidence are also authoritative where present. For example,
raw method selectors such as `push`, `append`, or `add` do not prove an append
effect; a consumer needs `Effect(BuilderAppendCall)` or the legacy canonical
`Builtin::Append` compatibility path when no effect evidence exists. Likewise,
non-overloadable index writes and fixed self-field writes are admitted through
`Effect` records, with `Place(SelfReceiver)` and `Place(SelfField)` proving the
receiver/place side. First-party `Place(SelfField)` depends on the matching
`Place(SelfReceiver)`, and `Effect(SelfFieldWrite)` depends on the matching
`Place(SelfField)`. Conflicting or ambiguous place/effect evidence blocks the
legacy language-gated fallback.

Library API evidence follows the same fail-closed rule. If a call carries
`LibraryApi` evidence for a selected API occurrence, consumers must validate the
contract id, callee coordinate, arity, dependencies, dependency anchors, and call
shape before using legacy name/symbol fallback. A conflicting, ambiguous,
wrong-callee-anchor, wrong-dependency-anchor, or dependency-broken API record on
the queried call closes that API path. A record anchored to another call is
irrelevant to this lookup and leaves the compatibility path available. The
record proves only API identity; exact consumers still separately prove
receiver/domain facts, source-surface requirements, argument safety, result
shape, mutation safety, and demand/effect obligations.

Imported API occurrence evidence is not a broad name guess. A call-site
`Symbol(ImportedBinding)` or `Symbol(ImportedNamespace)` dependency must itself
depend on the matching binding-anchor symbol, pass rebinding and local/parameter
shadow checks, and match the current receiver/callee span when that span is
available. If normalization erases an import occurrence into a seeded import
value, consumers pass no occurrence span and rely on that validated dependency
instead of accepting an unrelated imported symbol elsewhere in the file.

## Current Producers

First-party frontends now mirror these facts into `EvidenceRecord`:

- parameter semantic annotations become `Domain` evidence;
- source-origin facts become `Source` evidence;
- import binding and namespace lowering emits `Import` evidence for the proof RHS
  and `Symbol` evidence for the local alias identity;
- JS/TS static-global value occurrences that remain as `Var` nodes, such as
  member receivers, call callees, constructors, and `undefined`, emit
  `UnshadowedGlobal` symbol evidence when the frontend proves no local shadow;
- selected JS/TS qualified static global paths emit `QualifiedGlobal` symbol
  evidence at the lowered node anchor: own-property guards at their
  `Seq("own_property_guard")` node, and static member expressions such as
  `Array.from` and `Array.isArray` at their `Field` node;
- JS/TS own-property guard lowering emits `Guard::JsOwnProperty` evidence for
  the lowered `Seq("own_property_guard")`, with an asserted `QualifiedGlobal`
  dependency for the admitted API path;
- JS/TS record-shape guard lowering emits `Guard::JsRecordShape` evidence for
  the lowered `Seq("record_guard")`, including the shared subject hash, the
  null/truthiness clause kind, whether JS loose equality was admitted, and
  asserted dependencies for the required `Array.isArray` API proof plus optional
  `Boolean` proof;
- first-party lowering emits `Place(SelfReceiver)` for Java `this`,
  `Place(SelfField)` for Java `this.field`, `Effect(SelfFieldWrite)` for Java
  `this.field = ...`, `Effect(NonOverloadableIndexWrite)` for C/Go/Java index
  writes, and `Effect(BuilderAppendCall)` for canonical `Builtin::Append`.
  Self-field place/write evidence records include dependencies that link the
  write proof back to the receiver proof;
- first-party lowering emits `LibraryApi` evidence for selected API calls that
  remain as raw call nodes: JS-like `Array.from(...)`, `Array.isArray(...)`,
  `Boolean(...)`, `new Map(...)`, and `new Set(...)`; Python
  `collections.deque(...)` through imported binding/namespace proof; Python
  `math.prod(...)` through imported namespace proof; Java `java.util` static
  factories/adapters including `List.of`, `Set.of`, `Arrays.asList`, `Map.of`,
  `Map.ofEntries`, `Map.entry`, and `Arrays.stream`; and JS-like regex-literal
  `.test(...)`. These records depend on the relevant `QualifiedGlobal`,
  `UnshadowedGlobal`, import-backed call-site `Symbol`, construct-syntax
  `Source`, or regex-literal `Source` evidence. Calls collapsed into specialized
  guard surfaces emit their guard evidence instead. Shadowed roots, unsupported
  arities, unsupported static paths, raw free-name/path factories without a
  resolved symbol fact, and Ruby `require`-based APIs without require evidence
  do not emit API occurrence evidence;
- lowered `Seq` surfaces emit `SequenceSurface` evidence, including Go map
  literal and Go map-entry surfaces where those tags carry first-party meaning.

The older `ParamTypeFact`, `SourceFact`, and raw import `Seq` shapes remain as
compatibility mirrors. First-party JS/TS record-shape guards now have dedicated
guard evidence, and exact-fragment append/index/self-field gates now have the
first place/effect evidence substrate. Broader guard families, richer
source-clause dependencies, richer receiver/place families, and general evidence
validation remain open. These mirrors are not the desired pack boundary.

## Current Consumers

The first migrated consumers are the shared semantic helpers and their direct
callers:

- source-fact lookup for construct syntax, regex literal, and operator provenance;
- parameter domain lookup used by normalize and strict exact receiver gates;
- import proof parsing for compatibility helpers, with value-graph import
  identity and imported literal replacement consuming evidence-only facts;
- cross-file imported literal replacement copies the provider's closed evidence
  subgraph into the importer with remapped anchors/dependency ids, then records
  `Import(ImportedLiteralSnapshot)` provenance that depends on the importer
  import proof and copied provider evidence;
- imported namespace/binding symbol proof for normalize idiom admission,
  value-graph namespace fallbacks, and strict exact gates, without raw assignment
  fallback;
- value-graph internal import identity now uses dedicated
  `ImportNamespace`/`ImportBinding` value ops derived from `Import` evidence, so
  raw import `Seq` payloads cannot hash-cons with proof-bearing import values;
- unshadowed-global symbol proof for JS/TS `Math.*` method contracts,
  `new Map(...)`/`new Set(...)` constructor contracts, static `Array.isArray`
  exact gates, and `undefined` nullish-default handling, with compatibility
  fallback only when no relevant evidence record exists;
- qualified-global symbol proof for selected JS/TS API paths: own-property
  guard evidence depends on `Object.hasOwn` or
  `Object.prototype.hasOwnProperty.call`, and map-key view wrappers require
  evidence for `Array.from`;
- selected `LibraryApiContract` consumers now consult `LibraryApi` occurrence
  evidence first for the migrated JS-like, Python imported, Java static, and
  regex-literal surfaces; conflicting or dependency-broken API evidence keeps
  the value-graph, idiom, and strict exact paths closed. When no relevant API
  evidence is present, the current compatibility path still uses the older
  symbol/source proof helpers. Other factory and method/view contract rows still
  name API identity/result semantics while local consumers prove their current
  `Symbol`, `Import`, `Source`, `Domain`, and `SequenceSurface` obligations;
- JS/TS record-shape guard exact admission and value-graph tagging require both
  `SequenceSurface(RecordGuard)` and `Guard::JsRecordShape`; raw
  `Seq("record_guard")` cannot enter the proof-bearing exact/value-graph path by
  tag spelling alone;
- JS/TS own-property guard exact admission and value-graph map-default
  normalization require both `SequenceSurface(OwnPropertyGuard)` and
  `Guard::JsOwnProperty`; raw `Seq("own_property_guard")` plus a path-shaped
  spelling is not proof by itself;
- sequence-surface admission for normalize/value-graph/detect exact paths where
  the surface contract is independently exact-safe; guard surfaces use their
  dedicated guard helper instead. Go zero-map literal lookup also requires
  `SequenceSurface(GoCompositeMapLiteral)` and `SequenceSurface(GoMapEntry)`,
  so `composite_literal`/`keyed_element` tag spelling alone no longer admits the
  exact map-default path;
- exact-fragment append, non-overloadable index-write, and self-field-write
  gates now consult `Effect`/`Place` evidence first, falling back to the legacy
  language-gated helper only when no relevant evidence record exists. Ambiguous
  or conflicting evidence keeps the exact path closed.

Broader field/place/effect facts, `LibraryApi` occurrence evidence for remaining
free-name/path/receiver-method APIs, receiver/protocol evidence beyond parameter
domains, full scope-resolution and namespace-member evidence, require/import
evidence for surfaces such as Ruby `Set.new`, broader guard evidence, general
cross-module dependency manifests, report-level provenance, and external
manifest loading are still open work.
