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
  migrated.
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
| `Symbol` | resolved or proven symbol identity, with record kinds for unshadowed globals and static imported binding/namespace aliases |
| `SequenceSurface` | lowered aggregate surface such as collection, tuple, map, pair, import proof, record guard, or Go composite map literal |

## Consumption Rules

Consumers should go through `nose-semantics` helpers rather than scanning raw IL
side tables.

- A lookup succeeds only when asserted evidence resolves to exactly one relevant
  fact.
- Conflicting asserted evidence is treated as missing evidence.
- `Ambiguous` evidence is treated as missing evidence.
- If any relevant ambiguous/conflicting evidence exists, compatibility fallback
  must not reopen the exact path.
- Compatibility fallback is allowed only when no relevant evidence record exists.

This is stricter than a name or tag check. For example, a raw
`Seq("import_binding")` is still serialized for compatibility, but import
contracts first consult `Import` evidence. If that evidence is ambiguous, exact
import proof stays closed instead of falling back to the raw sequence payload.

Symbol identity follows the same rule. A method selector such as `abs` or a
receiver spelling such as `Math` is not proof. Exact consumers must require a
language-scoped contract plus symbol evidence, or a compatibility fallback that
proves the same import/global/shadowing obligations. Binding-level import
evidence does not by itself prove every use of the same local name; if the alias
is rebound or ambiguous, the exact path stays closed until a node-level symbol
fact or stronger scope-resolution evidence exists.

## Current Producers

First-party frontends now mirror these facts into `EvidenceRecord`:

- parameter semantic annotations become `Domain` evidence;
- source-origin facts become `Source` evidence;
- import binding and namespace lowering emits `Import` evidence for the proof RHS
  and `Symbol` evidence for the local alias identity;
- lowered `Seq` surfaces emit `SequenceSurface` evidence.

The older `ParamTypeFact`, `SourceFact`, and raw import `Seq` shapes remain as
compatibility mirrors. Unshadowed-global `Symbol` records are defined and
consumed through the same helper surface, but first-party frontend producers for
global identity are still a next migration slice; current global proof can still
come from the compatibility shadow-scan path. These mirrors are not the desired
pack boundary.

## Current Consumers

The first migrated consumers are the shared semantic helpers and their direct
callers:

- source-fact lookup for construct syntax, regex literal, and operator provenance;
- parameter domain lookup used by normalize and strict exact receiver gates;
- import proof parsing for normalize, value graph, imported literal replacement,
  and strict exact gates;
- imported namespace/binding symbol proof for normalize idiom admission,
  value-graph namespace fallbacks, and strict exact gates, plus a shared
  unshadowed-global helper that still depends on compatibility shadow proof until
  global `Symbol` producers land;
- sequence-surface admission for normalize/value-graph/detect exact paths.

Field/place/effect facts, receiver/protocol evidence beyond parameter domains,
full scope-resolution and namespace-member evidence, report-level provenance,
and external manifest loading are still open work.
