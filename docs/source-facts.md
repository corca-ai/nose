# Source facts and semantic evidence

Back to [semantic-kernel](semantic-kernel.md). Current implementation status is
recorded in [semantic-kernel-snapshot](semantic-kernel-snapshot.md); history and
remaining work are tracked in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md). The shared internal record
shape for all semantic evidence is described in
[evidence-records](evidence-records.md).

Source facts are the bridge between source syntax and semantic contracts. They
preserve source-origin distinctions that the shared IL intentionally abstracts
away, such as `new Set(...)` versus `Set(...)`, a JavaScript regex literal versus
an ordinary string, or JavaScript `===` versus `==`.

They are evidence, not semantics. A source fact never approves an exact clone,
mints a value fingerprint, or bypasses a law. It is admissible only when a
kernel contract explicitly consumes it together with the contract's language,
symbol, receiver, arity, shadowing, version, demand, effect, and proof
preconditions.

## Goal

- Make source-origin facts first-class inputs to semantic contracts.
- Let language and library packs state which source facts they emit and require.
- Preserve or improve precision by keeping exact matching fail-closed when a
  required source fact is absent.
- Improve recall only where the source fact supplies evidence that the previous
  IL shape had lost.
- Build a path to report provenance: source surface, pack id, contract id, law
  id, evidence status, and proof status.

## Non-goals

- Do not treat source facts as approval for exact equivalence.
- Do not use broad heuristics such as "a function named `map` means Map
  semantics" or "a method named `test` means regex semantics".
- Do not expose raw CST nodes as the pack interface.
- Do not promise that the current internal evidence records are the final scan
  JSON or external pack manifest contract.
- Do not make nose responsible for certifying external pack correctness. nose
  validates the extension shape and fails closed; external providers own their
  claims, and users own the decision to enable them.

## Terminology

| term | meaning |
|---|---|
| Source evidence | `EvidenceKind::Source` records keyed by stable source anchors. |
| Evidence record | Current internal form of a source or semantic fact, with stable ids, anchors, dependencies, status, and provenance. The external pack manifest is not defined yet. |
| Contract | Kernel rule that maps a proven source/API surface to a semantic operation or law under explicit preconditions. |
| Pack provenance | The pack id, provider, trust policy, version range, contract id, and evidence status behind an admitted fact or contract. |

## Evidence flow

1. A frontend or language pack parses source, lowers to IL, and emits
   kernel-defined evidence records for source distinctions that would otherwise
   be lost.
2. The semantic kernel checks the fact shape and contract admission obligations:
   language, arity, receiver, symbol/import proof, shadowing, scope, version,
   mutation, demand, and effects. External semantic correctness remains the
   provider's claim and the user's opt-in decision.
3. Contracts consume validated evidence. If a required fact is missing or
   ambiguous, the exact channel stays closed.
4. Normalize and detect use only admitted contracts and kernel proof helpers for
   exact semantic fingerprinting.
5. Reports should eventually expose the pack and contract provenance that
   influenced each exact match. The current public scan JSON does not yet expose
   those fields.

## Fact classes

The pack-facing vocabulary should cover at least these classes.

| class | examples |
|---|---|
| Symbol and import | resolved import binding, namespace import, unshadowed language global, qualified global/member API path, version range |
| Receiver and domain | array, collection, map, option, string, primitive integer, byte array, promise-like receiver |
| Operator | strict equality, loose equality, identity equality, value equality, type membership, language membership |
| Literal and surface | regex literal, string literal, map/object literal, tuple/list/array surface, computed property key |
| Call shape | constructor call, ordinary function call, method call, property access, macro-like call |
| Sequence and aggregate | collection surface, map-entry surface, iterator surface, exported literal surface |
| Place and mutation | receiver field, index assignment, builder append, immutable binding, direct write, opaque escape |
| Module export | exported binding, import dependency, provider mutation proof, importer mutation proof |

## Current internal slice

The current implementation has `Il::evidence` records in `nose-il` and
source-fact helpers in `nose-semantics`. Source-origin proof is evidence-only:
frontends emit `EvidenceKind::Source` records directly, and consumers do not
fall back to a side-table mirror when source evidence is missing.

- JS/TS lowering emits source facts for construct syntax, regex literals, strict
  equality, strict inequality, loose equality, loose inequality, and
  `instanceof`.
- Python lowering emits source facts for value equality/inequality and identity
  equality/inequality.
- JS-like `new Set(...)` and `new Map(...)` can enter exact matching only when
  construct syntax is proven, the `Set`/`Map` callee has unshadowed-global
  symbol proof, and the collection/map argument remains exact-safe. Plain
  `Set(...)` and `Map(...)` stay closed.
- JS/TS regex literal `.test(value)` can enter exact matching only when the
  receiver is proven to be a regex literal. Ordinary string `.test(...)` stays
  closed.
- JS-like static membership callbacks such as `x => x === value` consume source
  operator facts and require strict equality/inequality. Loose equality and
  `instanceof` stay closed for those exact contracts.
- Qualified API path evidence is symbol evidence, not a source fact by itself.
  The current JS/TS producer emits selected `QualifiedGlobal` facts such as
  `Object.hasOwn`, `Object.prototype.hasOwnProperty.call`, and `Array.from`
  only when their root global is proven unshadowed.

The slice deliberately does not claim that all equality semantics are modeled.
The IL still has coarse equality operators; source facts are the evidence that
lets a small set of contracts recover the original source surface safely.
Construct syntax likewise proves only that the surface used `new`; constructor
contracts still need callee identity evidence such as an unshadowed global or a
future resolved-symbol fact.

## Pack boundary

Packs should use only kernel-defined fact kinds and contract fields. A pack that
claims exact eligibility for a source/API surface must declare:

- the source surface and language/runtime/package/version range;
- the required symbol, receiver, import, shadowing, overload, and arity proof;
- the evaluation, demand, effect, mutation, and exception behavior relevant to
  the contract;
- the exact/near eligibility and proof status;
- positive conformance fixtures and hard negatives;
- known unsupported or unsound boundaries;
- provenance labels suitable for reports.

Meeting this shape is not nose certification. First-party default packs are
validated by the nose project. External packs are provider/user responsibility
and must be explicitly enabled by the user.
