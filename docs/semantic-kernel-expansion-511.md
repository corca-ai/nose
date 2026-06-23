# Semantic kernel expansion 511

Status: issue #511 implementation record, cycles 1 and 2 of the R1-R3 loop.

Source artifacts:

- [blocker_packet.v4.json](../bench/semantic_pack/blocker_packet.v4.json)
- [kernel_capability_matrix.v4.json](../bench/semantic_pack/kernel_capability_matrix.v4.json)
- [blocker_packet.v3.json](../bench/semantic_pack/blocker_packet.v3.json)
- [kernel_capability_matrix.v3.json](../bench/semantic_pack/kernel_capability_matrix.v3.json)
- [blocker_packet.v2.json](../bench/semantic_pack/blocker_packet.v2.json)
- [kernel_capability_matrix.v2.json](../bench/semantic_pack/kernel_capability_matrix.v2.json)

## Goal

Issue #511 asks for repeated internal kernel/builtin cycles before external-pack
readiness is declared. This first cycle keeps the #509 discipline:

1. select about 20 real blocker probes;
2. group them by proof shape;
3. prefer existing primitive composition over new vocabulary;
4. apply accepted primitive changes across existing builtins;
5. compress special-case paths in the same wave;
6. keep hard negatives, product-output classification, and the 10% runtime gate.

Cycle 1 deliberately did not move to R4. It widened one existing primitive and
recorded what still blocked external pack authorability. Cycle 2 then tested the
largest remaining authoring blocker without opening external influence.

## Cycle 1 R1 Blocker Packet

[`blocker_packet.v3.json`](../bench/semantic_pack/blocker_packet.v3.json)
records 20 probes. The dominant proof shape is
`admitted_api_record_materialized_result_domain`: a `LibraryApi` occurrence is
already admitted and has a fixed safe result domain, but the kernel needs a
single path that materializes that fact as call-node `DomainEvidence`.

| decision | count | meaning |
|---|---:|---|
| accepted | 12 | fixed result-domain materialization is safe for more builtin families |
| existing | 2 | #509 receiver chains and Rust `Some` already had the behavior but now use the shared path |
| blocked | 4 | external authoring, package/version, trait/materialization, or dtype proof is missing |
| rejected | 2 | HOF result domains and `Map.get` value domains remain unsafe to emit broadly |

## Cycle 1 Accepted Generalization

No new primitive was added. The accepted change generalizes
`admitted_api_result_domain` into a shared materializer:

- read an asserted `LibraryApi` evidence record on a call;
- resolve it back to a known builtin contract id and callee;
- require the evidence arity to match the actual call arity;
- require the API evidence to be admitted for the current call shape;
- emit language-core `DomainEvidence` only when that row has a fixed safe result
  domain;
- make the domain evidence depend on the exact `LibraryApi` evidence record.

This keeps the primitive small: API occurrence proof plus domain evidence. It
does not introduce a result type system or a broad HOF rule.

## Cycle 1 Builtin Expansion

The materializer now covers fixed-result call rows across multiple languages:

| family | emitted result domain |
|---|---|
| Python builtin/imported collection factories | `Collection` or `Set` |
| Rust std collection factories and `vec!`/`Vec::new` | `Collection` or `Set` |
| Rust std map factories | `Map` |
| Java collection factories and constructors | `Collection` or `Set` |
| Java map factories | `Map` |
| Ruby `Set.new` | `Set` |
| JavaScript/TypeScript `new Set` and `new Map` | `Set` or `Map` |
| JavaScript/TypeScript `Array.from` wrapper | `Array` |
| JavaScript/TypeScript `Promise.resolve` | `PromiseLike` |
| #509 receiver rows | unchanged result domains through the shared path |
| Rust `Some(...)` | `Option` through the shared path |

## Cycle 1 R3 Compression

The cycle reduces special-case emission paths:

- receiver-method result-domain emission now goes through the shared
  materializer;
- Rust `Some(...)` call result-domain emission now goes through the shared
  materializer;
- the registry's fixed-domain lookup reuses the same materialized-result-domain
  helper;
- HOF compatibility remains separate and is not used to emit call-node
  `DomainEvidence`.

That separation matters: existing protocol consumers may keep their conservative
compatibility behavior, while emitted exact evidence stays dependency-backed and
fixed-domain only.

## Cycle 1 Hard Boundaries

The materializer stays closed when:

- the evidence arity does not match the actual call arity;
- the API record is not admitted for the current call shape;
- the result domain is selected by the caller, as with collect-like APIs;
- the result domain is parametric in container value type, as with `Map.get`;
- the row is a broad HOF call result without materialization proof;
- Java `Arrays.asList(x)` has the ambiguous single-argument array-vs-element
  boundary.

## Cycle 1 Product And Performance Gates

The focused semantic-query subset compared `boltons`, `serde_json`, and `junit5`
between the `origin/main` baseline and this cycle's release binary. After
removing volatile timing fields, the JSON outputs were identical for all three
repos, so the product-output classification is no drift on that subset.

The warmed alternating r15 timing on the same subset passed the 10% runtime
gate:

| measurement | baseline | current | delta |
|---|---:|---:|---:|
| focused subset total median | 679.69 ms | 645.18 ms | -5.08% |
| `boltons` | 85.20 ms | 73.78 ms | -13.41% |
| `serde_json` | 61.14 ms | 62.50 ms | +2.22% |
| `junit5` | 533.35 ms | 508.90 ms | -4.58% |

## Cycle 1 Transition Assessment

This cycle is progress, but it is not enough to move #511 to R4.

Criteria met:

- zero new primitives were added;
- one primitive composition explains many accepted rows;
- the same vocabulary now spans Python, Rust, Java, Ruby, and JavaScript-like
  builtins;
- R3 found and removed call-specific result-domain emission paths.

Criteria not met:

- external packs still cannot author executable fixed result-domain rows;
- package/version, trait/materialization, and dtype/domain producers remain real
  blockers;
- only one post-#509 R1-R3 cycle has completed.

The next cycle should choose another 20-row packet biased toward the remaining
package/version, materialization, dtype, and external-authoring blockers. R4
should start early only if builtin rows can now express useful fixed-domain
capabilities that external packs still cannot express.

## Cycle 2 R1 Blocker Packet

[`blocker_packet.v4.json`](../bench/semantic_pack/blocker_packet.v4.json)
records the second 20-probe R1-R3 cycle. The packet is biased toward the
authoring gap left by cycle 1: external packs could see the vocabulary but could
not yet declare fixed result-domain contracts in a validated, dependency-backed
shape.

| decision | count | meaning |
|---|---:|---|
| accepted | 4 | external fixed result-domain authoring can be structurally validated |
| existing | 4 | existing metadata-only, conflict, and preflight gates already preserve the boundary |
| blocked | 9 | package/version, external execution, dtype/materialization, and runtime trust remain missing |
| rejected | 3 | package metadata, broad HOF rows, and parametric value-domain rows are still not semantic proof |

## Cycle 2 Accepted Generalization

No new primitive was added. The accepted change is a manifest authoring
validation rule for fixed call result domains:

- `semantics.result_domain` must be an object;
- `kind` must be `fixed`;
- `domain` must be one of the known kernel `DomainEvidence` labels;
- `subject`, when present, must be `call`;
- `notes`, when present, must be a non-empty string;
- the contract must require `LibraryApi.Contract` evidence, either directly or
  through a declared evidence producer of that kind.

This is deliberately an authoring surface, not an influence surface. External
manifest rows that pass validation are still registered as data-only rows. They
remain blocked by the external influence preflight because no external producer
runtime, influence trust gate, or dependency-backed evidence channel has been
opened.

## Cycle 2 R3 Compression

The R3 check did not find another builtin emission path to collapse. That is the
important result: cycle 1 had already moved fixed-domain emission into the
shared admitted-API materializer, and cycle 2 showed the matching external
manifest shape can reuse the same vocabulary without adding another primitive.

The kernel now has two halves of the same capability:

- compiled builtin rows can emit fixed call result-domain evidence only through
  admitted `LibraryApi` occurrence evidence;
- external manifests can declare the same fixed-domain intent only as
  dependency-backed, metadata-only rows.

## Cycle 2 Hard Boundaries

The second cycle keeps these blockers closed:

- package or module presence alone does not prove a `LibraryApi` occurrence;
- external rows cannot influence normalize, value-graph, exact, or detection
  consumers;
- executable producer hooks, recognizers, parser plugins, and sandboxed runtime
  code are not part of v0;
- broad HOF rows cannot claim a fixed result domain without materialization
  proof;
- `Map.get`-style value domains remain parametric and cannot be emitted as a
  fixed call result domain;
- dtype, tensor shape, stream lifecycle, and package-version proof are still
  separate future substrates.

## Cycle 2 Product And Performance Gates

The implementation changes manifest validation and committed assessment
artifacts only. No query, normalize, value-graph, fragment, oracle, or detection
hot path reads external result-domain rows. Product output is therefore expected
to be unchanged except for additive semantic-pack check/loading metadata.

The cycle records this as a descriptor/validation-only change in
[`kernel_capability_matrix.v4.json`](../bench/semantic_pack/kernel_capability_matrix.v4.json):
no hot-path measurement is required for the 10% runtime gate, and any later PR
that opens external influence must run the normal product-output and runtime
measurement gates.

## Cycle 2 Transition Assessment

The R1-R3 loop has now met the threshold for moving to R4:

- cycle 1 generalized a builtin primitive and applied it across existing
  builtins;
- cycle 1 compressed special-case fixed-domain emission paths;
- cycle 2 exposed the same capability as strict metadata-only external manifest
  vocabulary;
- both cycles added zero new primitives;
- hard negatives for HOF result domains, `Map.get` value domains, and
  package-presence-only claims remain closed;
- external influence is still blocked by explicit data-only and
  dependency-backed-evidence gates.

R4 should now focus on external pack authorability and conformance: prove that a
provider can write useful manifests, fixture gates, and unsupported-boundary
metadata for the capabilities already rehearsed by builtin rows, while exact
analysis continues to ignore external rows until the later influence gates
exist.

Back to [semantic-kernel](semantic-kernel.md).
