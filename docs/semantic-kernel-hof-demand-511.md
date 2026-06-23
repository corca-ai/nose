# Semantic kernel HOF demand 511

Status: issue #511 R5 implementation record.

Source artifacts:

- [hof_demand_materialization_matrix.v1.json](../bench/semantic_pack/hof_demand_materialization_matrix.v1.json)
- [external_authorability_matrix.v1.json](../bench/semantic_pack/external_authorability_matrix.v1.json)
- [kernel_capability_matrix.v4.json](../bench/semantic_pack/kernel_capability_matrix.v4.json)

## Goal

R5 reviews the high-risk boundary left after R4: higher-order functions,
demand/effect timing, and materialization.

The result is intentionally conservative. The kernel already has enough
vocabulary to describe narrow admitted lanes, so R5 does not add a new
primitive. It keeps broad HOF result-domain claims closed.

## Accepted Lanes

The accepted lanes are narrow:

| lane | status | boundary |
|---|---|---|
| Python list comprehension HOF demand | profile-only | describes eager callback demand, not arbitrary library result shape |
| JS-like array HOF demand | profile-only | describes timing, not Lodash, RxJS, or callback purity |
| Rust/Java pull-lazy HOF demand | profile-only | describes delayed callback effects, not terminal materialization |
| `Promise.then` | accepted narrow | requires exact `PromiseLike` receiver and admitted API occurrence |
| iterator identity adapters | accepted narrow | requires exact iterable receiver and supported zero-argument adapter shape |

These lanes reuse existing `DemandEffectProfile`, `DomainEvidence`, and
`LibraryApi.Contract` evidence. They do not create a generic HOF result-domain
rule.

## Closed Boundaries

The matrix keeps these shapes blocked or rejected:

- generic HOF fixed result domains;
- Lodash and RxJS collection operators;
- Rust `itertools::collect_vec`;
- caller-selected `collect` materialization;
- Java Stream terminal collectors;
- Python `itertools.chain` and one-shot iterator reuse;
- `Map.get` fixed value-domain claims;
- external HOF result-domain influence through metadata.

The hard negatives are grouped by laziness, callback effects, repeated demand,
async boundaries, scheduler behavior, lifecycle behavior, and materialization.

## Primitive Decision

R5 adds zero primitives.

The important distinction is this:

- `DemandEffectProfile` can describe timing once an operation is already
  admitted.
- It cannot prove that a raw selector is a safe API occurrence.
- It cannot prove terminal materialization.
- It cannot prove external trait identity, stream lifecycle, scheduler behavior,
  or parametric result domains.

Those remaining blockers need producers, runtime, or trust gates. Adding another
kernel primitive would hide the missing evidence instead of solving it.

## Product And Performance Gates

R5 changes artifacts and documentation only. It does not alter query,
normalization, value-graph, exact, or detection consumers, so no product-output
drift is expected.

The R5 matrix records no semantic hot-path change, so the 10% runtime gate does
not require a new benchmark in this round.

## Transition

R5 is complete enough to move to R6:

- broad generic HOF result domains remain rejected;
- narrow existing lanes are documented;
- materialization and lifecycle blockers are explicit;
- external influence remains closed.

R6 closes #511 by recording the final primitive set, builtin expansion,
external-pack readiness, remaining blockers, and validation requirements.

Back to [semantic-kernel](semantic-kernel.md).
