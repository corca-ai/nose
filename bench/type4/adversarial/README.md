# Type-4 focused cases and target packets

This directory keeps the small focused case fixtures that support Type-4 target packets and
regression gates. The former adversarial ledger has been retired: next work now comes from
`../frontier_target_packets.v1.json`, backed by `../real_frontier.v1.json` evidence and the
generated benchmark gates.

What remains here is deliberately small:

- `cases/cases.v1.json` records focused positive, hard-negative, oracle-gap, and perf case
  handles used by tests, `nose verify`, `type4-smoke`, scan regression, or target packets.
- `cases/**` stores the small focused fixture corpora referenced by those handles.
- `scripts/type4-next` prints task cards from `../frontier_target_packets.v1.json`.
- `scripts/type4-check` validates target packets, real-frontier evidence links, and focused
  case fixture paths.
- `scripts/type4-report` summarizes target packets and focused cases.
- `scripts/type4-ingest-leads` summarizes `nose verify --leads` JSON and emits draft target
  packet skeletons for manual curation.

## Basic loop

```sh
bench/type4/adversarial/scripts/type4-check
bench/type4/adversarial/scripts/type4-next --limit 3
bench/type4/adversarial/scripts/type4-report
```

`type4-next` does not infer new work from this directory. It reads target packets generated
by the frontier platform. When `nose verify --leads` produces leads, run
`scripts/type4-ingest-leads <leads.json> --axis <axis> --draft-json`; promote the draft only
after adding real-frontier evidence, hard negatives, and a focused gate.
