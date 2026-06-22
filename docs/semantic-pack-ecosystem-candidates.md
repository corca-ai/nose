# Semantic pack ecosystem candidates

Status: candidate tracker for future large-ecosystem builtin semantic-pack work.

This page records the narrow-slice rule for broad ecosystems such as Lodash,
RxJS, NumPy, pandas, Guava, Tokio, and Rails. It is intentionally not an
implementation plan for a broad default-enabled ecosystem pack.

## Rule

Large ecosystems must enter nose as small builtin pack candidates with explicit
evidence and adoption gates. Do not add a broad ecosystem pack just because the
ecosystem is popular.

Before implementation, candidate rows should pass through the corpus-backed
[semantic-pack candidate pricing](semantic-pack-candidate-pricing.md) loop. That
loop records whether each row is `priced-ready`, `priced-but-blocked`, or
`unpriced`, and keeps corpus prevalence separate from proof.

Create an implementation issue only when the candidate can name:

- proposed pack id;
- owner and maintainer responsibility;
- package and version policy;
- exact API surface and unsupported cases;
- required dependency-backed evidence;
- positive fixtures and hard negatives;
- product output and runtime measurement plan;
- adoption-gate and rollback evidence.

External ecosystem packs remain explicit opt-ins and metadata-only until the
compatibility, dependency-backed evidence, trust, conformance, and conflict gates
all exist.

## Candidate Matrix

| ecosystem | first narrow slice | candidate status | target lane/channel | value | risk | evidence availability | tracking issue |
|---|---|---|---|---|---|---|---|
| Guava | `nose.java.ecosystem.guava.collection_factories`: immutable collection factories (`ImmutableList.of`, `ImmutableSet.of`, `ImmutableMap.of`, safe `copyOf`) | selected first candidate | `builtin-optional` candidate | high for Java collection equivalence | medium | good: existing Java collection-factory vocabulary can be reused with Guava import/version proof | [#496](https://github.com/corca-ai/nose/issues/496) |
| Lodash | collection projection/predicate helpers (`map`, `filter`, `some`, `every`) | deferred until fixture evidence | undecided | high for JS/TS repos | high | mixed: callback demand, shorthand iteratees, object order, and lazy chains need hard negatives | [#497](https://github.com/corca-ai/nose/issues/497) |
| NumPy | scalar integer ufuncs or array clip/min/max laws | deferred | undecided | high for Python data/science repos | high | mixed: dtype, broadcasting, NaN, signed-zero, overflow, and mutation boundaries must be explicit | [#498](https://github.com/corca-ai/nose/issues/498) |
| RxJS | Observable identity/projection protocol slices | deferred | likely `near-only` before exact-capable rows | medium-high for JS/TS reactive code | high | limited: scheduler, subscription, hot/cold stream, error, and completion behavior need proof boundaries | [#499](https://github.com/corca-ai/nose/issues/499) |
| pandas | Series/DataFrame selection or aggregation slices | deferred | undecided | high | very high | weak until index alignment, dtype, NA semantics, mutation/view-copy, and version boundaries are scoped | future issue only after evidence |
| Tokio | Future/stream identity adapters or async utility slices | deferred | undecided | medium | high | weak until scheduler, cancellation, pinning, side effects, and error propagation are scoped | future issue only after evidence |
| Rails ActiveSupport | collection helper slices | deferred | undecided | medium | high | mixed: monkey patching, receiver class, version, nil behavior, and block effects need stronger proof | future issue only after evidence |

## First Candidate

Guava immutable collection factories are the first implementation candidate
because they can reuse the most existing Java collection-factory vocabulary. The
candidate prices as `priced-ready` in the current
[`candidate_pricing.v1.json`](../bench/semantic_pack/candidate_pricing.v1.json)
artifact, but it still remains `builtin-optional` until fixtures, hard
negatives, product output, runtime evidence, and adoption-gate evidence are
attached.

The Guava candidate must still prove the exact package coordinate, import or
static-import path, arity/overload identity, version policy, result domain, and
unsupported cases. It must not admit exact equivalence from selector names alone.

## Deferred Candidates

Lodash, NumPy, RxJS, pandas, Tokio, and Rails are deferred until their first
narrow slices have enough evidence. The main blockers are callback demand,
versioned API behavior, dtype/domain proof, scheduler or async semantics,
mutation/view-copy behavior, and framework monkey patching.

When evidence exists, create a separate issue for the exact slice. Keep the
initial issue scoped to one pack id and one support boundary.

## Related

- [semantic-pack-architecture](semantic-pack-architecture.md)
- [semantic-pack-adoption](semantic-pack-adoption.md)
- [semantic-pack-compatibility](semantic-pack-compatibility.md)
- [semantic-pack-candidate-pricing](semantic-pack-candidate-pricing.md)
