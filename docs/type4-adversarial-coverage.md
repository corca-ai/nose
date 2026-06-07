# Type-4 focused cases

Back to [home](home.md). This page describes the small focused-case library that supports
the Type-4 target-packet workflow.

## Current role

The active Type-4 planning path is now:

```text
frontier_platform.py
  -> real_frontier.v1.json evidence
  -> frontier_target_packets.v1.json implementation-ready target packets
  -> scripts/type4-smoke.sh / nose verify / focused tests
```

`bench/type4/adversarial` is no longer the source of truth for next work. The former
adversarial ledger was retired after each entry had a current gate in tests, `type4-smoke`,
focused verifier checks, or `scan_regression`.

What remains is intentionally smaller:

| file | role |
|---|---|
| `cases/cases.v1.json` | focused positive and hard-negative case handles |
| `cases/**` | small fixture corpora used by focused `scan` gates, focused verifier checks, or boundary documentation |
| `scripts/type4-check` | validate target packets, real-frontier links, and focused cases |
| `scripts/type4-next` | print next task cards from `frontier_target_packets.v1.json` |
| `scripts/type4-report` | summarize target packets and focused case coverage |
| `scripts/type4-ingest-leads` | turn `nose verify --leads` JSON into draft target packets |

Run the basic checks:

```sh
bench/type4/adversarial/scripts/type4-check
bench/type4/adversarial/scripts/type4-report
bench/type4/adversarial/scripts/type4-next --limit 3
```

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

## Focused cases

Every positive family needs adjacent negatives. The case library stores handles, not a
parallel rule catalog. A case can point to checked-in fixtures, generated manifest items, or
real frontier evidence. Important cases should be promoted into an automatic gate:

- Rust or CLI equivalence tests for stable semantic rules;
- `scripts/type4-smoke.sh` focused gates for generated positives and hard negatives;
- `nose verify --max-violations 0 <focused-corpus>` for named oracle-backed behavior
  checks; not every directory under `cases/**` is intended to pass as a standalone
  zero-violation verifier corpus;
- `scan_regression compare` for product output/runtime and HoF value-graph budget checks;
- formal obligations where a proof precondition is the boundary.

If a focused case is not used by a gate and does not clarify a target packet boundary, it is
only historical context and should be deleted instead of preserved.

Good hard negatives attack exactly the proof invariant a rule needs:

- flattened list vs nested list;
- changed predicate or mapped value;
- wrong collection/key/default coordinate;
- missing type/provenance/order proof;
- filter-map absence vs emitted falsey value;
- Java stream `flatMap` vs `map` returning streams;
- FlatMap aggregate seed/predicate changes and nested-list aggregation;
- effectful callback where a pure HoF rule would be unsound;
- deep/wide generated HoF chains where representation growth or scan time makes a coverage
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

## Relationship To Existing Type-4 Tools

- `bench/type4/generate.py` creates evidence-carrying synthetic pairs.
- `scripts/type4-smoke.sh` runs generated positives, hard negatives, verifier leads, stats,
  and frontier summaries.
- `bench/type4/frontier_platform.py` ranks real-corpus axes by breadth and evidence, then
  emits implementation-ready target packets.
- `bench/type4/scan_regression/` guards product semantic scan output, runtime, fragment
  buckets, and HoF value-graph budgets.
- `bench/type4/adversarial/cases` keeps small focused fixtures only when they support those
  gates or target packets.
