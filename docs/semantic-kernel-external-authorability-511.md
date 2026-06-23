# Semantic kernel external authorability 511

Status: issue #511 R4 implementation record.

Source artifacts:

- [external_authorability_matrix.v1.json](../bench/semantic_pack/external_authorability_matrix.v1.json)
- [external-guava-immutable-collections-pack.json](examples/semantic-packs/v0/external-guava-immutable-collections-pack.json)
- [kernel_capability_matrix.v4.json](../bench/semantic_pack/kernel_capability_matrix.v4.json)

## Goal

R4 tests whether the kernel material proven by the R1-R3 cycles can be authored
by external pack providers without granting external influence.

The answer is partial but useful:

- fixed call result-domain contracts are authorable as metadata;
- fixtures, hard negatives, and fixture-expectation gates are authorable;
- external rows still cannot emit dependency-backed evidence;
- package metadata still cannot prove project package occurrence;
- exact influence remains builtin-only until separate runtime, trust, conflict,
  and rollback gates exist.

## Dry-Run Pack

The dry-run pack is
[`com.example.java-guava-immutable-collections`](examples/semantic-packs/v0/external-guava-immutable-collections-pack.json).
It models a realistic Guava slice from the blocker corpus:
`ImmutableList.of(...)` and `ImmutableSet.of(...)` as fixed `Collection`
result-domain factory calls.

The manifest declares:

- Maven package metadata for `com.google.guava:guava`;
- an `Import.Binding` producer for Guava immutable collection classes;
- a `LibraryApi.Contract` producer for the static factory occurrence;
- a contract with `semantics.result_domain.kind = fixed` and
  `domain = Collection`;
- one positive fixture and two hard negatives;
- one fixture-expectation executable conformance gate.

This is a provider authoring test. Loading the manifest remains
`metadata-only`; query, normalize, value-graph, exact, and detection consumers do
not read the external rows.
The contract row's fixture-expectation gate passes, but the producer rows still
report `executable-conformance-unavailable` because v0 does not execute external
evidence producers.

## Authorability Matrix

The R4 matrix records these decisions:

| capability | external authoring | influence | decision |
|---|---|---|---|
| package metadata | authorable | metadata-only | useful for provenance, not occurrence proof |
| `Import.Binding` producer | authorable metadata-only | blocked | builtin-only execution remains intentional |
| `LibraryApi.Contract` producer | authorable metadata-only | blocked | external producer runtime is absent |
| fixed call result domain | authorable metadata-only | blocked | externalizable vocabulary exists |
| fixture and hard-negative metadata | authorable | metadata-only | provider review evidence exists |
| fixture-expectation gate | authorable | blocked | clears only executable-conformance blocker |
| package/version occurrence proof | blocked | blocked | needs lockfile/build-system producers |
| external exact evidence runtime | blocked | blocked | needs trust, provenance, sandbox, rollback design |
| HOF demand/materialization | blocked for exact | blocked | deferred to R5 |
| dtype, tensor shape, vector masks | blocked for exact | blocked | needs future domain producers |

## Builtin-Only Boundaries

Some builtin-only status is intentional:

- compiled Rust producers are currently the only exact evidence runtime;
- builtin rows own product-output and performance gates;
- builtin packs own rollback through normal release control;
- HOF, stream, async, dtype, and materialization semantics remain too risky for
  generic external exact rows.

Other builtin-only gaps are not intended as permanent policy:

- external packs should be able to author fixed-domain rows, as R4 now proves;
- package/version occurrence proof should become an explicit evidence substrate;
- fixture and hard-negative metadata should stay provider-authorable;
- adoption gates should eventually describe how a proven external row becomes
  builtin, optional, or disabled.

## Product And Performance Gates

R4 adds an example manifest, fixtures, documentation, and a machine-readable
matrix. It does not change normal query behavior. The only executable path used
by the dry-run is `nose semantic-pack check`, which validates manifest structure,
fixtures, executable gate metadata, and row-level influence preflight.

The authorability matrix records no hot-path change and no external influence
opening, so the 10% runtime gate does not require a query benchmark in this
round. Any later PR that lets external rows influence analysis must run the full
product-output and runtime gates.

## Transition

R4 is complete enough to move #511 to R5:

- an external provider can author a realistic fixed result-domain pack slice;
- conformance fixtures and hard negatives can be attached to that slice;
- executable conformance can pass without opening influence;
- the remaining blockers are now explicit runtime/trust/package-occurrence or
  high-risk HOF/materialization issues.

R5 should focus on narrow HOF, demand, and materialization boundaries. It should
not admit broad generic HOF result domains.

Back to [semantic-kernel](semantic-kernel.md).
