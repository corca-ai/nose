# Type-4 adversarial coverage harness

This directory is the agent-facing control plane for Type-4 co-evolution. The
generated benchmark in `bench/type4/` asks whether exact semantic detection
covers a synthetic manifest; this harness asks which semantic gap an engineer or
coding agent should work on next.

It is deliberately small and mostly data-driven:

- `coverage_matrix.v1.json` records semantic-family cells, their current status,
  required engine/oracle/proof/perf work, hard negatives, and focused gates.
- `rule_registry.v1.json` records the engine/oracle/proof/perf rules that cells
  depend on, including implementation locations and boundaries.
- `cases/cases.v1.json` records positive, hard-negative, and oracle-gap case
  handles used by matrix cells.
- `scripts/type4-next` prints the next actionable task cards.
- `scripts/type4-check` validates matrix/registry/case consistency.
- `scripts/type4-report` summarizes backlog shape.
- `scripts/type4-ingest-leads` summarizes `nose verify --leads` JSON and emits
  draft matrix cells for manual curation.

## Agent loop

```sh
bench/type4/adversarial/scripts/type4-check
bench/type4/adversarial/scripts/type4-next --limit 3
bench/type4/adversarial/scripts/type4-report
bench/type4/adversarial/scripts/type4-ingest-leads /tmp/leads.json --draft-json
```

For a selected task, the agent should:

1. add or confirm focused positives and adversarial negatives;
2. reproduce the current matrix state (`under-merged`, `oracle-blocked`, etc.);
3. fix the correct actor: engine, oracle, proof facts, or performance;
4. run the cell's focused gates and the ordinary repo gates;
5. update the matrix, registry, cases, and docs in the same PR.

The status values intentionally separate actors:

| status | meaning |
|---|---|
| `candidate` | plausible gap, not yet reproduced enough to classify |
| `covered` | positives converge, hard negatives split, oracle/gates are satisfied |
| `under-merged` | behavior-equivalent positives do not yet converge |
| `false-merged` | fingerprint-equal code was refuted by the oracle |
| `oracle-blocked` | engine work cannot be accepted until the oracle can judge the case |
| `proof-fact-blocked` | a safe rule needs more type/provenance/order facts |
| `perf-blocked` | the semantic rule is plausible but representation/runtime cost is too high |
| `unsafe` | semantics are too broad or language-specific edge cases are unresolved |
| `not-applicable` | the family/surface combination is intentionally out of scope |

The highest-priority ordinary work is usually `under-merged`, but `false-merged`
always wins because it is a soundness bug.
