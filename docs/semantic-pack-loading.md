# Semantic pack loading

Status: nose can validate local semantic-pack v0 manifests, compile typed v1
manifests, and validate content-pinned v1 project locks before `nose query`.
It also provides separate local check/lock/status commands. External packs are
explicit opt-ins. Unlocked v0/v1 packs are `metadata-only`; a content-pinned v1
lock may authorize narrow near influence, and a matching kernel conformance
receipt may additionally authorize a closed collection-factory exact claim.
Opaque v0 producer, contract, and value-law declarations remain data-only.
Typed v1 contracts compile into deterministic indexes and a semantic digest;
consumers read only locked, dependency-backed occurrences. `nose capabilities`
reports the same boundary with
`external_pack_influence = "metadata-or-locked-near-or-receipt-backed-external-claim-exact"`,
`external_exact_operations = ["collection-factory"]`, the current blocker
labels, and `external_pack_execution = "none"`.

## Local entry points

Use `--semantic-pack <file-or-dir>` on `nose query` to opt into local pack
metadata validation for one run:

```sh
nose query src --format json --semantic-pack semantic-packs/python-math-prod.json
```

Commit stable project opt-ins in `nose.toml`:

```toml
[query]
semantic-packs = ["semantic-packs/python-math-prod.json"]
```

Each path may be a manifest file or a directory. Paths from `[query].semantic-packs`
are resolved relative to the config file that declared them; paths from
`--semantic-pack` are resolved by the shell/current working directory like other
CLI paths. Directory loading reads direct `*.json` children in sorted order; it
does not recurse and it does not contact a registry or network service.

For a reviewed, content-pinned typed v1 set, configure a project lock instead:

```toml
[query]
semantic-pack-lock = "nose.semantic-pack-lock.json"
```

or pass `--semantic-pack-lock <file>`. A lock is mutually exclusive with
unlocked manifest paths and owns the full external pack set. nose validates all
manifest, dependency, selection, channel, receipt, path, and conflict pins
before lowering source. See
the [project-lock guide](semantic-pack-project-lock.md) for the pinning model,
commands, and failure behavior.

## Conformance entry point

Pack authors and users can check the same local manifest paths without loading
them into an analysis run:

```sh
nose semantic-pack check semantic-packs/python-math-prod.json
nose semantic-pack check semantic-packs --format json
nose semantic-pack check semantic-packs/typed-exact.json \
  --receipt-out semantic-packs/typed-exact.receipt.json
```

The conformance command validates manifest structure, trust policy, dependency
references, exact-capable contract obligations, fixture references, and closed
expectations. For v1 external-exact rows it analyzes bounded fixture source
through the product kernel and may write a content-bound receipt. It does not
execute fixture programs, external producers, provider commands, or downloaded
code, and it does not certify semantic correctness. The [semantic-pack
conformance guide](semantic-pack-conformance.md) defines its exact boundary.

For v1, the command validates the closed Java/Maven package-API grammar and
builds its canonical digest and indexes. v1 does not reuse v0's opaque row or
fixture declarations. See [semantic-pack-extension-api-v1](semantic-pack-extension-api-v1.md).

Create and inspect a local project authorization without fetching or installing
anything:

```sh
nose semantic-pack lock semantic-packs/guava.json --dependency pom.xml
nose semantic-pack status nose.semantic-pack-lock.json --format json
```

## Trust policy

Trust is separate from channel eligibility.

- Compiled builtin packs are enabled by default and are the only packs that
  influence builtin evidence and contracts. Machine output reports them with
  `compiled-builtin` source and `builtin-default` trust. Older v0 manifest
  examples may still use legacy first-party trust aliases, but local manifests
  that claim builtin trust are rejected after parsing. `nose.first_party` remains
  the legacy wire id for the temporary broad builtin compatibility facade; new
  in-tree code should refer to that role as builtin compatibility rather than
  first-party ownership.
  `nose.lang.python`, `nose.lang.javascript-typescript`, `nose.lang.go`,
  `nose.lang.rust`, `nose.lang.java`, `nose.lang.c`, `nose.lang.ruby`,
  `nose.lang.swift`, `nose.lang.css`, and `nose.lang.html` report official
  parser/lowering ownership metadata plus generic language-core and source-fact
  producer provenance for builtin language support while the implementation
  stays in tree. Immutable local/module binding-domain proof, normalize/front-end
  place/effect proof, normalize call-target/imported-occurrence proof, and
  module-import immutable literal export/snapshot proof also use the matching
  builtin language-core producer. `nose.lang.c` owns the specialized
  unsigned-cast source-fact producer used by exact byte-pack admission;

  Builtin pack ids, including the `nose.lang.*` language descriptor ids, are
  reserved. A local external manifest that claims one of those ids is rejected
  as a duplicate pack id. This is intentional fail-closed behavior: external
  packs may use the same vocabulary, but they cannot impersonate shipped nose
  ownership or default trust.
  `nose.python.builtins.collection_factories` is the first narrow Python
  builtins pack for `list`, `set`, `frozenset`, and `tuple` collection factory
  API occurrence provenance;
  `nose.python.stdlib.collection_factories` owns Python `collections.deque`
  imported binding, alias, and namespace collection factory API occurrence
  provenance;
  `nose.python.stdlib.math` owns Python `math.prod` imported namespace product
  reduction API occurrence provenance;
  `nose.ruby.stdlib.set` owns Ruby `require "set"; Set.new(...)` collection
  factory API occurrence provenance;
  `nose.rust.stdlib.vec` owns Rust `Vec::new` and `vec!` collection factory API
  occurrence provenance;
  `nose.rust.stdlib.option` owns Rust `Some`, `None`, and `and_then` Option API
  occurrence provenance;
  `nose.rust.stdlib.result` owns Rust `Ok`/`Err` Result constructor provenance
  and exact-Result `is_ok`/`is_err` predicate occurrence provenance, with local
  `Result` type shadows closed for unqualified receiver proofs;
  `nose.rust.stdlib.integer_methods` owns primitive integer
  `abs`/`min`/`max`/`clamp` method API occurrence provenance;
  `nose.java.stdlib.math` owns Java `Math.abs`, `Math.min`, and `Math.max`
  scalar integer API occurrence provenance;
  `nose.javascript.builtins.promise` owns JS/TS `Promise.resolve`,
  `Promise.reject`, `.then`, and `.catch` Promise API occurrence provenance;
  `nose.javascript.builtins.array` owns JS/TS `Array.from`, `Array.isArray`,
  exact-Array receiver `map`/`filter`/`flatMap`, and `some`/`every` API
  occurrence provenance;
  `nose.javascript.builtins.boolean` owns JS/TS `Boolean(...)` API occurrence
  provenance;
  `nose.javascript.builtins.regex` owns JS/TS regex literal `.test(...)` API
  occurrence provenance;
  `nose.javascript.builtins.static_index_membership` owns JS/TS static
  `indexOf`/`findIndex` membership API occurrence provenance;
  `nose.javascript.builtins.collection_constructors` owns JS/TS `new Set(...)`
  and `new Map(...)` API occurrence provenance;
  `nose.rust.stdlib.collection_factories` owns selected Rust
  `std::collections::{HashSet,BTreeSet,VecDeque}::from` collection factory API
  occurrence provenance;
  `nose.rust.stdlib.map_factories` owns selected Rust
  `std::collections::{HashMap,BTreeMap}::from` map factory API occurrence
  provenance;
  `nose.swift.stdlib.collection_factories` owns Swift `Array(sequence)`,
  `Set(sequence)`, and `Dictionary(uniqueKeysWithValues:)` collection/map
  factory API occurrence provenance;
  `nose.java.stdlib.map_factories` owns Java `java.util.Map.of` and
  `java.util.Map.ofEntries` map factory API occurrence provenance;
  `nose.java.stdlib.map_entries` owns Java `java.util.Map.entry` map-entry API
  occurrence provenance;
  `nose.java.stdlib.collection_factories` owns Java `java.util.List.of`,
  `Set.of`, and `Arrays.asList` collection factory API occurrence provenance;
  `nose.java.ecosystem.guava.immutable_collection_factories` owns Guava
  `ImmutableList.of`, `ImmutableSet.of`, and `ImmutableMap.of` factory API
  occurrence provenance while `copyOf` remains closed;
  `nose.java.stdlib.collection_constructors` owns Java empty `new
  ArrayList<>()` and `new LinkedList<>()` collection constructor API occurrence
  provenance;
  `nose.java.stdlib.static_collection_adapters` owns Java
  `java.util.Arrays.stream` static collection adapter API occurrence provenance;
  `nose.protocols.map_get` owns Java/Rust/JS-family `map.get(key)` API
  occurrence provenance under exact-map receiver proof;
  `nose.protocols.map_get_default` owns Python `dict.get(key, default)`, Ruby
  `Hash#fetch(key, default)` or zero-arg block fallback, and Java
  `Map.getOrDefault(key, default)` API occurrence provenance under exact-map
  receiver proof;
  `nose.protocols.free_function_builtins` owns unshadowed Python/Go/Swift
  free-name builtin API occurrence provenance;
  `nose.protocols.receiver_membership` owns receiver-method membership API
  occurrence provenance for map, collection, and set-or-map receiver contracts;
  `nose.protocols.map_key_views` owns Python/Ruby `keys`, Java `keySet`, and
  JS-family `Map.keys()` API occurrence provenance under exact-map receiver
  proof;
  `nose.protocols.property_builtins` owns JS/TS/HTML-family and Java `.length`,
  plus Swift `count` and `isEmpty`, API occurrence provenance under
  receiver-domain proof;
  `nose.protocols.builtin_method_calls` owns generic method-call and
  namespace-call builtin semantics that have not moved to a narrower protocol
  pack;
  `nose.protocols.string_affix_predicates` owns case-sensitive prefix/suffix API
  occurrence provenance under exact string receiver proof for receiver methods
  and imported `strings` namespace proof for Go `strings.HasPrefix`/`HasSuffix`,
  while JS/TS untyped receivers, object wrappers, nullable receivers, borrowed or
  custom same-name calls, offsets, and direct `String.prototype` patching stay
  closed;
  `nose.protocols.sequence_hof_adapters` owns Rust iterator
  `map`/`filter`/`filter_map`/`flat_map` HOF adapter occurrence provenance plus
  `any`/`all`/`count` terminal proof, Swift Array/Collection
  `map`/`filter`/`flatMap` HOF occurrence provenance, and Ruby Enumerable
  `map`/`collect`/`select`/`filter`/`reject` HOF occurrence provenance under
  receiver and callback/block proof;
  `nose.go.stdlib.namespace_calls` owns Go `fmt.Print*`, `strings.Contains`,
  and `slices.Contains` API occurrence provenance under imported-namespace
  proof;
  `nose.protocols.iterator_identity_adapters` owns Rust
  `iter`/`into_iter`/`iter_mut`/`collect`/`to_vec`/`copied`/`cloned` and Java
  `.stream()` iterator identity adapter API occurrence provenance;
  `nose.python.stdlib.type_domain` is the first narrow stdlib pilot pack for
  Python `typing`, `collections.abc`, and `asyncio` type-domain aliases;
  `nose.value_graph.laws` is the first LawPack pilot for selected proof-backed
  value-graph law provenance.
- Local external packs require explicit user opt-in through CLI or config.
- Local manifests must declare `trust = "external-opt-in"` and
  `enabled_by_default = false`; manifests that claim builtin trust or default
  enablement are rejected.
- Duplicate pack ids fail the run instead of letting provenance become
  ambiguous.

`nose query --format json` validates configured and CLI-provided semantic-pack
paths before analysis and reports the active builtin/local pack set in the
top-level `semantic_packs` array. Unlocked local external packs remain
metadata-only while builtin compiled packs report `evidence-and-contracts`
influence. A validated v1 project lock builds a query-local, immutable Maven
dependency and Java occurrence-evidence index from content-pinned inputs and
builtin frontend facts. Near-authorized admitted occurrences may support
existing near candidates and are serialized as separate family/member
provenance. Receipt-backed external-exact collection-factory occurrences may
select only the kernel's existing collection value and exact detector path;
affected results carry separate external-claim provenance and are never
reported as builtin certification. Opaque v0 rows and unsupported v1 operations
cannot enter normalize, value-graph, or exact consumers. Builtin pack order in
this array follows the compiled registry's stable reporting order; roadmap and
snapshot prose may group packs by migration narrative instead.

## Current limits

The loader validates manifest shape and pack provenance, registers external
producer, contract, and value-law declarations as data-only rows, can report
row-id conflicts with builtin or other external rows, and can run a data-only
influence preflight report. It also validates fixed call result-domain
declarations in `semantics.result_domain` against the known domain vocabulary
and requires required `LibraryApi.Contract` evidence for those rows. The v0
preflight still blocks opaque external rows. For locked typed v1 rows, a
kernel-owned Maven reader and Java matcher produce dependency-backed occurrence
facts. The near lane consumes only admitted `collection-factory` and
`map-factory` occurrences matched to builtin protocol evidence. The exact lane
consumes only receipt-backed `collection-factory` rows with the closed
eager/pure/no-throw/non-mutating/fresh profile. A project lock supplies explicit
content/row/lane authorization and rejects semantic-coordinate conflicts before
analysis; the evidence index alone cannot influence a result. `nose
semantic-pack check --format json` exposes source observations to providers and
integrations. It does not:

- execute external evidence producers;
- register arbitrary external contract rows with exact consumers;
- register provider value-law rows or new canonical value-graph operations;
- execute fixture contents, provider commands, recognizers, parser/lowering
  plugins, producer code, or sandboxed code;
- install packs from a registry or remote source.

Future loader work should keep this boundary: external pack claims can become
usable only through dependency-backed evidence records, product source
conformance, explicit user authorization, and fail-closed kernel contracts,
never through raw selectors, arbitrary recognizer hooks, sandboxed code
execution, parser/lowering plugins, or manifest presence alone.

## See also

- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md) defines
  the manifest schema that loading consumes.
- [semantic-pack-extension-api-v1](semantic-pack-extension-api-v1.md) defines
  typed package API contracts, canonical digests, and deterministic indexes.
- [semantic-pack-conformance](semantic-pack-conformance.md) describes validation
  before manifests are trusted for reporting.
- [semantic-pack-compatibility](semantic-pack-compatibility.md) records version
  and output compatibility policy for loaded packs.
- [semantic-pack-project-lock](semantic-pack-project-lock.md) records local
  content pins, authorization, deterministic conflict handling, and rollback.
- [semantic-kernel](semantic-kernel.md) owns the exact-admission boundary that
  loaded external manifests and receipts cannot bypass.
