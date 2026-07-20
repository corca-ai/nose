# Portable cache artifacts

The 0.20 Instant Monorepo cache uses one layered content-addressed store (CAS) contract for every
analysis stage. #873 defined the portable trust boundary; #874 activates dependency-aware source,
raw-IL, export-summary, and resolved-IL reuse without changing that storage model.

## Layer contract

Every address is a SHA-256 digest over a domain separator, the stage id, that stage's schema, and
length-framed semantic inputs. A stage cannot read another stage's entry, and a schema change lands
at a different address. The six stable stage identities are:

| Stage | 0.20 development state | Identity / consumer |
| --- | --- | --- |
| source snapshot | actively read and written | clean Git blob id or exact content SHA-256 |
| raw lowered IL | actively read and written | source snapshot; current path/`FileId` rebound |
| export/dependency summary | actively written and checksummed | deterministic export graph and SCC closure |
| resolved IL | actively read and written | raw IL plus the region's dependency context |
| units and syntax streams | actively read and written | resolved IL plus unit-affecting options |
| global detection indexes | address space defined | #875 incremental detection |

Raw and resolved IL use named MessagePack wrapped in fast LZ4 blocks; the decoder rejects a claimed
region expansion above 512 MiB before allocation. The units layer stays named MessagePack without
compression because those payloads are already compact feature hashes and dominate warm reads.
Discovery still walks the selected roots so additions and deletions are visible, but a clean
tracked raw hit needs no source read or parse in the lowering stage. Dependency summaries scan raw
IL, compute consumer-visible literal surfaces, collapse cycles deterministically, and resolve
imports against current module/package facts. Only resolved misses run corpus mutation; hits are
rebound afterward. Global detection, source-line frequency reads, family construction/ranking,
and presentation remain per-query work for #875.

## Dependency-aware invalidation

Each region's resolved address combines its path-independent raw semantic digest with a dependency
context digest. That context contains its own export coordinates, resolved imported-binding and
namespace-member surfaces, Rust module/re-export outcomes, Java/Go package facts, unresolved
language catalogs, and the Swift global shadow/overload/conformance sentinel when applicable.
Export surfaces hash copied literal structure and evidence but omit source coordinates and local
evidence ids. Therefore changing private provider implementation does not invalidate importers;
changing an exported literal does, including the #275 provider/importer case.

The export graph incorporates imports used by exported values. Strongly connected components are
collapsed before hashing, then the component DAG is resolved dependency-first. This gives cycles
and re-exports a deterministic fixed point without depending on traversal order. Cache identities
never include sorted corpus indexes, so adding or deleting an earlier path does not invalidate
unrelated artifacts merely because `FileId`s moved.

Unknown static dependencies include the language's export catalog in their key. This can invalidate
more than necessary when an unrelated export changes, but it cannot reuse a stale unresolved fact;
the path is listed under `over_invalidated`. Discovery membership, semantic-pack influence, and
corpus-global line-statistics digests are carried in diagnostics now even though their downstream
global stages remain #875 work.

The mutable `state-v1` workspace record exists only to explain changes and deleted paths. It is not
a cache-key or reuse input: deleting or corrupting it loses reason history, never correctness.
Under `NOSE_CACHE_STATS=1`, stderr includes one `nose.invalidation/v1` JSON closure alongside the
backward-compatible `[cache]` units line. Cold start is summarized globally instead of listing
every file; history-bearing runs list the exact changed/deleted closure. Resolved `passthrough`
counts identify regions whose raw IL is already the correct resolved form, so no duplicate payload
is stored.

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

The #275 provider/importer integration test remains the cross-file safety gate. Focused gates also
cover unchanged export surfaces, shifted `FileId`s with an active import edge, Swift global
barriers, unresolved-dependency over-invalidation, and Git-blob/content identity selection. The
benchmark's clean/empty/history equality remains the product-output authority. See
[continuous integration](continuous-integration.md) for current user-facing cache behavior.

## Checked #873 performance evidence

The checked [`issue-873-portable-cas-sympy-paired-2026-07-20.v1.json`](../bench/cache/issue-873-portable-cas-sympy-paired-2026-07-20.v1.json)
contains 30 alternating AB/BA replays against the checksum-verified published v0.19.0
`aarch64-apple-darwin` binary. Both roles independently passed exact clean/empty/history output
equivalence across all 180 raw rows.

| Phase | Official p50 / p95 | #873 p50 / p95 | p50 delta |
| --- | ---: | ---: | ---: |
| Clean | 1097.20 / 1247.37 ms | 1119.15 / 1251.12 ms | +2.0% |
| Empty store | 1180.43 / 1346.40 ms | 1235.86 / 1422.30 ms | +4.7% |
| Warm store | 734.30 / 857.68 ms | 774.74 / 900.45 ms | +5.5% |

The clean result remains within the epic's 5% gate. The warm delta prices full checksum validation
instead of trusting file presence, while named MessagePack limits that cost: the store falls from
380,153,028 to 190,665,950 bytes (-49.8%), and warm p50 RSS falls from 1,066,008,576 to 996,851,712
bytes (-6.5%). This remains the before-#874 foundation measurement; the #874 comparison below owns
the activated parse/resolve result, while #875 owns repeated global work.
