# Portable cache artifacts

The 0.20 Instant Monorepo cache uses one layered content-addressed store (CAS) contract for every
analysis stage. This page defines the #873 boundary: what is portable now, what is actively reused,
and what later dependency-aware issues may activate without changing the storage trust model.

## Layer contract

Every address is a SHA-256 digest over a domain separator, the stage id, that stage's schema, and
length-framed semantic inputs. A stage cannot read another stage's entry, and a schema change lands
at a different address. The six stable stage identities are:

| Stage | #873 state | Next consumer |
| --- | --- | --- |
| source snapshot | address space defined | #874 discovery/source inventory |
| raw lowered IL | portable codec and round-trip gate complete | #874 parse/lower reuse |
| export/dependency summary | address space defined | #874 dependency graph |
| resolved IL | portable codec and round-trip gate complete | #874 affected-closure resolution |
| units and syntax streams | actively read and written by `--cache-dir` | current query pipeline |
| global detection indexes | address space defined | #875 incremental detection |

Defining a stage is not a claim that queries already skip it. At #873 only units and syntax streams
are active, encoded as named MessagePack so serde-compatible schema evolution remains possible
without JSON's repeated decimal expansion of feature hashes. Discovery, source reads, parsing,
lowering, corpus resolution, global detection, and
presentation still run as described by the [incremental benchmark](incremental-cache-benchmark.md).
Raw/resolved payloads are not duplicated into the store before #874 can invalidate them precisely;
that avoids knowingly worsening the cache-size ratio merely to populate unused artifacts.

## Envelope and failure behavior

CAS v1 entries live below `cas-v1/<stage>/<digest-prefix>/`. A fixed binary header binds:

- the `NOSECAS1` magic and envelope schema;
- stage id and stage-local schema;
- the complete 256-bit requested address;
- exact payload length; and
- an independent SHA-256 checksum of the payload.

Writers finish a private temporary file before atomic publication. Readers validate every header
field, exact file length, and checksum before deserialization. A missing, truncated,
corrupt, wrong-stage, wrong-schema, or misplaced entry is a cache miss and recomputes; no failure
path returns unchecked bytes. Concurrent writers of one address converge on complete envelopes,
while a racing reader can at worst miss and recompute.

The payload checksum is deliberately separate from the semantic address. The address proves that
the requested stage inputs match; the checksum proves that storage returned the exact bytes that
were published for those inputs. Silent reuse therefore never rests on the previous u64
`valued_tree_hash` key.

## Portable IL identity

The raw and resolved codecs retain the full IL arena and reporting contract:

- nodes, ordered edges, root, spans, units, unit facets, suppression ranges, and language;
- complete evidence ids, anchors, kinds, statuses, dependencies, provenance, and every nested span;
- original unit/canonical-identifier symbol strings; and
- a stable script/style/markup subidentity for embedded containers.

Checkout-local paths and process-local `FileId` values are neutralized before serialization and
rebound on load. Interner symbols are serialized through a lexicographically sorted string table,
then interned into the destination corpus. Thus parallel interning order, process restart, and an
absolute checkout move do not change semantic identity. Embedded subidentity depends on analyzed
language, region root kind, and container kind rather than byte offsets or content, so moving or
editing a region does not silently turn it into another logical sub-file.

The resolved semantic digest hashes every report-affecting coordinate and all evidence content.
Changing only an evidence status, dependency, kind, anchor, or provenance changes the digest.
Paths and `FileId`s do not. The stage/schema domain still distinguishes the same portable bytes as
raw IL versus resolved IL.

## Executable gates

Focused tests prove:

- evidence-only changes miss, while repeating the changed artifact hits;
- identical source in a different absolute checkout reuses one payload and reports the new path;
- different interner insertion orders and `FileId`s produce one semantic digest;
- raw and resolved portable round trips produce byte-identical detector JSON;
- script, style, and markup regions have stable distinct subidentities; and
- corruption and truncation fail closed.

The existing #275 provider/importer integration test remains the cross-file safety gate, and the
benchmark's clean/empty/history equality remains the product-output authority. See
[continuous integration](continuous-integration.md) for current user-facing cache behavior.
