# Semantic kernel expansion closeout 511

Status: issue #511 R6 closeout record.

Source artifacts:

- [kernel_expansion_closeout.v1.json](../bench/semantic_pack/kernel_expansion_closeout.v1.json)
- [hof_demand_materialization_matrix.v1.json](../bench/semantic_pack/hof_demand_materialization_matrix.v1.json)
- [external_authorability_matrix.v1.json](../bench/semantic_pack/external_authorability_matrix.v1.json)
- [kernel_capability_matrix.v4.json](../bench/semantic_pack/kernel_capability_matrix.v4.json)
- [kernel_capability_matrix.v3.json](../bench/semantic_pack/kernel_capability_matrix.v3.json)

## Goal

Issue #511 was created to keep improving the semantic kernel before widening
semantic packs. The target was not to add imagined mechanisms. The target was to
put enough real material into the kernel so meaningful external packs can be
authored later, while keeping the exact channel fail-closed.

The issue is complete when the R6 PR merges.

## Completed Work

The issue completed two R1-R3 cycles, R4, R5, and this R6 closeout.

| phase | result |
|---|---|
| R1-R3 cycle 1 | generalized admitted API fixed result-domain materialization across builtin rows |
| R1-R3 cycle 2 | added strict external fixed result-domain authoring validation while keeping influence metadata-only |
| R4 | proved a realistic Guava fixed-domain external dry-run pack is authorable |
| R5 | fixed HOF, demand, and materialization boundaries without adding a broad primitive |
| R6 | records the minimal capability set, remaining blockers, and validation gates |

No new kernel primitive was added. The work generalizes existing primitive
composition.

## Minimal Capability Set

The closeout artifact records the current minimal set:

- domain requirement composition;
- admitted `LibraryApi.Contract` occurrence evidence;
- admitted API result-domain materialization;
- demand/effect profiles for already-admitted operations;
- external manifest metadata schema;
- conformance and preflight gates.

This is enough to support a wider builtin fixed-result surface and metadata-only
external authoring. It is not enough to open arbitrary external exact influence.

## Builtin Expansion

The shared fixed-result materializer now covers rows such as:

- Python builtin and imported collection factories;
- Rust `Vec`, std collection, std map, and `Some` rows;
- Java collection and map factories and constructors;
- Ruby `Set.new`;
- JavaScript/TypeScript `Set`, `Map`, `Array.from`, and `Promise.resolve`;
- existing receiver rows such as map key views, scalar integer methods, Rust
  `Option.and_then`, and `Promise.then`.

The compression matters: receiver-method result domains and Rust `Some` now use
the same shared materializer, while HOF compatibility remains separate from
exact fixed-domain evidence emission.

## External Pack Readiness

External providers can now author:

- package metadata;
- evidence producer metadata;
- `LibraryApi.Contract` producer metadata;
- fixed call result-domain contract metadata;
- fixtures and hard negatives;
- fixture-expectation gate metadata.

External rows still cannot influence exact analysis. That remains blocked until
dependency-backed producer runtime, explicit trust, package/version occurrence,
recognizer/parser hooks, and rollback/adoption gates exist.

## Remaining Blockers

The remaining blockers are evidence and runtime problems, not missing imagined
vocabulary:

- package/version occurrence proof;
- external evidence producer runtime;
- trait identity and materialization proof;
- dtype, tensor, vector mask, shape, and broadcasting domains;
- stream lifecycle and scheduler proof;
- broad HOF result-domain claims, which remain rejected.

## Product And Performance Gates

Cycle 1 ran a focused product-output and runtime comparison. Product output was
unchanged after volatile timing fields were ignored, and the warmed subset median
runtime improved by 5.08%.

Later phases are metadata, validation, documentation, and artifact changes only.
They do not change semantic hot paths.

The issue-level runtime gate therefore passes the "no more than 10% median
regression" rule.

## Closeout

The #511 goal is satisfied after the R6 PR merges:

- the kernel has broader builtin fixed-result coverage;
- external fixed-domain authoring is real and validated;
- broad HOF/materialization influence remains closed;
- no new primitive was added;
- remaining blockers are explicitly recorded as producer, runtime, trust, or
  domain-model work.

Back to [semantic-kernel](semantic-kernel.md).
