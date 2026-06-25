# Semantic pack architecture

Status: active rulebook for the builtin-first semantic-pack architecture started
by [issue #473](https://github.com/corca-ai/nose/issues/473) and stabilized by
[issue #484](https://github.com/corca-ai/nose/issues/484).

This page defines the target boundary for builtin and external semantic packs.
It is the contributor-facing rulebook for moving language, stdlib, library, and
law knowledge out of privileged kernel-only paths without changing product
behavior by accident.

## Terminology

Use `builtin` for packs maintained and shipped by nose.

| lane | meaning |
|---|---|
| `builtin-default` | shipped with nose and enabled by default |
| `builtin-optional` | shipped with nose but opt-in until default-surface risk is accepted |
| `external-opt-in` | provider/user responsibility; enabled only by explicit user choice |

Current v0 manifest parsing still accepts legacy compatibility spellings, but
machine output uses builtin vocabulary:

- `builtin-default`: trust label for builtin packs enabled by default;
- `builtin-optional`: trust label for nose-owned builtin packs not enabled by default;
- `compiled-builtin`: source/loading label for compiled builtin packs.

Use the builtin vocabulary in docs, internal naming, and future schema planning.
A public enum or field-value rename requires a schema/capabilities update, not a
silent string change.

The broad `nose.first_party` pack id is a temporary compatibility descriptor for
compiled semantic knowledge that has not yet moved behind narrower builtin pack
ids. New ownership should move toward explicit ids such as `nose.lang.python`,
`nose.lang.javascript-typescript`, `nose.lang.go`, `nose.lang.rust`,
`nose.lang.java`, `nose.lang.c`, `nose.lang.ruby`, `nose.lang.swift`,
`nose.lang.css`, `nose.lang.html`,
`nose.python.builtins.collection_factories`,
`nose.python.stdlib.collection_factories`,
`nose.python.stdlib.math`,
`nose.javascript.builtins.promise`,
`nose.javascript.builtins.array`,
`nose.javascript.builtins.boolean`,
`nose.javascript.builtins.regex`,
`nose.javascript.builtins.static_index_membership`,
`nose.javascript.builtins.collection_constructors`,
`nose.ruby.stdlib.set`, `nose.rust.stdlib.vec`,
`nose.rust.stdlib.option`,
`nose.rust.stdlib.result`,
`nose.rust.stdlib.integer_methods`,
`nose.rust.stdlib.collection_factories`, `nose.rust.stdlib.map_factories`,
`nose.swift.stdlib.collection_factories`,
`nose.java.stdlib.math`,
`nose.java.stdlib.map_factories`, `nose.java.stdlib.map_entries`,
`nose.java.stdlib.collection_factories`,
`nose.java.stdlib.collection_constructors`,
`nose.java.stdlib.static_collection_adapters`,
`nose.protocols.map_get`,
`nose.protocols.map_get_default`,
`nose.protocols.free_function_builtins`,
`nose.protocols.receiver_membership`,
`nose.protocols.map_key_views`,
`nose.protocols.property_builtins`,
`nose.protocols.builtin_method_calls`,
`nose.go.stdlib.namespace_calls`,
`nose.protocols.iterator_identity_adapters`,
`nose.python.stdlib.type_domain`, and
`nose.value_graph.laws`.

## Legacy Compatibility Inventory

`nose.first_party` is no longer a place for new active semantic ownership. It
remains only as a compatibility surface while older v0 manifests, IL snapshots,
tests, and helper aliases still need the legacy spelling.

Current intentional compatibility uses are:

- the empty compiled descriptor for the `nose.first_party` wire id;
- parser aliases for legacy `default-first-party` and `first-party-optional`
  trust labels, which still map to builtin trust lanes before local manifests
  that claim builtin ownership are rejected;
- `EvidenceEmitter::FirstParty` and `FIRST_PARTY_*`/`first_party_*` helper
  aliases for older callers and fixtures;
- legacy broad-row hard negatives and update-in-place tests that prove old
  broad provenance does not open exact admission when a narrower builtin pack is
  required.

Active official semantics should instead use narrow builtin packs. The current
remaining broad slice is the generic `nose.protocols.builtin_method_calls` pack
for method-call rows that have not yet earned a clearer stdlib/protocol
boundary. Go namespace calls are no longer part of that catch-all: `fmt.Print*`,
`strings.HasPrefix`/`HasSuffix`, `strings.Contains`, and `slices.Contains` are
owned by `nose.go.stdlib.namespace_calls`.

Removal condition: delete the compatibility descriptor and legacy helper aliases
only in a schema/capabilities migration that drops v0 first-party spellings and
updates checked-in IL/test fixtures. Until then, new code should treat
`nose.first_party` as compatibility-only and link a follow-up issue if it must
temporarily add active semantics there.

## Boundary

The semantic kernel owns the stable admission machinery:

- IL and evidence vocabulary;
- evidence anchors and dependency validation;
- fail-closed admission policy;
- trust and channel policy;
- pack loading, registry, capabilities, and provenance reporting.

Packs own language, library, and law knowledge:

- file, language, and embedded-region identity;
- parser and lowering entrypoints for builtin compiled language packs;
- source, import, symbol, domain, type, guard, place, effect, call-target,
  library API, and sequence-surface evidence producers;
- API contract rows and law rows;
- demand and effect profiles attached to admitted operations;
- conformance fixtures, hard negatives, proof/oracle links, and unsupported
  boundaries;
- pack metadata, versioning, trust, and provenance.

Builtin language packs may keep parser and lowering implementation in tree. The
first goal is pack ownership, provenance, and shared admission vocabulary, not an
external parser plugin runtime.

Promotion from external or optional support to official builtin support is
defined by [semantic-pack-adoption](semantic-pack-adoption.md). Promotion changes
ownership, trust, default enablement, CI gates, and rollback responsibility; it
does not create a separate semantic vocabulary.
Compatibility policy for manifest API versions, nose version ranges, kernel
vocabulary migration, and fail-closed external influence is defined by
[semantic-pack-compatibility](semantic-pack-compatibility.md).
Future large-ecosystem work is tracked as narrow builtin candidates in
[semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md),
not as broad default-enabled ecosystem packs.

The current pre-release boundary review is recorded in
[semantic-pack-boundary-review-2026-06-22](semantic-pack-boundary-review-2026-06-22.md).
That review found no release-blocking kernel/pack split issue, with the explicit
caveat that builtin packs are still compiled Rust descriptors and in-tree
evidence producers rather than an external plugin runtime.

## Contributor Rule

New builtin language, stdlib, library, or law semantics should be pack-owned. If
a change must add a temporary kernel/frontend shim before a pack-owned path
exists, the PR must link the tracking issue and state the removal condition.

Do not add new raw selector, raw name, raw type, or raw tag admission checks when
the same fact can be represented by a pack-owned evidence producer or contract
row. Selectors and syntax may be inputs to evidence production; they are not
admission proof by themselves.

## Product Behavior Gate

The pack migration is structural first. Descriptor, registry, naming, reporting,
and provenance changes should not change which clone families are found, hidden,
or accepted as exact/near unless the PR explicitly says it changes behavior.

Every implementation PR for this migration must run a product output comparison,
not just unit tests. Report changed family counts, surface counts, family shapes,
fragment reason codes, semantic-law provenance, or query JSON schema fields.
Classify each drift as one of:

- intentional metadata drift;
- precision improvement;
- measured recall change;
- bug fix.

Behavior-change defaults:

- Descriptor-only and reporting-only phases should preserve family output except
  intentional top-level pack metadata such as `semantic_packs`.
- Moving existing builtin semantic surfaces behind pack descriptors should
  preserve exact/near results unless the old path was demonstrably unsafe.
- Any widening of exact acceptance requires dependency-backed pack evidence and
  hard negatives.
- Any reduction in exact acceptance must be documented as a fail-closed
  precision fix or deferred recall with follow-up tracking.
- Metadata-only external packs must not change families, ranking, witnesses,
  surfaces, or exact/near results.

## Performance Gate

The migration should be performance-neutral or performance-positive. A pack
boundary must not put manifest parsing, string-heavy lookup, dynamic dispatch,
global locks, or repeated registry scans on per-node or per-unit hot paths.
Builtin descriptors should be static data or once-built indexes. External
manifest loading should happen before analysis, and metadata-only packs must not
add work inside normalize, detect, value-graph, fragment, or oracle loops.

Use the product query-regression path when a corpus is available:

```sh
cargo build --release --bin nose
python3 bench/type4/query_regression/query_regression.py baseline \
  --nose ./target/release/nose \
  --repos-root /path/to/main/bench/repos \
  --build-ref "main@$(git rev-parse --short HEAD)"
python3 bench/type4/query_regression/query_regression.py compare \
  --nose ./target/release/nose \
  --repos-root /path/to/main/bench/repos
```

The measured product command is:

```sh
nose query <repo> all top=0 --mode semantic --format json
```

For small or corpus-free changes, also run:

```sh
python3 bench/type4/query_regression/query_regression.py selftest
cargo bench -p nose-detect --bench pipeline
```

Use `<= 5%` median wall-clock and `NOSE_TIME` phase growth with a 5 ms floor as
the normal target. Any repo or phase above that target needs a root-cause note
and either an optimization or explicit follow-up. Any `> 10%` median growth on
the query-regression subset, or any HoF smoke budget failure, should block the
PR unless there is a documented product decision accepting the cost. Report
material binary-size growth from descriptor metadata or static tables, and
justify large static tables by the behavior or performance benefit they unlock.

## Issue #473 Phase Order

These phases are specific to issue #473. Older roadmap phase headings record
previous semantic-kernel tranches.

1. **Phase 0, boundary and measurement:** align docs, terminology, trust lanes,
   compatibility labels, contributor rules, and behavior/performance gates.
2. **Phase 1, builtin descriptor registry:** add a small descriptor shape for compiled
   builtin packs without adding plugin lifecycle, sandboxing, remote registries,
   or external execution.
3. **Phase 2, query reporting:** carry the active `SemanticPackSet` through
   `nose query` and add top-level `semantic_packs` to query JSON schema v7.
   Update [query-json](query-json.md) and [capabilities](capabilities.md) in
   the same PR because older query JSON schemas reject unknown fields.
4. **Phase 3, reference stdlib pack:** make `nose.python.stdlib.type_domain` the first
   end-to-end descriptor-backed builtin stdlib pack.
5. **Phase 4, builtin language slices:** keep official parser/lowering
   implementation in tree, but report builtin language ownership through
   `nose.lang.<language>` descriptors. The registry now has descriptor metadata
   for Python, JavaScript/TypeScript, Go, Rust, Java, C, Ruby, Swift, CSS, and
   HTML/Vue/Svelte embedded-region lowering. Generic language-core evidence and
   source-fact provenance now emit through those language packs, including
   immutable local/module binding-domain proof, normalize/front-end place/effect
   proof, normalize call-target/imported-occurrence proof, and module-import
   immutable literal export/snapshot proof. `nose.lang.c` also covers the
   explicit unsigned 32-bit byte-lane cast source-fact producer.
6. **Phase 5, builtin stdlib/library/law packs:** move official semantic rows behind
   narrow builtin pack ids and shared admitted-contract resolvers. The first
   slice is `nose.python.builtins.collection_factories`, which owns Python
   `list`, `set`, `frozenset`, and `tuple` collection-factory API occurrence
   provenance. The Python stdlib collection slice is
   `nose.python.stdlib.collection_factories`, which owns `collections.deque`
   imported binding, alias, and namespace collection-factory API occurrence
   provenance. The current Python stdlib math slice is
   `nose.python.stdlib.math`, which owns `math.prod` imported namespace product
   reduction API occurrence provenance. The Ruby stdlib Set slice is
   `nose.ruby.stdlib.set`, which owns Ruby `require "set"; Set.new(...)`
   collection-factory API occurrence provenance.
   The Rust stdlib Vec slice is `nose.rust.stdlib.vec`, which owns Rust
   `Vec::new` and `vec!` collection-factory API occurrence provenance.
   The current Rust stdlib Option slice is `nose.rust.stdlib.option`, which
   owns Rust `Some`, `None`, and `and_then` Option API occurrence provenance.
   The current Rust stdlib Result slice is `nose.rust.stdlib.result`, which
   owns `Ok`/`Err` constructor channel provenance and exact-Result
   `is_ok`/`is_err` predicate occurrence provenance.
   The current Rust stdlib integer-method slice is
   `nose.rust.stdlib.integer_methods`, which owns primitive integer
   `abs`/`min`/`max`/`clamp` method API occurrence provenance. The current
   Java stdlib Math slice is `nose.java.stdlib.math`, which owns Java
   `Math.abs`, `Math.min`, and `Math.max` scalar integer API occurrence
   provenance.
   The current JavaScript builtins Promise slice is
   `nose.javascript.builtins.promise`, which owns JS/TS `Promise.resolve` and
   `.then` Promise API occurrence provenance.
   The current JavaScript builtins Array slice is
   `nose.javascript.builtins.array`, which owns JS/TS `Array.from` and
   `Array.isArray` API occurrence provenance.
   The current JavaScript builtins Boolean slice is
   `nose.javascript.builtins.boolean`, which owns JS/TS `Boolean(...)` API
   occurrence provenance.
   The current JavaScript builtins regex slice is
   `nose.javascript.builtins.regex`, which owns JS/TS regex literal `.test(...)`
   API occurrence provenance.
   The current JavaScript builtins static-index slice is
   `nose.javascript.builtins.static_index_membership`, which owns JS/TS static
   `indexOf`/`findIndex` membership API occurrence provenance.
   The current JavaScript builtins collection-constructor slice is
   `nose.javascript.builtins.collection_constructors`, which owns JS/TS
   `new Set(...)` and `new Map(...)` API occurrence provenance.
   The Rust stdlib collection slice is
   `nose.rust.stdlib.collection_factories`, which owns selected
   `std::collections::{HashSet,BTreeSet,VecDeque}::from` collection-factory API
   occurrence provenance. The Rust stdlib map slice is
   `nose.rust.stdlib.map_factories`, which owns selected
   `std::collections::{HashMap,BTreeMap}::from` map-factory API occurrence
   provenance. The current Swift stdlib collection-factory slice is
   `nose.swift.stdlib.collection_factories`, which owns Swift
   `Array(sequence)`, `Set(sequence)`, and
   `Dictionary(uniqueKeysWithValues:)` occurrence provenance while shadowed
   type names, same-corpus typealias shadows, wrong labels, implicit tuple-entry
   shape, and static duplicate-key inputs stay closed. The current Java stdlib
   map slice is
   `nose.java.stdlib.map_factories`, which owns `java.util.Map.of`,
   `Map.ofEntries`, `Collections.emptyMap`, and `Collections.singletonMap`
   map-factory API occurrence provenance. The current Java stdlib map-entry
   slice is `nose.java.stdlib.map_entries`, which owns
   `java.util.Map.entry` map-entry API occurrence provenance.
   The current Java stdlib collection slice is
   `nose.java.stdlib.collection_factories`, which owns `java.util.List.of`,
   `Set.of`, `Arrays.asList`, `Collections.emptyList`,
   `Collections.emptySet`, `Collections.singleton`, and
   `Collections.singletonList` collection-factory API occurrence provenance.
   The current Guava immutable collection slice is
   `nose.java.ecosystem.guava.immutable_collection_factories`, which owns
   `ImmutableList.of`, `ImmutableSet.of`, and `ImmutableMap.of` factory API
   occurrence provenance under exact `com.google.common.collect` import proof;
   `ImmutableMap.of` is limited to the supported 0-10 entry overloads, static
   null elements/key-values and duplicate static map keys stay closed, and
   `copyOf` remains unsupported until source-domain proof exists.
   The current Java stdlib collection-constructor slice is
   `nose.java.stdlib.collection_constructors`, which owns empty `new
   ArrayList<>()` and `new LinkedList<>()` collection-constructor API occurrence
   provenance. The current Java stdlib static collection adapter slice is
   `nose.java.stdlib.static_collection_adapters`, which owns
   `java.util.Arrays.stream` API occurrence provenance.
   The current map-get protocol slice is `nose.protocols.map_get`, which owns
   Java/Rust/JS-family `map.get(key)` API occurrence provenance under the shared
   exact-map receiver contract.
   The current map-get-default protocol slice is `nose.protocols.map_get_default`,
   which owns Python `dict.get(key, default)`, Ruby `Hash#fetch(key, default)`
   or zero-arg block fallback, and Java `Map.getOrDefault(key, default)` API
   occurrence provenance under the shared exact-map receiver contract.
   The current free-function builtin protocol slice is
   `nose.protocols.free_function_builtins`, which owns unshadowed
   Python/Go/Swift free-name builtin API occurrence provenance.
   The current Python iterator-builtin protocol slice is
   `nose.protocols.iterator_builtins`, which owns dependency-backed Python
   builtin iterator producer and terminal occurrence provenance for `map`,
   `filter`, `zip`, `enumerate`, `any`, and `all`. Lazy producer semantics and
   `list`/`tuple`/`set` materialization require the iterator/source proof and
   terminal factory proof rather than a wider builtin-name allowlist.
   The current receiver-membership protocol slice is
   `nose.protocols.receiver_membership`, which owns receiver-method membership
   API occurrence provenance for map, collection, and set-or-map receiver
   contracts. Go `slices.Contains` remains outside this slice because it is a
   namespace-function-style contract.
   The current map-key-view protocol slice is `nose.protocols.map_key_views`,
   which owns Python/Ruby `keys`, Java `keySet`, JS-family `Map.keys()` API
   occurrence provenance under the shared exact-map receiver contract, and
   JS/TS `Object.keys` provenance under static object-argument proof.
   The current property-builtin protocol slice is
   `nose.protocols.property_builtins`, which owns JS/TS/HTML-family and Java
   `.length`, plus Swift `count` and `isEmpty`, API occurrence provenance under
   receiver-domain proof.
   The current builtin method-call protocol slice is
   `nose.protocols.builtin_method_calls`, which owns generic method-call and
   namespace-call builtin semantics that have not moved to a narrower protocol
   pack, such as append, cardinality, string-affix, option-default, reduction,
   and HOF-style receiver method rows. The current Go stdlib namespace-call
   slice is `nose.go.stdlib.namespace_calls`, which owns `fmt.Print*`,
   `strings.HasPrefix`/`HasSuffix`, `strings.Contains`, and `slices.Contains`
   API occurrence provenance under imported-namespace proof. `strings.Contains`
   uses the separate `StringContains` semantic rather than collection
   membership.
   The current iterator identity adapter protocol slice is
   `nose.protocols.iterator_identity_adapters`, which owns Rust
   `iter`/`into_iter`/`iter_mut`/`collect`/`to_vec`/`copied`/`cloned` and Java
   `.stream()` API occurrence provenance under the shared receiver-proof
   contract.
7. **Phase 6, external influence:** only after the builtin path is proven, start with a
   small data-only external row class behind explicit opt-in trust gates.
8. **Phase 7, adoption gates:** define `external-opt-in -> builtin-optional` and
   `builtin-optional -> builtin-default` promotion criteria, rollback behavior,
   corpus regression, docs, and performance budgets.

## See also

- [semantic-kernel](semantic-kernel.md)
- [semantic-kernel-roadmap](semantic-kernel-roadmap.md)
- [semantic-pack-adoption](semantic-pack-adoption.md)
- [semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md)
- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md)
- [semantic-pack-loading](semantic-pack-loading.md)
