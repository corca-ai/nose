# Import-backed immutable provenance closeout

Issue #567 turns imported immutable values into dependency-backed semantic
evidence instead of another table of API exceptions. The capability is narrow:
exact admission may use an imported value only when import/export identity,
provider value shape, LibraryApi or domain proof, and provider/importer mutation
exclusion are all proven.

The durable metric artifact is
[issue-567-closeout.v1.json](../bench/recall_loss/issue-567-closeout.v1.json).
The phase artifacts are linked from
[recall-loss-recovery-loop](recall-loss-recovery-loop.md).

## What changed

Imported immutable provider values can now feed existing exact semantic families:

| family | imported coordinates now admitted |
|---|---|
| `map_default_lookup` | Python literal dict bindings, Java `Map.of` bindings, Go namespace map members, Rust imported const entry arrays consumed by `HashMap::from`, and JS/TS `new Map(...)` provider bindings |
| `membership_contains` | TypeScript/Rust imported literal collection bindings, JS/TS `new Set(...)` provider bindings, Python `set([...])` and `collections.deque([...])` provider bindings, and Java `List.of(...)`/`Set.of(...)` provider bindings |
| `string_affix` | TypeScript, Java, Rust, and Go imported string-affix coordinates |

The evidence substrate is shared:

- `ImportedLiteralSnapshot` provenance records that an importer consumed a
  provider-owned immutable value snapshot.
- Provider literal export safety lives in `nose-semantics`, not in per-consumer
  frontend shortcuts.
- Snapshot copy preserves provider source-origin spans and rewires dependency
  ids into the importer.
- Existing `LibraryApi`, `Domain`, `Import`, and `Symbol` evidence must already
  prove supported provider calls or binding coordinates.

This keeps the design aligned with capabilities over features: the new behavior
is a reusable proof lane that existing semantic packs consume, not a growing
selector allow-list.

## Closed boundaries

These remain intentionally closed:

- broad cross-file constant propagation;
- package ecosystem semantics;
- dynamic import, `eval`, reflection, and side-effecting re-export paths;
- raw import-coordinate sequences as value proof;
- provider mutation, importer rebinding, namespace shadowing, missing
  `LibraryApi` proof, and ambiguous factory/provider shapes.

The focused closeout inventory records 32 guard assertions or groups across
product fixtures, equivalence tests, frontend snapshot gates, and diagnostic
reason pins. The recall-loss hard gate stayed closed in every phase:
`false_merges == 0` and `canon_preservation_violations == 0`.

## Metrics

The phase deltas are:

| slice | positive delta | hard-negative / diagnostic guard |
|---|---:|---:|
| JS/TS `new Map(...)` imported defaults | `0/2 -> 2/2` | constructor snapshot guards `0/4 -> 4/4` |
| JS/TS `new Set(...)` imported membership | `0/2 -> 2/2` | included in constructor snapshot guards |
| Python imported collection factories | `0/2 -> 2/2` | collection snapshot guards `0/5 -> 5/5` |
| Java imported collection factories | `0/2 -> 2/2` | included in collection snapshot guards |
| recall-loss import snapshot census | `0/1 -> 1/1` | mutation/API/provider miss reasons pinned |
| aggregate boundary attribution | `0/1 -> 1/1` | broad aggregate miss bucket `3 -> 0` |

Runtime has two relevant measurements:

- The initial imported provenance slice was measured in the changelog at
  `0.92s -> 0.95s` median wall time (+3.3%) and `145.4ms -> 151.7ms`
  `import-resolve` (+4.3%) on `nose query crates all top=0 --mode semantic
  --format json`, with JSON bytes unchanged.
- The closeout phases after #583 through #585 were remeasured over five paired
  release runs on the same current `crates` input: median wall time
  `488.03ms -> 445.77ms` (-8.7%), `import-resolve` `69.6ms -> 72.0ms`
  (+3.4%), family count `31 -> 31`, and JSON bytes `67200 -> 67200`.

## Follow-up boundary

Do not continue #567 by widening aggregate child export safety. The remaining
large import-snapshot census buckets are module/export resolution scope:
`provider-module-missing` and `provider-export-missing`. If imported snapshots
stay the priority, start a separate module-resolution milestone rather than
relaxing imported literal admission.

That milestone is
[#587](https://github.com/corca-ai/nose/issues/587). Its checked-in starting
census is
[issue-587-module-export-census.v1.json](../bench/recall_loss/issue-587-module-export-census.v1.json).

## See also

- [recall-loss-diagnostics](recall-loss-diagnostics.md)
- [recall-loss-recovery-loop](recall-loss-recovery-loop.md)
- [semantic-kernel-snapshot](semantic-kernel-snapshot.md)
- [evidence-records](evidence-records.md)
- [source-facts](source-facts.md)
