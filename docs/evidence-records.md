# Semantic evidence records

Back to [semantic-kernel](semantic-kernel.md). Current implementation status is
recorded in [semantic-kernel-snapshot](semantic-kernel-snapshot.md); history and
remaining work are tracked in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). Source-origin facts are
covered in [source-facts](source-facts.md).

Evidence records are the internal substrate that lets language and library packs
emit proof facts without giving those packs authority to approve exact clones.
They are facts, not verdicts. Contracts in `nose-semantics` decide whether a fact
can satisfy an exact-channel precondition.

## Goal

- Give source, domain, import, symbol-identity, and sequence-surface proof facts
  one shared shape.
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
- Do not model place/effect/demand evidence completely in this slice. Those are
  next consumers of the same record substrate.

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
| `Import` | static import binding and namespace proof |
| `Symbol` | resolved or proven symbol identity, with record kinds for unshadowed globals, static imported binding/namespace aliases, and selected qualified global API paths |
| `Guard` | multi-obligation guard proof facts such as the first JS/TS record-shape guard contract |
| `SequenceSurface` | lowered aggregate surface such as collection, tuple, map, pair, import proof, record guard, or Go composite map literal |

`LibraryApiContract` is deliberately not listed here yet. It is currently an
internal `nose-semantics` contract layer that names first-party library/API
identity, callee obligations, shadow/import requirements, and result semantics.
Existing evidence kinds such as `Symbol`, `Import`, `Source`, and
`SequenceSurface` prove those obligations; the contract decides whether the
facts are enough to admit an exact or value-graph path. A future external pack
schema may expose library API contracts, but providers still emit facts and
contracts rather than exact-clone verdicts.

## Consumption Rules

Consumers should go through `nose-semantics` helpers rather than scanning raw IL
side tables.

- A lookup succeeds only when asserted evidence resolves to exactly one relevant
  fact.
- Conflicting asserted evidence is treated as missing evidence.
- `Ambiguous` evidence is treated as missing evidence.
- If any relevant ambiguous/conflicting evidence exists, compatibility fallback
  must not reopen the exact path.
- Compatibility fallback is allowed only when no relevant evidence record exists,
  and only for explicitly legacy compatibility helpers.

This is stricter than a name or tag check. For example, a raw
`Seq("import_binding")` is still serialized for compatibility, but import
contracts first consult `Import` evidence. If that evidence is ambiguous, exact
import proof stays closed instead of falling back to the raw sequence payload.
Value-graph import identity goes further: it consumes only sequence `Import`
evidence and materializes dedicated internal import values, never raw
`ValOp::Seq("import_*")` proof objects.

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
- JS/TS record-shape guard lowering emits `Guard::JsRecordShape` evidence for
  the lowered `Seq("record_guard")`, including the shared subject hash, the
  null/truthiness clause kind, whether JS loose equality was admitted, and
  asserted dependencies for the required `Array.isArray` API proof plus optional
  `Boolean` proof;
- lowered `Seq` surfaces emit `SequenceSurface` evidence.

The older `ParamTypeFact`, `SourceFact`, and raw import `Seq` shapes remain as
compatibility mirrors. First-party JS/TS record-shape guards now have dedicated
guard evidence, but broader guard families, richer source-clause dependencies,
and general evidence validation remain open. These mirrors are not the desired
pack boundary.

## Current Consumers

The first migrated consumers are the shared semantic helpers and their direct
callers:

- source-fact lookup for construct syntax, regex literal, and operator provenance;
- parameter domain lookup used by normalize and strict exact receiver gates;
- import proof parsing for compatibility helpers and imported literal
  replacement, with value-graph import identity consuming evidence-only facts;
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
  guard normalization and strict exact safety require evidence for
  `Object.hasOwn` or `Object.prototype.hasOwnProperty.call`, and map-key view
  wrappers require evidence for `Array.from`;
- JS/TS record-shape guard exact admission and value-graph tagging require both
  `SequenceSurface(RecordGuard)` and `Guard::JsRecordShape`; raw
  `Seq("record_guard")` cannot enter the proof-bearing exact/value-graph path by
  tag spelling alone;
- sequence-surface admission for normalize/value-graph/detect exact paths where
  the surface contract is independently exact-safe; guard surfaces use their
  dedicated guard helper instead.

Field/place/effect facts, receiver/protocol evidence beyond parameter domains,
full scope-resolution and namespace-member evidence, broader guard evidence and
dependency validation, report-level provenance, and external manifest loading
are still open work.
