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
| `python-loop-demorgan.boolean-demorgan` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Requires universal short-circuit, effect safety, and iterator identity. |
| `python-loop-demorgan.effect-safety` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only permits comparing short-circuit boundaries after effects are closed. |
| `python-loop-demorgan.iterator-identity` | `modeled-controlled` | `focused-executable`, `source-evidence` | Only rules out different receiver/iterator inputs. |
| `python-loop-demorgan.universal-short-circuit` | `specified-not-modeled` | `focused-executable`, `source-evidence` | Requires boolean De Morgan, effect safety, and iterator identity. |

Registry entries guide implementation work; they are not detector admission.
