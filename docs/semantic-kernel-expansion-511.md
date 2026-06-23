# Semantic kernel expansion 511

Status: issue #511 implementation record, cycle 1 of the R1-R3 loop.

Source artifacts:

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

The cycle deliberately does not move to R4 yet. It widens one existing primitive
and records what still blocks external pack authorability.

## R1 Blocker Packet

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

## Accepted Generalization

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

## Builtin Expansion

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

## R3 Compression

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

## Hard Boundaries

The materializer stays closed when:

- the evidence arity does not match the actual call arity;
- the API record is not admitted for the current call shape;
- the result domain is selected by the caller, as with collect-like APIs;
- the result domain is parametric in container value type, as with `Map.get`;
- the row is a broad HOF call result without materialization proof;
- Java `Arrays.asList(x)` has the ambiguous single-argument array-vs-element
  boundary.

## Product And Performance Gates

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

## Transition Assessment

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

Back to [semantic-kernel](semantic-kernel.md).
