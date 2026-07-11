# Current missed-worthy frontier (#816)

This audit selects the next product-recall tranche from the current
source-independent worthy pool. It supersedes the 2026-06-10 recall-ceiling
snapshot for current planning, while preserving that older run as historical
evidence. The result is deliberately narrower than “add more matching”: a
substantial cohort already passes raw matching and is lost during family
grouping or presentation.

## Protocol

The audit kept the v5 multi-source pool as the only worthy-recall denominator.
The v6 current-top-10 overlay remained precision-only. The order of operations
was fixed in commits:

1. Commit the collector, deterministic selector, validators, and self-tests.
2. From that clean commit, build nose 0.18.0 and freeze the current artifact.
3. Freeze a language-stratified dev sample before reading its source.
4. Record raw extraction/candidate/accepted stages for every dev miss.
5. Audit only the 35 selected dev families and commit the dev Route A proposal.
6. Pre-register a held-out gate, then run the same mechanical stage check once.
7. Price the current product baseline with the #809 same-binary control.

No held-out source received a human judgment, and no threshold or selection rule
changed after dev review.

## Reproduction

Use the [`recall_ceiling_probe_2026_07_11.v2.json` reproduction record](../bench/labels/recall_ceiling_probe_2026_07_11.v2.json) as the checked artifact.
It records the exact command, source commit, clean-tree state, nose version and
binary hash, v5/v6/evaluation/corpus/prune/query-schema hashes, all 120 repository
pins, per-repository query hashes, feature-run hashes, source hashes, and failures.

```sh
cargo build --release -p nose-cli
python3 bench/labels/recall_ceiling_probe.py \
  --nose target/release/nose \
  --repos-root bench/repos \
  --recall-labelset bench/labels/refactoring_families.v5.json \
  --precision-labelset bench/labels/refactoring_families.v6.json \
  --evaluation-report bench/labels/product_quality_evaluation_2026_07_11.v2.json \
  --corpus-manifest bench/goldens/corpus.json \
  --prune-manifest bench/labels/prune_manifest.json \
  --json-out bench/labels/recall_ceiling_probe_2026_07_11.v2.json
```

The arm-1 query exactly reproduces the checked evaluator:

| split | worthy | arm-1 hit | miss | sub-DAG ceiling | inline | same-unit | unrecovered | extraction/other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dev | 2,849 | 2,626 | 223 | 117 | 17 | 17 | 48 | 24 |
| held-out | 2,091 | 1,949 | 142 | 71 | 5 | 10 | 15 | 41 |

At the shipped weight-20 floor the optimistic sub-DAG counts are 59 dev and 40
held-out. These are multiset-intersection ceilings, not connected witnesses.

## Frozen dev selection

The selection SHA-256 is
`7bd95e47fa0191249e20a7bddb527868074a1531abe6fe2c0eb708f14cafb461`.
It chooses five misses from each of C, Go, Java, Python, Ruby, Rust, and
TypeScript. One candidate from every residual lane is reserved first, then each
language is filled by preferring weight-20 sub-DAG ceilings and distinct
repositories. Selection uses label metadata and probe measurements only.

Use the [`missed_worthy_audit_decisions_2026_07_11.dev.v1.json` dev audit](../bench/labels/missed_worthy_audit_decisions_2026_07_11.dev.v1.json) for the 35 checked source decisions.
Every row links the frozen candidate hash to source lines, a blocker rationale,
and its smallest sound invariant.

| blocker | selected dev families | interpretation |
| --- | ---: | --- |
| family folding / overlap matching | 18 | A direct raw pair is already accepted, but final query covers neither labeled endpoint. |
| candidate generation | 4 | Coherent source shape exists, but no direct structural candidate is emitted. |
| connected anchor / sub-DAG construction | 3 | A candidate exists, but the optimistic mass is not yet an accepted connected witness. |
| unit extraction / missing kind | 4 | Pytest decorator tables, Rust tests with local items, or nested TS test windows are absent as relevant units. |
| same-unit actionable fragment | 2 | Repeated table/subtest windows live inside one large enclosing function. |
| no coherent general mechanism | 4 | The ceiling is class-wide, inverse-table, call-plumbing, or effect-mismatched evidence. |

There is no direct pure-callback HOF cohort. In particular, the Rust `mul` versus
`imul` test pair crosses returning versus in-place mutation, so it cannot justify
callback-purity work. Issue #806 therefore remains deferred.

## Stage evidence

The dev-only [`missed_worthy_stage_audit_2026_07_11.dev.v1.json` stage artifact](../bench/labels/missed_worthy_stage_audit_2026_07_11.dev.v1.json)
runs the raw structural candidate surface once per repository and joins every one
of the 223 misses to extracted units, direct candidate edges, and direct accepted
pairs:

| dev stage | families |
| --- | ---: |
| direct accepted pair | 51 |
| candidate only | 41 |
| extracted, no direct candidate | 96 |
| missing unit | 35 |

The 51 accepted pairs are a conservative lower bound: the diagnostic does not
include query's syntax channel or additional shape-candidate arm. They span all
seven dev languages. The source sample confirms 18/18 selected accepted-pair rows
as coherent refactoring families rather than optimistic mass accidents.

After the dev proposal was committed, the pre-registered held-out gate required
at least 15 accepted pairs in at least three languages. The one-time mechanical
confirmation [`missed_worthy_stage_confirmation_2026_07_11.heldout.v1.json` artifact](../bench/labels/missed_worthy_stage_confirmation_2026_07_11.heldout.v1.json)
passed with 42/142 accepted pairs across C, Go, Java, Python, Rust, and TypeScript.
Ruby had no accepted-pair row in that held-out snapshot. This confirms the stage
mechanism without using held-out source to tune it.

## Decision: Route A

The selected next tranche is follow-up issue
[#817](https://github.com/corca-ai/nose/issues/817): preserve accepted-pair
endpoint coverage through union-find clustering, same-file site collapse, family
subsumption, and query presentation.

The smallest invariant is:

> Every direct accepted pair with two distinct, non-nested source sites is
> covered by a final query family that overlaps both endpoints, or by explicit
> suppression provenance pointing to such a covering family.

Pair coverage must not turn transitive A-B and B-C edges into an unsupported A-C
equivalence or one misleading all-member skeleton. Nested sites remain one
refactoring location, existing covering families suppress redundant pair rows,
dense components remain bounded rather than O(n²), and generated/ignored/surface
policy stays effective.

This is post-acceptance grouping work. It does not lower a threshold, change
ranking/default policy, or promote a new exact Type-4 law. The proof-carrying
frontier therefore remains at zero ready-for-defender packets.

## Product and runtime price

The #809 same-binary pricing run covers one source-backed repository per dev
language: curl, prometheus, netty, rich, rubocop, nushell, and axios. Its checked
primary/control/status artifacts are adjacent to the label artifacts; see the [compact pricing summary](../bench/labels/missed_worthy_grouping_pricing_2026_07_11.summary.md) for the result.

| current semantic product baseline | value |
| --- | ---: |
| raw families | 1,263 |
| default families | 415 |
| hidden families | 795 |
| divergence families | 53 |
| aggregate median | 2,503.17 ms |
| output drift / adjusted runtime delta | 0 / 0 (same-binary control passed) |

Those seven repositories contain 25 of the 51 dev accepted-pair misses. Emitting
one new row for every uncovered pair would therefore be a deliberately loose
upper bound of +25 raw families (+1.98%) and +25 default families (+6.02%) on the
slice. #817 must do better by reusing covering families and applying normal
surface policy. Because #816 does not implement the head behavior, the actual
base/head output and runtime delta remains a mandatory #817 gate: exact output
declaration, alternating order, same-binary control, and a focused rerun for a
signal above both 5% and 5 ms.

## Rejected routes

- **B — extraction first:** 4/35 source decisions and 35/223 dev stage rows are
  real extraction evidence, but they are smaller and heterogeneous (pytest
  metadata, local-item Rust tests, nested TS tests) than the accepted-pair loss.
- **C — same-unit fragment first:** 2/35 selected rows are actionable fragments;
  they do not dominate, and tiny fragment output still needs its own product
  policy.
- **D — HOF roadmap:** no direct pure-callback miss survived source review.
  `mul`/`imul` changes mutation and ownership, so #794-#797 are not opened.
- **E — no material mechanism:** rejected because 51 dev and 42 held-out misses
  already have direct accepted raw pairs, with 18 source-coherent selected cases.

## What happens after #817

- If bounded endpoint coverage recovers a material dev cohort and passes hard
  negatives plus #809 output/runtime gates, merge it, rerun this frontier, and
  select the next tranche only from the remaining misses.
- If it requires pair explosion, misleading transitive families, or unacceptable
  default noise/runtime, record a no-go for grouping work and open the second
  split-safe precision-label refresh over the remaining 380/1,200 unmatched
  current top-10 positions.
- If tracing isolates a smaller deterministic defect, reduce #817 to that defect;
  do not broaden detector admission.

## Validation commands

```sh
python3 bench/labels/recall_ceiling_probe.py --self-test
python3 bench/labels/recall_ceiling_probe.py \
  --validate bench/labels/recall_ceiling_probe_2026_07_11.v2.json
python3 bench/labels/missed_worthy_stage_audit.py \
  --validate bench/labels/missed_worthy_stage_audit_2026_07_11.dev.v1.json
python3 bench/labels/recall_ceiling_probe.py \
  --validate-decisions bench/labels/missed_worthy_audit_decisions_2026_07_11.dev.v1.json \
  --artifact bench/labels/recall_ceiling_probe_2026_07_11.v2.json
python3 bench/labels/missed_worthy_heldout_confirmation.py \
  --validate bench/labels/missed_worthy_stage_confirmation_2026_07_11.heldout.v1.json
python3 scripts/check-query-regression.py \
  bench/labels/missed_worthy_grouping_pricing_2026_07_11.primary.v1.json \
  --same-binary-control bench/labels/missed_worthy_grouping_pricing_2026_07_11.control.v1.json \
  --require-same-binary-control \
  --max-runtime-delta-pct 5 \
  --min-runtime-delta-ms 5
python3 bench/labels/recall_ceiling_probe.py \
  --validate-closeout bench/labels/missed_worthy_frontier_closeout_2026_07_11.v1.json
```
