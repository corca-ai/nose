# Type-4 focused cases

This page describes the small focused-case library that supports the Type-4 target-packet
workflow. The cross-axis generalization of this adversarial method (attacker → assessor →
defender over every claim nose makes, not just Type-4 recall) is the [adversarial co-evolution runbook](adversarial-coevolution.md).

## Current role

The active Type-4 planning path is now:

```text
frontier_platform.py
  -> real_frontier.v1.json evidence
  -> frontier_target_packets.v1.json implementation-ready target packets
  -> proof_carrying_frontier.v1.json admission readiness
  -> frontier_readiness.md roadmap triage
  -> scripts/type4-smoke.sh / nose verify / focused tests
```

`bench/type4/adversarial` is no longer the source of truth for next work. The former
adversarial ledger was retired after each entry had a current gate in tests, `type4-smoke`,
focused verifier checks, or `query_regression`.

What remains is intentionally smaller:

| file | role |
|---|---|
| `cases/cases.v1.json` | focused positive and hard-negative case handles |
| `cases/cases.v1.json::hard_negative_groups` | packet-level positive/negative/gate linkage |
| `cases/**` | small fixture corpora used by focused query gates, focused verifier checks, or boundary documentation |
| `scripts/type4-check` | validate target packets, real-frontier links, and focused cases |
| `scripts/type4-exec-check` | execute focused-case `nose query` expectations |
| `scripts/type4-next` | print next task cards from `frontier_target_packets.v1.json` |
| `scripts/type4-report` | summarize target packets and focused case coverage |
| `scripts/type4-ingest-leads` | turn `nose verify --leads` JSON into draft target packets |

Run the basic checks:

```sh
bench/type4/adversarial/scripts/type4-check
NOSE_BIN=target/debug/nose bench/type4/adversarial/scripts/type4-exec-check
bench/type4/adversarial/scripts/type4-report
bench/type4/adversarial/scripts/type4-next --limit 3
```

`type4-check` is the structural gate and intentionally does not require a built `nose`
binary. `type4-exec-check` is the executable witness gate. It runs declared focused-case
expectations through `nose query` and fails when a pair expected to be `same-family` is
split, or when a pair expected to stay `split` is merged. CI runs this gate with the
release binary after build, so packet perimeters can no longer drift as manifest-only
strings.

## Target packets

The next-work queue is `bench/type4/frontier_target_packets.v1.json`, not a separate
ledger. A packet links human evidence in `real_frontier.v1.json`, names the proof invariant,
records hard-negative siblings, and routes the work with `owner_route`.

`type4-next` is a thin reader over those packets. It does not infer work from raw prevalence
or from the retired ledger:

```sh
bench/type4/adversarial/scripts/type4-next
bench/type4/adversarial/scripts/type4-next --route proof-fact-prerequisite --json
```

Before opening exact semantic behavior from a target packet, check the
[frontier readiness artifact](../bench/type4/frontier_readiness.md) and the
[proof-carrying frontier](proof-carrying-frontier.md) report:

```sh
python3 bench/type4/proof_carrying_frontier.py --check
```

The readiness artifact is the compact roadmap view. It keeps target-packet routing
separate from admission readiness, names the next work item, and repeats stable wording for
release notes. For example, numeric clamp is `admitted/resolved` only for its controlled
detector slice; the current cross-language pair still lacks fzf-side bound-order evidence
and shared integer-domain evidence. The Python loop plus De Morgan packet is
`blocked-on-proof` until its universal-loop, boolean-only De Morgan, and effect-safety
facts exist.

## Focused cases

Every positive family needs adjacent negatives. The case library stores handles, not a
parallel rule catalog. A case can point to checked-in fixtures, generated manifest items, or
real frontier evidence. Important cases should be promoted into an automatic gate:

- Rust or CLI equivalence tests for stable semantic rules;
- `scripts/type4-smoke.sh` focused gates for generated positives and hard negatives;
- `nose verify --max-violations 0 <focused-corpus>` for named oracle-backed behavior
  checks; not every directory under `cases/**` is intended to pass as a standalone
  zero-violation verifier corpus;
- `query_regression compare` for product output/runtime and HoF value-graph budget checks;
- formal obligations where a proof precondition is the boundary.

Cases that are stable enough for direct detector replay can add
`executable_expectations`. Each expectation names a fixture path, two or more members, and
whether those members must currently be `same-family` or `split`. Positive frontier cases
can still declare `split` while they are blocked-on-proof; that makes the current miss
explicit and gives the admission PR a concrete expectation to flip when the proof lands.

If a focused case is not used by a gate and does not clarify a target packet boundary, it is
only historical context and should be deleted instead of preserved.

Target packets now cite `hard_negative_groups`. A group binds the packet's positive focused
cases, hard-negative focused cases, and regression gates together, then labels the perimeter
with convention IDs. The convention categories are `numeric`, `boolean`, `loop`,
`collection`, and `protocol-boundary`; `proof_carrying_frontier.py --check` fails if a
packet omits its group, if a group case has the wrong kind, or if detector admission cites a
positive without the group's hard-negative gates.

Convention glossary:

| convention | boundary to prove or keep closed |
|---|---|
| `numeric.domain` | integer/finite numeric evidence, especially excluding float/NaN-sensitive behavior |
| `numeric.precondition` | required order, range, or nonzero preconditions |
| `numeric.shape` | operator nesting, bound coordinate, and wrong-literal shape |
| `boolean.truth-table` | predicate-coordinate and truth-table preserving rewrites |
| `boolean.value-context` | languages where logical operators can return operand payloads |
| `boolean.effect-safety` | predicate calls, overloads, mutation, or observed effects |
| `loop.empty-input` | empty iterable/collection result such as vacuous truth |
| `loop.short-circuit` | first-failing element, break/continue, and stopping-time behavior |
| `loop.iterator-identity` | same receiver/source iterable and no receiver mutation during traversal |
| `collection.cardinality` | flat vs nested shape, dropped vs kept elements, and aggregation seed behavior |
| `collection.absence-vs-value` | absent item vs emitted null/None/falsey payload |
| `collection.receiver-provenance` | map/set/list identity, imported literal provenance, and key/default coordinates |
| `protocol-boundary.api-identity` | library/member identity, custom method names, receiver type, and imports |
| `protocol-boundary.lifecycle` | settlement, cancellation, scheduling, channel, or runtime lifecycle state |
| `protocol-boundary.callback-effect` | callback mutation, ordering, exceptions, and externally observed effects |

The `python.loop_demorgan_all` focused group clarifies the README-facing same-language
packet. Its positive fixture captures `all(x != 0 and x != 1 for x in xs)` versus the
early-return counterexample loop; hard negatives cover vacuous truth, observed effects,
Python value-returning boolean operands, changed predicates, and different iterable
identity.

Good hard negatives attack exactly the proof invariant a rule needs:

- flattened list vs nested list;
- changed predicate or mapped value;
- wrong collection/key/default coordinate;
- missing type/provenance/order proof;
- filter-map absence vs emitted falsey value;
- Java stream `flatMap` vs `map` returning streams;
- FlatMap aggregate seed/predicate changes and nested-list aggregation;
- effectful callback where a pure HoF rule would be unsound;
- deep/wide generated HoF chains where representation growth or query time makes a coverage
  win too expensive.

## Verifier leads

`nose verify --leads <file>` exports under-merged behavior-equal pairs. These are not target
packets yet. Use:

```sh
bench/type4/adversarial/scripts/type4-ingest-leads leads.json --axis <axis> --draft-json
```

The output is a draft packet skeleton for manual curation. Before committing it, add or link
human evidence in `real_frontier.v1.json`, classify the proof invariant, record adjacent hard
negatives, and add a focused gate.

For semantic-kernel PRs, prefer also writing
[`recall-loss diagnostics`](recall-loss-diagnostics.md) with
`nose verify <path> --max-violations 0 --recall-loss-report <file>`. It embeds
the under-merge signal and adds soundness gate numbers, oracle exclusions, and
structured exact-admission rejection buckets.

## Relationship to existing Type-4 tools

- `bench/type4/generate.py` creates evidence-carrying synthetic pairs. It is the
  stable CLI/import entry point; focused generator internals live under
  `bench/type4/type4gen/`.
- `scripts/type4-smoke.sh` runs generated positives, hard negatives, verifier leads, stats,
  and frontier summaries.
- `bench/type4/frontier_platform.py` ranks real-corpus axes by breadth and evidence, then
  emits implementation-ready target packets.
- `bench/type4/query_regression/` guards product semantic query output, runtime, fragment
  buckets, and HoF value-graph budgets.
- `bench/type4/adversarial/cases` keeps small focused fixtures only when they support those
  gates or target packets.
