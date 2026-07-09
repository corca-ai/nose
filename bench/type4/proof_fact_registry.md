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
| `numeric.aggregate-value-model-domain` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires source identity, identity/empty, step coordinate, and effect facts; does not prove runtime no-overflow integer arithmetic or admit aggregate convergence by itself. |
| `numeric.selection-value-order-domain` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires source identity, seed/identity, comparator coordinate, and effect facts; does not prove a broad runtime strict-total-order domain or admit min/max selection by itself. |
| `numeric.float-special-value-boundary` | `modeled-controlled` | `focused-executable`, `source-evidence` | Boundary fact only; keeps NaN, signed-zero, and grouping-sensitive float surfaces closed unless separately modeled. |
| `boolean.demorgan.proven-bool-operands` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires source identity, purity, counterexample-loop, and vacuous-truth facts. |
| `quantifier.universal.counterexample-loop` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires vacuous truth, source identity, predicate purity, and any boolean rewrite facts. |
| `quantifier.vacuous-truth` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes the empty-input boundary for a separately proven counterexample loop. |
| `iteration.same-source-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only rules out different receivers, iterators, or traversal sources. |
| `effect.pure-predicate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only permits comparing short-circuit boundaries after predicate effects are closed. |
| `effect.pure-callback` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes callback effect/timing boundaries for separately proven HoF facts. |
| `hof.filter-map.drop-condition-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves that both surfaces drop the same source elements. |
| `hof.filter-map.emitted-value-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves that present branches, maps, or guarded pushes emit the same value. |
| `option.absence-channel.identity` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes absence-vs-payload channel boundaries. |
| `option.value-coordinate-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires absence-channel identity, direction, fallback coordinate, pure/default-trigger, and API identity facts. |
| `option.presence-direction` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only preserves absence versus present boolean direction. |
| `option.default-fallback-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the fallback/default coordinate. |
| `option.default-short-circuit` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes the nullish/absence trigger for pure or already-evaluated fallbacks. |
| `option.api-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves standard nullish, nullable, Optional, or Option channel identity. |
| `hof.flat-map.nested-iteration-order` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves matching outer/inner traversal coordinates. |
| `hof.flat-map.emitted-value-coordinate` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only proves each matched nested coordinate emits the same value. |
| `collection.flatten-depth.one-level` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Only closes flattened-vs-nested output shape boundaries. |
| `collection.membership.api-domain-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires element, collection/source, and mutation facts; does not admit membership convergence by itself. |
| `collection.membership.element-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the searched element coordinate. |
| `collection.membership.collection-source-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the collection/source receiver coordinate. |
| `collection.membership.no-intervening-mutation` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes mutation and stale-receiver boundaries. |
| `collection.empty.receiver-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the checked receiver coordinate. |
| `collection.empty.domain-kind-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves compatible collection domain/kind identity. |
| `collection.empty.predicate-direction` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves empty versus non-empty boolean direction. |
| `collection.empty.no-intervening-mutation` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes mutation and stale-receiver boundaries. |
| `string.affix.receiver-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Requires API, affix coordinate, direction, and arity facts; does not admit affix convergence by itself. |
| `string.affix.affix-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the literal, parameter, or immutable binding affix coordinate. |
| `string.affix.api-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the standard case-sensitive prefix/suffix API identity. |
| `string.affix.import-source-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves namespace/helper or imported affix binding provenance. |
| `string.affix.direction` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only preserves prefix versus suffix direction. |
| `string.affix.whole-string-single-affix` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes offset, tuple, and multi-affix arity boundaries. |
| `reduction.identity-empty-behavior` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes seed and empty-input behavior for separately proven aggregate facts. |
| `reduction.step-coordinate-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves the terminal step, predicate, or contribution observes the same flattened element coordinate. |
| `reduction.terminal-predicate-coordinate` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only proves any/all terminal predicate coordinates after source identity is proven. |
| `reduction.short-circuit-direction` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only closes existential versus universal short-circuit direction. |
| `reduction.selection-seed-domain` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only preserves seeded min/max selection behavior; unseeded terminals need separate proof. |
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

## Option Presence/Defaulting Pattern Matrix

This matrix records the neutral proof perimeter for null/Option presence
predicates and nullish/defaulting surfaces. The supported surfaces still depend
on source evidence for the checked value coordinate, absence channel, boolean
direction, fallback coordinate, pure/default trigger, and API identity.

| fact | Python | JS/TS | Go | Java | Rust | C | Ruby | Swift |
|---|---|---|---|---|---|---|---|---|
| `option.value-coordinate-identity` | modeled-controlled presence | modeled-controlled presence/defaulting | modeled-controlled presence | modeled-controlled presence; Optional fq evidence | modeled-controlled presence/defaulting | modeled-controlled presence | open sweep-only | open probe-only |
| `option.absence-channel.identity` | specified-not-modeled | specified-not-modeled | specified-not-modeled | specified-not-modeled | specified-not-modeled | specified-not-modeled | specified-not-modeled | specified-not-modeled |
| `option.presence-direction` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | open sweep-only | open probe-only |
| `option.default-fallback-coordinate` | not-applicable | modeled-controlled | not-applicable | modeled-controlled Optional fq evidence | modeled-controlled unwrap_or | not-applicable | not-applicable | open |
| `option.default-short-circuit` | not-applicable | modeled-controlled for pure/already-evaluated fallbacks | not-applicable | modeled-controlled Optional fq evidence for pure/already-evaluated fallbacks | modeled-controlled unwrap_or with already-evaluated fallback | not-applicable | not-applicable | open |
| `option.api-identity` | modeled-controlled for built-in None | modeled-controlled for nullish protocol | modeled-controlled nil comparison | modeled-controlled null and fq java.util.Optional | modeled-controlled Option; Result closed | modeled-controlled NULL | open for nil? focused evidence | open probe-only |

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

## Collection Empty-Check Pattern Matrix

This matrix records the neutral proof perimeter for length-zero, size-zero,
named-empty, truthiness, and explicit non-empty collection checks. The supported
surfaces still depend on source evidence for receiver identity, collection
domain/kind identity, predicate direction, and receiver mutation closure.

| fact | Python | JS/TS | Go | Java | Ruby | Rust | C | Swift |
|---|---|---|---|---|---|---|---|---|
| `collection.empty.receiver-coordinate` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | open probe-only |
| `collection.empty.domain-kind-identity` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | open |
| `collection.empty.predicate-direction` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled probe |
| `collection.empty.no-intervening-mutation` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | open |

## String Affix Pattern Matrix

This matrix records the neutral proof perimeter for case-sensitive whole-string
prefix/suffix predicates. The supported surfaces still depend on source evidence
for receiver identity, standard API/import identity, affix coordinate, direction,
and single-affix arity.

| fact | Python | JS/TS | Go | Java | Ruby | Rust | Swift |
|---|---|---|---|---|---|---|---|
| `string.affix.receiver-identity` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled |
| `string.affix.affix-coordinate` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled |
| `string.affix.api-identity` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled |
| `string.affix.import-source-identity` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled immutable binding |
| `string.affix.direction` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled |
| `string.affix.whole-string-single-affix` | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled | modeled-controlled |

## Reduction Pattern Matrix

This matrix records the neutral proof perimeter for sum/product reductions,
any/all terminal predicates, and seeded min/max selection reductions. The
supported surfaces still depend on source evidence for traversal identity,
identity/empty behavior, step or terminal predicate coordinates,
short-circuit direction, selection seed/domain, controlled aggregate value-model
or selection value-order numeric-domain closure, float special-value boundaries,
and effect closure.

| fact | C | Go | Java | Python | JS/TS | Rust | Ruby | Swift |
|---|---|---|---|---|---|---|---|---|
| `reduction.identity-empty-behavior` | modeled-controlled sum loop | modeled-controlled sum loop | modeled-controlled stream reduce | modeled-controlled sum/reduce | modeled-controlled typed reduce | modeled-controlled fold/sum | probe-covered; receiver proof open | probe-covered; focused proof open |
| `reduction.step-coordinate-identity` | modeled-controlled sum/count split | modeled-controlled sum/count split | modeled-controlled sum/product split | modeled-controlled sum/product split | modeled-controlled typed sum/wrong-seed split | modeled-controlled sum/product split | probe-covered; receiver proof open | probe-covered; focused proof open |
| `reduction.terminal-predicate-coordinate` | open int-bool terminal proof | open | open | modeled via quantifier card | modeled-controlled TS any/some plus dense-literal one-arg every | modeled-controlled any/all | open receiver proof | open |
| `reduction.short-circuit-direction` | open int-bool terminal proof | open | open | modeled via quantifier card | modeled-controlled TS any/some plus dense-literal one-arg every; array-param and callback-extra-arg every stay split | modeled-controlled any/all | open receiver proof | open |
| `reduction.selection-seed-domain` | open | open | open | modeled-controlled seeded min/max | open numeric proof | modeled-controlled seeded min/max | open | open |
| `numeric.aggregate-value-model-domain` | modeled-controlled value-model sum; signed overflow/UB closed | modeled-controlled value-model sum; overflow-sensitive proof still separate | modeled-controlled value-model stream reduce; fixed-width overflow closed | modeled-controlled value-model sum/reduce; dynamic float/string values closed | modeled-controlled typed sum; `number` NaN/signed-zero and untyped number remain closed | modeled-controlled value-model fold/sum; overflow-sensitive proof still separate | open receiver proof | open proof |
| `numeric.selection-value-order-domain` | open | open | open | modeled-controlled seeded min/max in focused value-order model; untyped broad total-order proof closed | open numeric proof | modeled-controlled seeded min/max; float/custom order closed | open | open |
| `numeric.float-special-value-boundary` | modeled-controlled boundary | modeled-controlled boundary | modeled-controlled boundary | modeled-controlled boundary | modeled-controlled boundary | modeled-controlled boundary | open | open |

## Universal Quantifier Pattern Matrix

This matrix records capability by language surface. Python is admitted through
`python-loop-demorgan-all-2026-07-07`; TypeScript is admitted for the focused
dense-literal one-argument `Array.prototype.every` plus `for-of` counterexample-loop slice,
while plain `number[]` parameters and callbacks that observe index/source-array arguments
remain split because they do not prove the value-only dense-source perimeter. Other columns
are open candidates until they have their own evidence producer, focused replay, hard
negatives, and PCF/readiness admission.

| fact | Python `all(...)` + loop | TypeScript `every` + loop | JavaScript `every` + loop | Ruby `Enumerable#all?` | Rust `Iterator::all` |
|---|---|---|---|---|---|
| `quantifier.universal.counterexample-loop` | modeled-controlled; packet admitted | admitted for dense-literal one-arg every/for-of; number[] param and callback extra args stay split | open | open | open |
| `quantifier.vacuous-truth` | modeled-controlled; packet admitted | admitted for fallthrough true | open | open | open |
| `iteration.same-source-identity` | modeled-controlled; packet admitted | admitted for same dense-literal source | open | open | open |
| `effect.pure-predicate` | modeled-controlled; packet admitted | admitted for pure comparison predicates | open | open | open |
| `boolean.demorgan.proven-bool-operands` | modeled-controlled; packet admitted | admitted for boolean comparison results; value-returning && remains closed | open | open | open |

Registry entries guide implementation work; they are not detector admission.
