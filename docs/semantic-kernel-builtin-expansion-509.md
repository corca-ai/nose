# Semantic kernel builtin expansion 509

Status: issue #509 implementation record. This pass follows the #507
capability-minimization loop, but raises the bar from one representative
reduction to one foundational primitive improvement with substantial builtin
expansion.

Source artifacts:

- [candidate_pricing.v1.json](../bench/semantic_pack/candidate_pricing.v1.json)
- [kernel_capability_matrix.v1.json](../bench/semantic_pack/kernel_capability_matrix.v1.json)
- [blocker_packet.v2.json](../bench/semantic_pack/blocker_packet.v2.json)
- [kernel_capability_matrix.v2.json](../bench/semantic_pack/kernel_capability_matrix.v2.json)

## Goal

The target is a kernel with enough real material to support meaningful external
packs, without adding speculative primitives. The rule for this pass:

1. Refresh the #507 blocker vocabulary with at least 20 product-relevant probes.
2. Group the blockers by proof shape, not by ecosystem name.
3. Prefer composition of existing primitives over new vocabulary.
4. Implement every accepted primitive improvement and the builtins it unlocks in
   this issue.
5. Keep hard negatives for unsupported surfaces in the same change.
6. Keep runtime within the semantic-pack performance gate: no more than 10%
   median regression.

## Probe Packet

[`blocker_packet.v2.json`](../bench/semantic_pack/blocker_packet.v2.json)
records 20 probes over Guava, Rails, Go maps, Rust iterators, Python itertools,
RxJS, Lodash, NumPy, pandas, Java streams, Rust options, JS promises, map key
views, integer APIs, and one generic map-value counterexample.

The packet is deliberately not an admission source. It is a blocker packet:
corpus and product relevance can move work into the queue, but exact semantic
admission still requires dependency-backed kernel evidence.

The 20 probes collapse to:

| decision | count | meaning |
|---|---:|---|
| accepted | 7 | fixed result-domain proof can be emitted from already-admitted APIs |
| existing | 2 | demand/effect vocabulary exists, but more row proof is still needed |
| blocked | 10 | package, type, trait, scheduler, lifecycle, or materialization proof is missing |
| rejected | 1 | fixed result-domain proof would be unsound |

## Accepted Primitive

The accepted primitive is `admitted_api_result_domain`.

Its shape is intentionally small: after a receiver-method API occurrence is
admitted, and only if that exact API row has a fixed safe result domain, the
normalizer emits `DomainEvidence` on the call node. That domain evidence depends
on the admitted `LibraryApi` evidence record, so downstream consumers can trace
the proof back to the API occurrence instead of trusting a selector name.

This composes existing primitives:

- `LibraryApi` proves that a call is a specific builtin API occurrence.
- `DomainEvidence` records the proven result domain on the call node.
- Existing receiver-domain lookup consumes that result domain for chained API
  admission.

No new open-ended result type system is introduced.

## Builtin Expansion

The first rollout is limited to rows where the result domain is fixed by the
admitted API and not chosen by the caller:

| row family | emitted result domain |
|---|---|
| `MapKeyView(Collection)` | `Collection` |
| `MapKeyView(Iterator)` | `Iterator` |
| `ScalarIntegerMethod(*)` | `Integer` |
| `RustOptionAndThen` | `Option` |
| `PromiseThen` | `PromiseLike` |

These rows make chained builtin admission exact-capable without adding a
separate primitive per chain:

- Java `m.keySet().contains(k)` can admit `contains` from the `keySet()` result.
- Rust `n.abs().max(x)` can admit `max` from the `abs()` result.
- Rust `maybe.and_then(f).and_then(g)` can admit the second `and_then`.
- TypeScript/JavaScript `p.then(f).then(g)` can admit the second `then`.
- TypeScript/JavaScript `m.keys()` records an iterator result for later
  iterator-domain consumers.

## Hard Boundaries

The implementation stays closed for surfaces where the result domain is not a
fixed property of the admitted API row:

| surface | reason |
|---|---|
| Rust `collect` | result type is selected by the caller |
| HOF result domains such as generic `map` | callback and materialization proof are separate blockers |
| `Map.get` value domain | result depends on the map value type, not the API alone |
| package/version gated APIs | project dependency occurrence proof is still absent |
| NumPy/pandas dtype or series rows | safe type/domain producers are still absent |
| RxJS/lifecycle rows | scheduler and observable lifecycle proof are still absent |

These are recorded in
[`kernel_capability_matrix.v2.json`](../bench/semantic_pack/kernel_capability_matrix.v2.json)
so future pack rows can reuse the blocker taxonomy instead of reopening the same
argument from memory.

## Implementation

The result-domain table lives with receiver-method API rows in
[`receiver.rs`](../crates/nose-semantics/src/library_api/rows/methods/receiver.rs).
The registry exposes the same result-domain lookup for contract consumers in
[`registry.rs`](../crates/nose-semantics/src/library_api/registry.rs). The
normalizer emits dependency-backed call-node domain evidence in
[`recording.rs`](../crates/nose-normalize/src/library_api_evidence/recording.rs).

Focused tests cover both the safe rows and the hard negatives:

- receiver-method contract rows emit only safe result domains;
- admitted receiver-method APIs emit result-domain evidence;
- wrong or missing receiver proof emits neither API evidence nor result-domain
  evidence;
- result-domain evidence from one call feeds the next chained builtin admission;
- the second chained API record depends on the first call's result-domain
  evidence.

## Pricing Impact

This pass does not refresh `candidate_pricing.v1.json` because it does not add
new corpus candidates to the pricing scanner. It adds a second blocker packet
and capability matrix for issue #509. A pricing rerun should therefore produce
no committed pricing diff.

The pricing rerun was:

```sh
python3 bench/semantic_pack/pricing.py --nose ./target/release/nose --query-sample-repos 1
```

It rewrote the generated pricing files but produced no committed diff.

Rows that become easier later:

- scalar and receiver-domain rows where a fixed admitted API result can feed the
  next receiver proof;
- future package rows whose API occurrence can safely carry a result-domain
  contract after dependency proof exists;
- iterator or collection rows that need fixed receiver result proof before a
  terminal consumer can be admitted.

Rows that remain blocked:

- package/version rows without dependency context;
- scheduler or lifecycle rows;
- trait/materialization rows;
- dtype, nominal, and table/series domain producer rows.

## Performance Gate

The performance limit is the semantic-pack gate from
[semantic-pack-architecture](semantic-pack-architecture.md): more than 10%
median runtime growth blocks the PR unless explicitly accepted as a product
decision.

The expected hot-path cost is one optional domain-evidence upsert for an
already-admitted receiver-method API row whose contract has a fixed result
domain. Rows without a fixed result domain keep the old path.

The measured release binaries were:

- baseline: clean worktree at `96fd9a44`, built in `target/issue509/base-worktree`;
- current: this issue branch with the #509 changes, built at `target/release/nose`.

The query-regression `repeats 7` compare produced investigation triggers in
`parse+lower`/`lower` dominated phases. Those phases are outside the changed
path, so the issue uses paired alternating wall-clock measurements for the
10% gate:

| measurement | baseline | current | delta |
|---|---:|---:|---:|
| query-regression subset, sum of repo wall medians, alternating r9 | 3001.70 ms | 3019.50 ms | +0.6% |
| focused noisy repos (`boltons`, `serde_json`, `junit5`), alternating r31 | 898.76 ms | 886.48 ms | -1.4% |

The generated HoF value-graph smoke from query-regression stayed well under its
hard budgets: 9.64 ms for features and 7.70 ms for semantic query, both against
3000 ms budgets. The implementation therefore stays under the 10% runtime
regression limit.

Back to [semantic-kernel](semantic-kernel.md).
