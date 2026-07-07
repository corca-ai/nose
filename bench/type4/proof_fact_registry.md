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
