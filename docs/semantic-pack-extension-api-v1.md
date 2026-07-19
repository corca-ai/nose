# Typed semantic pack influence manifest v1

Status: implemented typed manifest, deterministic compiler, content-pinned
project authorization, kernel-owned dependency/occurrence evidence, locked near
influence, and receipt-backed external-claim exact influence.
`nose.semantic-pack.v0` remains permanently metadata-only.

## Purpose

v1 is the first semantic-pack format that nose can interpret without executing
provider code. It replaces v0's opaque `surface` and `semantics` JSON with a
small closed grammar. Loading a v1 file validates and compiles that grammar
before analysis starts. A validated project lock pins and authorizes rows,
channels, dependency files, and a kernel conformance receipt. During a locked query nose
can now read the pinned Maven inputs and bind selected rows to builtin Java
import/symbol/receiver/effect facts. For near-authorized rows, the resulting
immutable index may support an existing near candidate. A much narrower
`external-exact` row may enter the existing exact kernel only when source
conformance produced a matching receipt and the project lock pins it. Unlocked
v1 and every v0 manifest remain `metadata-only`.

The schema is [semantic-pack-v1.schema.json](schemas/semantic-pack-v1.schema.json).
The [Guava example](examples/semantic-packs/v1/guava-immutable-collections.json)
shows both range and explicit-set arity contracts.

## Closed v1 surface

The first slice deliberately covers only the Java/Maven static package APIs
needed to exercise the 0.20 external-pack path:

| Field | Accepted vocabulary |
| --- | --- |
| language / ecosystem | `java` / `maven` |
| anchor / matcher | `call-node` / `imported-api` |
| import role | `type`, `static-member` |
| call shape | `static-method`, `free-function` |
| receiver | `imported-type`, `none` |
| arity | bounded `range`, or an explicit bounded `set`; values are at most 32 |
| operation | `collection-factory`, `map-factory` |
| fixed result domain | `collection`, `map` |
| demand / effects | `eager` / `pure` |
| exceptions | `no-throw`, `may-throw` |
| mutation / identity | `none` / `fresh` |
| requested lane | `near`, `external-exact` |

The compiler checks combinations, not just individual enum labels. A type import
must be a static method on an imported type; a static-member import must be a
free function with no receiver. Collection and map factory operations must use
their corresponding fixed result domain. Map-factory arities must contain only
even positional key/value counts.

`external-exact` is narrower still: v1 accepts only `collection-factory` with
`eager`, `pure`, `no-throw`, `none`, and `fresh` profiles, plus at least one
positive and one hard-negative source fixture per row. Map, HOF, lazy, async,
stream, provider value-law, and new-canon exact claims remain closed.

Exact coordinates use validated Maven `group:artifact` names and exact Java
module, imported-name, and member segments. There is no regex, expression,
callback, provider matcher, selector, fingerprint, value law, generic HOF
result, lazy/async/stream profile, or private value-graph-node field. Unknown
fields and enum values fail while the manifest is loaded.

This vocabulary is intentionally smaller than the internal kernel. Expanding it
requires a later API version; a provider cannot introduce a new operation by
spelling it in a string.

## Compilation and indexes

Each valid manifest compiles into a read-only `CompiledSemanticPackV1` before a
query begins. It exposes deterministic ordered indexes:

- contract id to typed contract;
- exact package coordinate to its version requirement;
- exact package/import/call coordinate to sorted contract ids;
- kernel operation to sorted contract ids.
- contract id to a canonical SHA-256 row digest.

Manifest path, directory order, JSON object-key order, hash-map iteration, and
process state do not affect those indexes. Set arities are sorted before
compilation. Duplicate ids, duplicate package coordinates, duplicate exact
contract coordinates plus arity, invalid cross-field combinations, and
out-of-range arities fail before analysis.

The compiled indexes feed the dependency/occurrence index and the two locked
consumers. Unlocked v1 summaries remain `metadata-only`; a valid lock that
allows only `near` reports `near-only`. Near authorization never changes
normalization, fingerprints, exact membership, witnesses, or oracle behavior.

## Locked dependency and occurrence evidence

Evidence construction runs once per query after builtin lowering. It is
available only when a validated `nose.semantic-pack-lock.v1` authorizes the row
and its requested lane; loading the same manifest without a lock produces no
row or occurrence evidence.

The first dependency reader accepts checked-in Maven POM XML only. It reads
top-level direct dependencies, top-level `dependencyManagement` versions, and
bounded `${property}` references from the project, parent coordinate, and
top-level `properties`. Profiles, plugins, external entities, unresolved
environment/system properties, Maven version ranges, snapshots, release
aliases, malformed XML, and files over the resource cap do not produce facts.
The Guava distribution suffixes `-jre` and `-android` are matched by their
three-component release version; other prerelease qualifiers stay closed. The
reader rechecks each locked content digest and never invokes Maven, a registry,
a build tool, an installer, provider code, or the network.

For an in-range unique dependency, the Java matcher supports two exact source
forms:

- an explicit type import followed by a qualified static call such as
  `ImmutableList.of(...)`;
- an explicit static-member import followed by a free call such as `of(...)`.

Every admitted occurrence records the dependency fact id, binding import id,
symbol proof id, receiver span/proof when the contract has an imported-type
receiver, and any builtin effect, call-target, domain, and place ids already
not provider-emitted IL. Exact package/module/member, arity, binding visibility,
and dependency liveness are checked. Wildcards, wrong-package or same-name local
APIs, shadowed/rebound bindings, conflicting or ambiguous versions, missing or
out-of-range dependencies, unsupported receiver dispatch, and evidence-broken
records produce no occurrence.

The index exposes deterministic row and dependency summaries plus lookup by row
or exact call span. Its collections are immutable after construction, it uses no
global registry, and its ids are query-local. Merely building it does not emit
IL evidence or change a family; the near consumer additionally requires a valid
near authorization, an admitted occurrence, and a matching builtin protocol
operation in an existing near candidate.

## Locked near consumer

The first consumer maps the closed `collection-factory` and `map-factory`
operations onto admitted builtin factory evidence. It annotates extracted units
once per query, after cache lookup, and can only raise the score of a candidate
already proposed by the ordinary value/shape/anchor indexes. It does not create
candidate pairs. The base score must be at least `0.60`; supporting protocol
evidence moves it one quarter of the remaining distance toward `1.0`, capped at
`0.95`. This opens a narrow default-threshold boundary without turning a
provider claim into equality proof.

Only the near detector reads these annotations. Semantic-only runs, exact
fingerprints, exact groups, `nose verify`, connected witnesses, copy-paste runs,
and `base=<ref>` remain unchanged. Removing the lock removes the annotations;
cached and uncached runs attach the same query-local evidence.

Affected family and member JSON includes pack and row digests, lane, trust,
operation, dependency coordinate/version/source pins, occurrence span, and
caveats. The top-level pack entry reports selected/admitted/rejected rows plus
admitted and influential occurrence counts. Rows with dependency blockers stay
visible in these counts but cannot annotate a unit.

The [#867 closeout receipt](../bench/semantic_pack/issue-867-locked-near-closeout-2026-07-19.v1.json)
binds the implementation and release-binary identities, focused lock/rollback
fixture, 9-repository no-pack parity, official-v0.19.0 semantic and near runtime
measurements, and the zero-false-merge verify result.

## Kernel source conformance and external-claim exact

An exact-capable v1 manifest declares closed source fixtures under
`conformance.fixtures`. Each fixture binds one row, a non-escaping source path,
a checked-in Maven dependency input, a `positive` or `hard-negative` kind, and
the matching closed expectation (`external-exact-match` or
`no-external-exact-match`). Run:

```sh
nose semantic-pack check semantic-packs/example.json \
  --receipt-out semantic-packs/example.receipt.json
```

The runner enforces 64 files and 1 MiB of fixture content plus a 2 MiB Maven
input cap, rejects symlinks, path escapes, and parse-error recovery, lowers
source with the product frontend, builds the ordinary
dependency/occurrence evidence index, injects only the kernel-owned collection
factory contract, then uses the product normalizer, strict exact-safety gate,
value graph, and exact detector. It never runs fixture programs, Maven,
provider commands, or downloaded code. Resource-limit and analysis failures
cannot satisfy a negative expectation.

The v1 receipt uses the [semantic-pack conformance receipt
schema](schemas/semantic-pack-conformance-receipt-v1.schema.json), which is
validated again at lock creation, status, and query time.
It binds the nose version, kernel capability, pack/version/semantic digest,
external-exact row and row digests, fixture paths and content digests,
dependency digests, expected and observed outcomes, and pass status. Lock
creation and every later status/query validation reparse the receipt and rehash
the declared fixtures. Missing, edited, stale, failed, differently selected, or
wrong-capability receipts fail before analysis.

After validation, nose emits a query-local external evidence record for each
dependency/import/symbol/receiver/arity-matched occurrence. The record can only
select the existing collection-sequence operation; it cannot name a private
value-graph node, fingerprint, new law, callback, or rewrite. Normalization and
strict exact safety independently recheck the record's external provenance,
dependency closure, current call anchor, unique admission, and arity.

Affected exact families and members use `semantic_pack_external_exact`, not
builtin semantic-law provenance. The payload names lane `external-exact`,
assurance `external-claim-exact`, pack/row digests, receipt and dependency
digests, occurrence span, and caveats including
`external-claim-not-builtin-certification`. This is a provider claim tested by
nose and explicitly authorized by the user; it is not nose certification of
the external API's universal behavior. Removing the lock restores ordinary
output.

## Semantic content digest

v1 reports a lowercase `sha256:<64 hex digits>` digest over canonical typed
semantic content. The canonical projection includes:

- API version and pack kind;
- sorted supported languages and declared package coordinates/version ranges;
- every typed contract field, sorted by contract id, with set arities sorted.

It excludes display text, provider/contact, repository, license, source
revision, local path, and the pack id/version. Those are provenance or separate
lock coordinates; the future project lock pins API version, pack id, pack
version, and semantic digest independently. Any change to a valid semantic
field changes the digest, while reordering JSON object keys does not.

v0 keeps its existing 64-bit id-derived report hash and has no semantic digest.
That distinction is intentional: a v0 manifest can never become influential by
being locked.

## Reporting and current limits

`nose capabilities` schema v7 lists both `nose.semantic-pack.v0` and
`nose.semantic-pack.v1`. `nose semantic-pack compatibility` lists the same
accepted versions. Query semantic-pack summaries conditionally add
`api_version`; v1 also adds `semantic_digest`. Compiled builtin summaries keep
their existing shape so no-pack product output is unchanged.

`nose semantic-pack check --format json` schema v4 includes `api_version`,
`semantic_digest`, and a separate `kernel_conformance` result. For v0 it keeps
the historical fixture-metadata behavior; v0 never receives a kernel receipt or
influence grant.

The project-lock layer is now defined by
[semantic-pack-project-lock](semantic-pack-project-lock.md). It pins API,
pack/version, nose compatibility, semantic digest, selected rows/channels,
dependency content, and optional receipts; it also rejects overlapping
cross-pack coordinates independent of load order.

Shipping and measuring a reviewed real external reference pack remains outside
this implementation slice.

## See also

- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md) defines
  the permanently metadata-only compatibility format.
- [semantic-pack-loading](semantic-pack-loading.md) describes local discovery
  and reporting.
- [semantic-pack-architecture](semantic-pack-architecture.md) owns the kernel
  boundary and behavior/performance gates.
- [semantic-pack-compatibility](semantic-pack-compatibility.md) defines version
  failure behavior.
