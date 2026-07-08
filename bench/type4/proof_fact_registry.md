# Type-4 proof fact registry

Reusable proof prerequisites for proof-carrying Type-4 frontier packets. Packets
cite `fact_id` values from `proof_fact_registry.v1.json` and keep only their
packet-specific current status locally.

## Statuses

- `specified-not-modeled`: named prerequisite; no reusable detector/proof
  implementation consumes it yet.
- `modeled-controlled`: implemented or machine-checked for controlled evidence;
  real-corpus members still need source evidence.
- `admitted-real-pair`: reusable and backed by current real-pair evidence, but
  detector admission still depends on every fact required by the packet.
- `retired`: retained for historical artifacts; do not cite from new packets.

## Evidence Requirements

- `source-evidence`: the source program exposes the fact. Names alone are not
  proof.
- `focused-executable`: a focused positive or hard-negative expectation exercises
  the boundary.
- `formal-or-mechanized`: a formal proof, proof obligation, or machine-checked
  model justifies the rewrite precondition.

## Current Facts

| fact | status | evidence requirements | detector admission |
|---|---|---|---|
| `numeric-clamp.bound-order` | `modeled-controlled` | `formal-or-mechanized`, `focused-executable`, `source-evidence` | Requires `numeric-clamp.integer-domain`; does not admit a real pair by itself. |
| `numeric-clamp.integer-domain` | `modeled-controlled` | `formal-or-mechanized`, `focused-executable`, `source-evidence` | Requires `numeric-clamp.bound-order`; does not admit a real pair by itself. |
| `boolean.demorgan.proven-bool-operands` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires source identity, purity, counterexample-loop, and vacuous-truth facts. |
| `quantifier.universal.counterexample-loop` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires vacuous truth, source identity, predicate purity, and any boolean rewrite facts. |
| `quantifier.vacuous-truth` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes the empty-input boundary for a separately proven counterexample loop. |
| `iteration.same-source-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only rules out different receivers, iterators, or traversal sources. |
| `effect.pure-predicate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only permits comparing short-circuit boundaries after predicate effects are closed. |
| `effect.pure-callback` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes callback effect/timing boundaries for separately proven HoF facts. |
| `hof.filter-map.drop-condition-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves that both surfaces drop the same source elements. |
| `hof.filter-map.emitted-value-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves that present branches, maps, or guarded pushes emit the same value. |
| `option.absence-channel.identity` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes absence-vs-payload channel boundaries. |
| `hof.flat-map.nested-iteration-order` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves matching outer/inner traversal coordinates. |
| `hof.flat-map.emitted-value-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves each matched nested coordinate emits the same value. |
| `collection.flatten-depth.one-level` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes flattened-vs-nested output shape boundaries. |
| `collection.membership.api-domain-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires element, collection/source, and mutation facts; does not admit membership convergence by itself. |
| `collection.membership.element-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the searched element coordinate. |
| `collection.membership.collection-source-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the collection/source receiver coordinate. |
| `collection.membership.no-intervening-mutation` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes mutation and stale-receiver boundaries. |
| `reduction.identity-empty-behavior` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes seed and empty-input behavior for separately proven aggregate facts. |
| `reduction.step-coordinate-identity` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves the terminal step, predicate, or contribution observes the same flattened element coordinate. |
| `hof.flat-map.aggregate-guard-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves outer, inner, and terminal guard placement for flat-map aggregates. |
| `map.default.absence-fallback` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Requires receiver identity, key/default coordinates, and mutation closure. |
| `map.receiver.source-identity` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves that both sides query the same map value source. |
| `map.default.key-fallback-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes the key/default coordinate boundary. |
| `map.receiver.no-intervening-mutation` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes mutation and stale-receiver boundaries. |

## Compatibility Aliases

The Python loop/De Morgan packet first landed with packet-local fact IDs. New
packets must cite the neutral IDs above; the old IDs are retained as `retired`
aliases so historical artifacts remain understandable and validators can reject
new packet-local citations.

| retired alias | neutral fact |
|---|---|
| `python-loop-demorgan.boolean-demorgan` | `boolean.demorgan.proven-bool-operands` |
| `python-loop-demorgan.effect-safety` | `effect.pure-predicate` |
| `python-loop-demorgan.iterator-identity` | `iteration.same-source-identity` |
| `python-loop-demorgan.universal-short-circuit` | `quantifier.universal.counterexample-loop`, `quantifier.vacuous-truth` |

## Collection Membership Pattern Matrix

This matrix records the neutral proof perimeter for literal, factory-backed,
imported immutable, and typed dynamic collection membership. The supported
surfaces still depend on source evidence for receiver/API identity, searched
element identity, collection/source identity, and receiver mutation closure.

| fact | Python | JS/TS | Go | Java | Ruby | Rust | Swift |
|---|---|---|---|---|---|---|---|
| `collection.membership.api-domain-identity` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled probe |
| `collection.membership.element-coordinate` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled probe |
| `collection.membership.collection-source-coordinate` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled probe |
| `collection.membership.no-intervening-mutation` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | open |

## Universal Quantifier Pattern Matrix

This matrix records capability by language surface. Python is the first admitted
surface through `python-loop-demorgan-all-2026-07-07`; the other columns are open
candidates until they have their own evidence producer, focused replay, hard
negatives, and PCF/readiness admission.

| fact | Python `all(...)` + loop | Ruby `Enumerable#all?` | Rust `Iterator::all` | JS/TS `every` |
|---|---|---|---|---|
| `quantifier.universal.counterexample-loop` | modeled-controlled; packet admitted | open | open | open |
| `quantifier.vacuous-truth` | modeled-controlled; packet admitted | open | open | open |
| `iteration.same-source-identity` | modeled-controlled; packet admitted | open | open | open |
| `effect.pure-predicate` | modeled-controlled; packet admitted | open | open | open |
| `boolean.demorgan.proven-bool-operands` | modeled-controlled; packet admitted | open | open | open |

Registry entries guide implementation work; they are not detector admission.
