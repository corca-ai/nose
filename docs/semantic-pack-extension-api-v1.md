# Typed semantic pack influence manifest v1

Status: implemented typed manifest and deterministic compiler; influence is not
enabled yet. `nose.semantic-pack.v0` remains permanently metadata-only.

## Purpose

v1 is the first semantic-pack format that nose can interpret without executing
provider code. It replaces v0's opaque `surface` and `semantics` JSON with a
small closed grammar. Loading a v1 file validates and compiles that grammar
before analysis starts, but the compiled rows still have `metadata-only`
influence until project locks, dependency and occurrence evidence, conflicts,
and lane-specific conformance are implemented.

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

Manifest path, directory order, JSON object-key order, hash-map iteration, and
process state do not affect those indexes. Set arities are sorted before
compilation. Duplicate ids, duplicate package coordinates, duplicate exact
contract coordinates plus arity, invalid cross-field combinations, and
out-of-range arities fail before analysis.

The indexes are deliberately not read by normalize, fingerprint, exact, near,
or detection consumers in this change. v1 summary rows therefore report
`source: local-manifest` and `influence: metadata-only`, just like v0, while
also reporting their API version and semantic digest.

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

`nose capabilities` schema v5 lists both `nose.semantic-pack.v0` and
`nose.semantic-pack.v1`. `nose semantic-pack compatibility` lists the same
accepted versions. Query semantic-pack summaries conditionally add
`api_version`; v1 also adds `semantic_digest`. Compiled builtin summaries keep
their existing shape so no-pack product output is unchanged.

`nose semantic-pack check --format json` schema v3 includes `api_version` and
`semantic_digest` for each manifest. For v1 it validates and compiles the typed
content, but it does not invent v0 fixture rows or report an influence grant.

The following work remains outside this slice:

- content-pinned project locks, trust limits, and overlap conflicts;
- dependency/version readers and occurrence evidence;
- near consumers and the separate external-claim exact lane;
- source-analyzing conformance receipts and a real external reference pack.

## See also

- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md) defines
  the permanently metadata-only compatibility format.
- [semantic-pack-loading](semantic-pack-loading.md) describes local discovery
  and reporting.
- [semantic-pack-architecture](semantic-pack-architecture.md) owns the kernel
  boundary and behavior/performance gates.
- [semantic-pack-compatibility](semantic-pack-compatibility.md) defines version
  failure behavior.
