# Semantic pack extension API v0

Back to [semantic-kernel](semantic-kernel.md). Current implementation status is
in [semantic-kernel-snapshot](semantic-kernel-snapshot.md); history and remaining
work are tracked in [semantic-kernel-roadmap](semantic-kernel-roadmap.md). The
internal evidence substrate is described in
[evidence-records](evidence-records.md), and source-origin evidence is described
in [source-facts](source-facts.md).

Schema artifacts:

- [semantic-pack-v0 schema](schemas/semantic-pack-v0.schema.json)
- [language-pack example](examples/semantic-packs/v0/language-pack.json)
- [library-pack example](examples/semantic-packs/v0/library-pack.json)
- [law-pack example](examples/semantic-packs/v0/law-pack.json)
- [Python stdlib type-domain pack example](examples/semantic-packs/v0/python-typing-domain-pack.json)
- [semantic-pack-conformance](semantic-pack-conformance.md)
- [semantic-pack-loading](semantic-pack-loading.md)

Status: design v0 plus local metadata loading and a local conformance harness.
nose can load local manifests for explicit opt-in provenance reporting, and pack
authors can run `nose semantic-pack check` against manifests and declared fixture
assets. External packs are still `metadata-only`: they do not emit evidence or
open exact contracts. The first compiled first-party pilot pack now reports
Python `typing`, `collections.abc`, and `asyncio` stdlib type-domain alias
evidence through this vocabulary. This page defines the extension surface that
first-party compiled packs and future external packs must use.

## Context

nose is moving toward a DefinitelyTyped-style semantic-pack ecosystem: providers
publish language, standard-library, library, protocol, and law packs; users opt
into the packs they trust; nose validates the extension shape and fails closed
when required facts are absent. nose certifies only the first-party default packs
that it ships and gates in CI.

The current implementation already has the internal pieces that this API narrows
into a public boundary: `EvidenceRecord` facts in `nose-il`, contract rows and
admission helpers in `nose-semantics`, and evidence-only consumers in normalize,
detect, value-graph, fragment, and oracle paths. v0 is intentionally a schema and
contract vocabulary. The current loader validates local manifest shape and
reports provenance; it does not execute external producers.

## Goal

- Define a narrow external API for adding language and library semantics without
  reaching into private normalization or detection internals.
- Make every semantic claim name the exact language/API surface it covers, the
  evidence required to admit that surface, and the channel where the claim may
  be used.
- Preserve exact-channel precision by making missing, ambiguous, conflicting, or
  dependency-broken evidence fail closed.
- Let first-party built-in semantics use the same vocabulary as future external
  packs, even while first-party packs remain compiled into nose.
- Give pack authors a conformance checklist that is clear enough for users to
  judge external packs without implying nose approval.

## Non-goals

- Do not implement network or registry pack loading.
- Do not let local pack loading make external producers executable yet.
- Do not expand language or library coverage.
- Do not let packs mint fingerprints, rewrite value graphs directly, approve
  exact clone pairs, or bypass the law registry.
- Do not model lazy, async, stream, channel, observable, or call-by-need behavior
  beyond naming the demand/effect fields those later contracts must fill.
- Do not make nose responsible for approving, certifying, or auditing external
  pack correctness. Providers own claims; users own opt-in decisions.

## Versioning

Every manifest must declare:

- `api_version`: currently `nose.semantic-pack.v0`;
- `compatibility.nose`: the nose versions the pack claims to support;
- `pack.version`: the provider's pack version;
- stable ids for packs, evidence producers, contracts, laws, fixtures, and
  dependencies.

v0 is a strict manifest contract: the published schema rejects unknown fields,
and the local loader rejects unknown fields and enum values. Adding fields,
adding enum values, removing fields, changing the meaning of an existing field,
or changing exact-channel admission rules requires a new API version.

## Manifest Shape

The schema is in [schemas/semantic-pack-v0.schema.json](schemas/semantic-pack-v0.schema.json).
A v0 manifest has these top-level sections:

| section | purpose |
|---|---|
| `api_version` | schema family, fixed to `nose.semantic-pack.v0` |
| `pack` | stable id, kind, version, trust policy, default-enable status, and human label |
| `provenance` | provider, license, repository, source revision, and contact metadata |
| `compatibility` | supported nose version range and optional schema notes |
| `supported_languages` | language/runtime/version claims the pack covers |
| `packages` | stdlib or ecosystem package coordinates for non-core library packs |
| `dependencies` | other packs or protocols that must be present |
| `declares.evidence_producers` | facts the pack may emit |
| `declares.contracts` | semantic contracts the pack may claim |
| `declares.value_laws` | optional law contracts, where ready |
| `conformance` | positive fixtures, hard negatives, commands, proof links, and unsupported edges |

The manifest is declarative. It is not a hook API. A data-only external pack can
declare rows that the local metadata loader validates and reports. A later
producer runtime may feed compatible rows into kernel helpers only after the same
fail-closed evidence obligations are satisfied. First-party compiled packs can
generate the same manifest metadata for reports and conformance gates while
still emitting facts from Rust.
The Python stdlib type-domain example mirrors the first compiled first-party pilot
surface; loading that example locally remains metadata-only because local
manifests cannot claim first-party trust.

## Pack Kinds

| kind | responsibility |
|---|---|
| `LanguagePack` | file/language identity, parser/lowering binding, source facts, core evaluation facts, language-level operator/domain/source contracts |
| `StdlibPack` | standard library APIs tied to a language/runtime version |
| `LibraryPack` | ecosystem package APIs tied to package/version coordinates |
| `ProtocolPack` | shared protocol vocabulary such as `Iterable`, `Iterator`, `Map`, `Option`, `Result`, `Promise`, `Observable`, or `Tensor` |
| `LawPack` | reusable laws with explicit preconditions, proof status, and conformance fixtures |

Language core and library semantics are intentionally separate. JavaScript
call-by-value order, `Array.prototype.map`, Rust `Iterator::map`, RxJS
`Observable.map`, and NumPy vector operations must be distinct contracts even if
some of them map onto a shared protocol operation.

## Trust And Enablement

Trust policy is separate from channel eligibility.

| trust | meaning |
|---|---|
| `default-first-party` | maintained, tested, and enabled by nose by default |
| `first-party-optional` | maintained and tested by nose, but not enabled by default |
| `external-opt-in` | provider/user responsibility; must be enabled explicitly by the user |

Local external manifests must set `trust: "external-opt-in"` and
`enabled_by_default: false`. The loader rejects local manifests that claim
first-party trust or default enablement. A manifest may declare that a contract
is intended for `exact-empirical` or `exact-proven`, but nose does not certify
that claim for external packs. A user may still opt into such a pack, and nose
should surface provenance so the user can see which external pack affected a
match.

## Channel Eligibility

Each evidence producer, contract, and law declares where it may be used.

| channel | use |
|---|---|
| `syntax-only` | source/lowering/report provenance only |
| `near-only` | candidate generation or review scoring only |
| `abstraction-witness` | weak refactoring-template witnesses over `near`; never exact equivalence |
| `exact-empirical` | exact channel only when required fixtures, hard negatives, and oracle-style checks are provided |
| `exact-proven` | exact channel with proof obligations for the core law or contract |

An exact-capable channel still requires the kernel to find all declared evidence
and dependencies for the specific occurrence. A contract row is not an admission
by itself.

## Evidence Producers

Packs declare the evidence kinds they may emit. The current v0 vocabulary mirrors
the implemented internal substrate without exposing Rust layout as the public
schema:

| family | examples |
|---|---|
| `Source` | construct syntax, macro invocation, regex literal, equality/operator family, ranges, patterns, async/generator/error and Go channel/concurrency protocol boundaries, Python comprehension surfaces |
| `Symbol` | unshadowed language global, imported binding, imported namespace, qualified global path |
| `Import` | binding import, namespace import, wildcard import, Ruby require, C quote include, imported literal snapshot |
| `Domain` | array, collection, iterable, iterator, set, map, record, option/result, promise/future-like, string, boolean, integer/float/number, byte array, nominal type |
| `Type` | C type-alias proofs for exact byte-pack surfaces and nominal type-domain rows |
| `Guard` | JS/TS record-shape and own-property guard facts |
| `Place` | fixed receiver/place facts such as self receiver and self field |
| `Effect` | builder append, non-overloadable index write, self-field write, binding write, receiver mutation, opaque argument escape |
| `LibraryApi` | occurrence proof that a call, field, property, constructor, macro, or sentinel matches one contract coordinate |
| `CallTarget` | explicit call target proof for direct functions/methods, imported function/member coordinates, and dispatch-family facts |
| `SequenceSurface` | lowered aggregate surface such as collection, tuple, map, pair, record guard, own-property guard, Go map literal, or Go map entry |

The producer declaration must include:

- a stable `id`;
- the `kind` it may emit;
- allowed `anchors`;
- the dependencies it requires;
- the channel it can influence;
- stable hash inputs used to derive record ids or occurrence ids;
- a conflict policy, normally `fail-closed`.

Packs may inspect selectors and syntax while producing evidence, but they must
emit a specific fact such as `Symbol.UnshadowedGlobal` or
`LibraryApi.Contract`. A selector string alone is never a semantic fact.

## Anchors

Evidence must attach to one of the kernel-defined subjects:

| anchor | subject |
|---|---|
| `source-span` | a source surface that is not tied to a normalized node |
| `node` | a specific IL node and its span |
| `param` | a function parameter span |
| `binding` | a binding span plus stable local-name hash |
| `sequence` | a lowered aggregate sequence span |
| `module` | a module or file-level fact, used only when the contract also names how occurrence uses are linked |
| `package` | a package/version coordinate, used for manifest dependency claims, not as occurrence proof by itself |

Exact consumers should prefer occurrence anchors (`node`, `param`, `binding`, or
`sequence`) over broad module/package anchors. A module-level dependency can help
prove an imported namespace, but it must be linked to the queried occurrence by a
symbol/import dependency before it admits a call or property.

## Dependencies

Dependencies are local references to other evidence records or manifest
contracts. They are how v0 prevents name-only semantics.

For example, a Python `math.prod(xs)` contract should depend on a call-site
`Symbol.ImportedNamespace` proof for `math`, which itself depends on import
evidence for the namespace binding and on shadow/rebinding checks. The local
name `math` is only a selector inspected by the producer; the admitted contract
is the dependency-backed occurrence.

A dependency reference has:

- `ref`: the producer, contract, law, or protocol id being required;
- `subject`: the occurrence role, such as `callee`, `receiver`, `argument[0]`,
  `callback`, `binding`, or `source`;
- `required`: whether missing evidence closes the contract;
- optional `same_anchor_as`, `before`, `after`, or `within_scope` constraints.

If a required dependency is missing, ambiguous, conflicting, dependency-broken,
wrong-anchor, wrong-arity, wrong-version, or outside scope, exact admission must
fail closed.

## Contract Rows

Contracts describe what a proven surface means. They do not emit fingerprints.

Every contract must declare:

- stable `id`;
- `surface`: language/runtime/package coordinate, API kind, selector/path, arity,
  receiver role, overload identity, and source call shape;
- `requires`: evidence obligations for source, symbol, import, receiver, domain,
  type, guard, place, effect, call-target, version, shadowing, and scope;
- `semantics`: the protocol operation, result domain, evaluation/demand profile,
  callback demand, effect summary, exception behavior, mutation behavior, and
  allocation/identity caveats;
- `channel`: the strongest channel the contract may influence;
- `proof_status`: `proven`, `covered`, `missing`, `empirical-only`, or
  `rejected-counterexample`;
- conformance references and known unsupported boundaries.

### Source-Fact Contracts

Source facts preserve distinctions the IL erases. Examples include `new Map`
versus `Map`, regex literal versus string, strict versus loose equality, Python
generator expression versus list comprehension, Rust half-open versus inclusive
range, and Go channel receive versus ordinary call.

A source fact is syntax provenance. It admits an exact path only when a contract
also proves the API, receiver, symbol, arity, demand, and effect obligations that
make the surface meaningful.

### Symbol And Import Contracts

Symbol/import contracts must name the exact coordinate they prove:

- unshadowed language global;
- imported binding or namespace;
- qualified global path;
- Java `java.util` import or wildcard import with local-shadow checks;
- Ruby `require` module proof;
- C quote include proof.

Local spelling is not enough. Rebinding, wildcard import ambiguity, local type
shadowing, missing require/import evidence, or unresolved namespace exports must
close exact admission.

### Domain And Type Contracts

Domain/type contracts state what is proven about a value or receiver:

- `Array`, `Collection`, `Set`, `Map`, `Option`, `PromiseLike`, `String`,
  `Integer`, `Number`, `ByteArray`, or future protocol-specific domains;
- binding-domain proof for immutable local/module values;
- result-domain proof for admitted factories and protocol-producing APIs such as
  Promise-like producers;
- type-alias proof for current C byte-pack surfaces.

Domain evidence can satisfy receiver-domain preconditions. It is not a proof
that an opaque binding value is exact-tree safe, immutable, or mutation-free
unless separate effect/place/import facts prove those obligations.

### Library API Occurrence Contracts

`LibraryApi` is occurrence evidence: it says this specific call, field, macro,
constructor, property, or sentinel matches a contract coordinate. The occurrence
must name:

- contract id;
- callee coordinate;
- language/package/version coordinate;
- arity and receiver position;
- dependencies that prove callee, receiver, import, source shape, shadowing, and
  overload obligations.

Examples of valid coordinates include `Python math.prod`, Rust
`std::collections::HashMap::from`, Java `java.util.Map.of`, JS-like
`Array.isArray`, JS regex-literal `.test`, JS-like `Promise.resolve`, or a
language-scoped receiver method whose receiver domain is proven. `method named
map` or `method named then` is not a valid coordinate.

### Demand, Effect, And Place Contracts

Demand and effect fields are mandatory for exact-capable contracts, even when v0
uses a conservative placeholder.

Demand fields should state:

- argument order and whether arguments are eager, conditional, repeated,
  per-element pull, delayed, memoized, or never demanded;
- callback demand, including how often callbacks may run and under which
  consumer;
- observation boundary for iterators, generators, promises, futures, channels,
  streams, and observables.

Effect/place fields should state:

- pure, read-only, local mutation, receiver mutation, builder append, index
  write, self-field write, opaque escape, allocation, async scheduling,
  exception, yield, channel send/receive, or stream emission;
- which place is affected and whether final state or ordered effects are the
  observable;
- which effects are skipped, delayed, repeated, or memoized under demand.

If the contract cannot express the demand/effect behavior precisely enough,
`channel` must be `near-only` or `syntax-only`.

### Call-Target Contracts

Call-target facts prove that a user call resolves to a direct in-file callable.
They are separate from library APIs. A raw callee spelling such as `f(...)` is
not enough because local shadowing, duplicate definitions, nested functions,
method dispatch, imports, or computed callees can change the target.

v0 supports direct target evidence only where the producer can prove a unique
target span and no shadowing along the relevant lexical path.

### Value Laws

Value laws declare reusable semantic rewrites or equivalence laws. They are the
future pack-facing shape for current first-party value-graph rule modules.

A law must declare:

- stable `id`;
- required domains;
- required demand/effect conditions;
- protocol operation or operator family;
- proof status and proof-obligation references;
- positive fixtures and hard negatives;
- whether the law is first-party certified or external provider/user trust.

External law declarations do not let a pack bypass the first-party law registry
or emit private value-graph nodes. Until the law registry is pack-facing, external
law packs should stay `near-only` unless nose adopts them as first-party.

The first compiled first-party `LawPack` pilot is `nose.value_graph.laws`. It
reports pack-facing law provenance for two proof-backed value-graph laws:
numeric common-factor distribution and integer ordered min/max clamp. Those laws
still execute from compiled first-party Rust and their exact status is tied to
the project's formal obligation registry. The [law-pack example](examples/semantic-packs/v0/law-pack.json)
shows the external v0 declaration shape and fixtures, but loading that manifest
locally remains `metadata-only`.

## Conflict And Ambiguity Policy

The default policy is fail closed:

- more than one asserted fact for the same subject and incompatible kind closes
  the queried contract;
- any relevant `ambiguous` fact closes exact admission;
- a broken dependency closes the dependent fact;
- overlapping contracts must remain distinct unless a higher-level protocol
  contract explicitly describes how they compose;
- external pack conflicts do not get resolved by "newest wins" or provider
  priority for exact matching;
- user configuration may disable or select packs, but selection does not make
  missing proof appear.

Near-mode scoring may retain conflict provenance as a review signal. Exact
semantic fingerprints must not.

## Stable Hashes And IDs

Stable ids are report and cache coordinates. They must not depend on process
memory, interner order, local filesystem paths, or non-reproducible timestamps.

v0 hash inputs should include:

- `api_version`;
- `pack.id` and `pack.version`;
- producer/contract/law id;
- language/package coordinate;
- source span or node anchor, when the hash identifies an occurrence;
- callee coordinate, arity, receiver role, and version range for API
  occurrences;
- dependency ids for records whose meaning depends on other proof facts.

Providers may publish human-readable ids; nose may derive internal numeric hashes
from those ids. The human ids remain the public contract.

## Structural Validation Versus Trust

nose validates the following through manifest loading and
`nose semantic-pack check`:

- manifest JSON parses and matches the v0 schema;
- required sections and ids exist;
- ids are unique and stable-looking;
- enum values are known;
- dependency references resolve;
- exact-capable contracts declare required evidence, demand, effect, channel,
  proof status, positive fixtures, and hard negatives;
- external packs are opt-in and not enabled by default;
- declared compatibility ranges are syntactically valid enough to compare later;
- conformance fixtures have expectation labels and point at files that exist
  relative to the manifest path.

nose does not validate or certify for external packs:

- whether the provider's semantic claim is true for all versions claimed;
- whether fixtures and hard negatives are complete;
- whether an `exact-proven` proof is mathematically sufficient;
- whether package version metadata, license metadata, repository metadata, or
  provider identity is trustworthy;
- whether an external pack is safe to enable in a user's risk model.

First-party default packs are different: nose owns their tests, hard negatives,
proof obligations, release gating, and documentation.

Expectation labels are provider-authored strings. The example LawPack uses
`semantic-law-provenance-present` only for report-visible provenance and
`internal-law-unit-positive` for narrower fixtures that exercise a first-party
law without promising a scan-family JSON row.

## First-Party Mapping

The current `nose.first_party` compiled facade should be understood as a set of
implicit v0 packs:

- language packs for JS/TS, Python, Go, Rust, C, Java, Ruby, and embedded JS/TS
  containers emit source, symbol, import, domain, guard, place/effect,
  call-target, and sequence-surface evidence;
- stdlib packs declare library API occurrence contracts such as Python builtins,
  Python `math.prod`, Rust `Vec::new`, Rust `Option` constructors, Java
  `java.util` factories, JS-like globals, regex literal methods, selected
  property builtins, receiver-method APIs, and builder append APIs;
- protocol/law packs correspond to current first-party protocol operations,
  demand profiles, operator laws, value-domain laws, and named value-graph rule
  modules.

Those packs may remain compiled Rust for now. The important rule is that new
first-party work should add or consume the same pack-shaped evidence and contract
vocabulary that an external pack would use later.

## Conformance Checklist

The operational workflow is in
[semantic-pack-conformance](semantic-pack-conformance.md). A pack provider should
publish:

- manifest with stable ids, provenance, license, repository, support contact,
  compatibility, dependencies, trust policy, and default status;
- one or more evidence producer declarations for every fact the pack emits;
- contract rows that identify exact language/API/package/version coordinates,
  not broad function or method names;
- explicit source, symbol, import, receiver, domain, type, guard, place, effect,
  demand, version, shadowing, overload, and arity obligations for exact-capable
  contracts;
- positive fixtures that should converge;
- hard negatives that must not converge, especially same-named APIs, shadowing,
  missing imports, wrong receiver domains, wrong arity, unsupported versions,
  dynamic dispatch, mutation, lazy effects, and ambiguous dependencies;
- known unsupported boundaries and counterexamples;
- proof links for `exact-proven` laws;
- a reproducible conformance command.

First-party packs must run this checklist in nose CI before becoming default.
External packs should ship it so users can evaluate the pack. Passing
`nose semantic-pack check` is not nose certification; it only proves the local
manifest and declared fixture assets satisfy the v0 structural contract.

## v0 Acceptance

This issue is complete when:

- the v0 schema and design document are linked from home, snapshot, and roadmap;
- the schema has at least one language-pack and one library-pack example;
- examples are checked by the local conformance harness and docs example gate;
- the docs state that external pack correctness is provider/user responsibility;
- the docs state that first-party built-in semantics use the same extension
  vocabulary even if they remain compiled in initially.
