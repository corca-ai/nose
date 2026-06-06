# Type-4 adversarial coverage

Back to [home](home.md). This page is the operating guide for the agent-facing
Type-4 coverage loop. It complements the synthetic [type4-benchmark](type4-benchmark.md)
and the real-corpus [frontier-platform](frontier-platform.md).

## Purpose

Type-4 coverage should grow by co-evolution, not by a pile of ad hoc canons. The
engine, oracle, proof facts, adversarial cases, and performance guards should
pressure each other:

```text
generator/cases attack -> engine tries to converge positives
  -> hard negatives and oracle refute unsafe merges
  -> proof/type facts gate risky generalization
  -> performance guards reject expensive representations
  -> matrix records the state
  -> the next agent task is selected from the matrix
```

The harness is intentionally not a fully automatic optimizer. It gives a coding
agent enough structure to choose the next item, reproduce the current state, make
the smallest correct engine/oracle/proof/perf change, and record the result.

## Files

The control plane lives in [`bench/type4/adversarial`](../bench/type4/adversarial/):

| file | role |
|---|---|
| `coverage_matrix.v1.json` | semantic-family cells, status, next actor, cases, gates, docs |
| `rule_registry.v1.json` | engine/oracle/proof/perf rules and their boundaries |
| `cases/cases.v1.json` | positive, hard-negative, oracle-gap, and perf case handles |
| `scripts/type4-next` | print the next actionable task cards |
| `scripts/type4-check` | validate matrix/registry/case consistency |
| `scripts/type4-report` | summarize backlog by status, action, and family |
| `scripts/type4-ingest-leads` | summarize `nose verify --leads` output and emit draft matrix cells |

Run the basic harness check:

```sh
bench/type4/adversarial/scripts/type4-check
bench/type4/adversarial/scripts/type4-report
bench/type4/adversarial/scripts/type4-next --limit 3
bench/type4/adversarial/scripts/type4-ingest-leads /tmp/leads.json --draft-json
```

## Status Model

Each matrix cell has one status. The status says which actor should move next:

| status | next work |
|---|---|
| `covered` | monitor only; positives converge, hard negatives split, gates passed |
| `candidate` | add focused reproductions and classify the cell |
| `under-merged` | engine/value graph/idiom work: behavior-equal positives split |
| `false-merged` | soundness bug hunt: oracle refuted a fingerprint-equal pair |
| `oracle-blocked` | interpreter/oracle must learn enough behavior before engine work is safe |
| `proof-fact-blocked` | type/provenance/order facts are missing |
| `perf-blocked` | semantics are plausible but representation/runtime cost is too high |
| `unsafe` | semantics are too broad or edge cases are unresolved |
| `not-applicable` | intentionally out of scope |

`type4-next` scores actionable cells. Soundness bugs rank first, then
under-merged engine work, then oracle/proof-fact blockers, then survey
candidates and performance blockers.

## Agent Loop

For a selected task card:

1. Read the matrix cell, referenced rules, referenced cases, and docs.
2. Add or confirm focused positive cases and adjacent hard negatives.
3. Reproduce the current status. Do not implement a canon from speculation.
4. Fix the actor named by `next_action`:
   - `engine`: frontend lowering, idiom canonicalization, value graph, or shared representation;
   - `oracle`: interpreter behavior and `nose verify` coverage;
   - `proof-facts`: type/provenance/order facts that make a canon safe;
   - `performance`: compact representation, runtime guard, or scan-regression harness;
   - `soundness`: remove or gate a false merge.
5. Run the cell's focused gates, then the normal project gate.
6. Update the matrix, registry, cases, and docs in the same PR.

The ordinary PR/push gate remains:

```sh
./scripts/check-ci-local.sh --fast
```

For Type-4 cells, also run the focused command listed in the cell. When a cell
has a generated benchmark axis, prefer:

```sh
GATE=focused AXIS=<axis> scripts/type4-smoke.sh
```

## Rule Registry

The registry is the anti-duplication mechanism for the semantic engine. Every new
rule should declare:

- whether it is engine, oracle, proof-fact, or performance work;
- implementation files;
- positive cases and hard negatives;
- boundaries where the rule must fail closed;
- docs that explain the rule.

When two cells need the same proof invariant or value-graph representation, extend
one registry rule instead of adding parallel language-specific exceptions.
For option-producing iterator rules, record the absence/value boundary explicitly:
the current oracle `FilterMap` model treats callback-level `Null` as absence,
propagates `Err`, and emits every other value, including falsey values such as `0`.
The covered engine slice recognizes direct Rust `if p { Some(v) } else { None }`
filter-map callbacks, match-guard option callbacks, pure `Some(x).and_then(...)`
helper chains, and guarded `Vec::new()`/`push` builders as filtered maps; mapped
`None` payloads, wrapped `Some(None)` emitted payloads, changed `Some` values,
truthy filtering after `Some(0)`, and effectful callbacks stay hard-negative or
fail-closed boundaries.
For Java stream rules, keep the registry scoped to the proven pure subset:
`Arrays.stream(...).flatMap(...map...)` can share the FlatMap HoF only while
`map`-returning-stream siblings stay nested and callback effects remain observable
oracle evidence instead of purity assumptions.
For FlatMap aggregate rules, keep the aggregate consumer explicit about the HoF
layout: pure `FlatMap[outer, Map[contrib]]` streams and equivalent nested inner
`Reduce` loops can share sum/max/any fingerprints when `contrib` uses the outer
element, but the bridge must not read FlatMap's `[outer, inner]` arguments as
filtered Map's `[contrib, pred]`. Wrong sum seeds, nested-list aggregation,
outer-cardinality-only cases, and changed flattened predicates remain hard
negatives; filtered Sum/Reduce FlatMap aggregates, method-terminal Any/All
predicates, and filtered nested early-return any/all loops preserve carried
outer/inner predicates.
For numeric clamp rules, require a concrete integer-domain and `lo <= hi` proof;
proof-backed min/max compositions, two-comparison ternaries, and proven numeric
library clamp methods share `Clamp(x, lo, hi)`. Name-only lower/upper conventions,
method names without numeric receiver proof, non-exiting checks, swapped bounds,
and float domains must stay hard negatives.
For map-default import rules, resolve only unambiguous static imported bindings whose
provider has one safe immutable literal or proven map-factory binding. The covered import
slice includes Python sibling literals, JS/TS named imports from sibling map exports, Java
static imports from class `Map.of` fields, and Rust `use` imports of const entry arrays
consumed by `HashMap::from`/`BTreeMap::from`. Provider mutation, importer mutation,
duplicate providers, shadowing, unresolved imports, unsupported coordinates, and changed
receiver maps remain fail-closed hard negatives or successor work.

## Adversarial Cases

Every positive family needs adjacent negatives. The case library stores handles,
not necessarily full generated source. A case can point to existing fixtures,
future generated manifest items, or a real frontier packet. Good hard negatives
attack exactly the proof invariant a rule needs:

- flattened list vs nested list;
- changed predicate or mapped value;
- wrong collection/key/default coordinate;
- missing type/provenance/order proof;
- filter-map absence vs emitted falsey value;
- Java stream `flatMap` vs `map` returning streams;
- FlatMap aggregate seed/predicate changes and nested-list aggregation;
- effectful callback where a pure HoF rule would be unsound;
- representation growth that makes a coverage win too expensive.

## Relationship To Existing Type-4 Tools

- `bench/type4/generate.py` creates evidence-carrying synthetic pairs.
- `scripts/type4-smoke.sh` runs generated positives, hard negatives, verifier leads,
  stats, and frontier summaries.
- `bench/type4/frontier_platform.py` ranks real-corpus axes by breadth and evidence.
- The adversarial harness turns those signals into agent task cards and a durable
  coverage ledger.

The loop should normally start from `type4-next`, but real-code evidence from the
frontier platform or `nose verify --leads` can add or update matrix cells. Use
`type4-ingest-leads` to turn verifier lead output into draft cells, then manually
classify the semantic family, positives, hard negatives, and gates before committing
the matrix update.
