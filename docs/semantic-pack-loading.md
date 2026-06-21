# Semantic pack loading

Status: nose can validate local semantic-pack v0 manifests on `nose query`, and
it can run a separate local conformance check for manifests and declared fixture
assets. External packs are explicit opt-ins and are currently `metadata-only`:
they do not emit evidence, open exact contracts, mint fingerprints, approve clone
pairs, or change exact/near query results. Local `declares.evidence_producers`,
`declares.contracts`, and `declares.value_laws` entries are registered as
data-only rows on the active `SemanticPackSet`, but no normalize, value-graph,
or exact consumer reads those external rows yet. `nose capabilities` reports the
same boundary with `external_pack_influence = "metadata-only"`,
the current external influence blocker labels, and
`external_pack_execution = "none"`.

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

## Conformance entry point

Pack authors and users can check the same local manifest paths without loading
them into an analysis run:

```sh
nose semantic-pack check semantic-packs/python-math-prod.json
nose semantic-pack check semantic-packs --format json
```

The conformance command validates manifest structure, trust policy, dependency
references, exact-capable contract obligations, conformance fixture references,
fixture expectation labels, executable fixture-expectation gates, and fixture
file existence. It does not execute external producers, provider commands, or
fixture contents, and it does not certify semantic correctness. See
[semantic-pack-conformance](semantic-pack-conformance.md).

## Trust policy

Trust is separate from channel eligibility.

- Compiled builtin packs are enabled by default and are the only packs that
  currently influence evidence and contracts. Machine output reports them with
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
  `nose.rust.stdlib.integer_methods` owns primitive integer
  `abs`/`min`/`max`/`clamp` method API occurrence provenance;
  `nose.java.stdlib.math` owns Java `Math.abs`, `Math.min`, and `Math.max`
  scalar integer API occurrence provenance;
  `nose.javascript.builtins.promise` owns JS/TS `Promise.resolve` and `.then`
  Promise API occurrence provenance;
  `nose.javascript.builtins.array` owns JS/TS `Array.from` and
  `Array.isArray` API occurrence provenance;
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
  `nose.java.stdlib.map_factories` owns Java `java.util.Map.of` and
  `java.util.Map.ofEntries` map factory API occurrence provenance;
  `nose.java.stdlib.map_entries` owns Java `java.util.Map.entry` map-entry API
  occurrence provenance;
  `nose.java.stdlib.collection_factories` owns Java `java.util.List.of`,
  `Set.of`, and `Arrays.asList` collection factory API occurrence provenance;
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
top-level `semantic_packs` array. Local external packs remain metadata-only
while builtin compiled packs report `evidence-and-contracts` influence. External
producer, contract, and value-law rows are available to the loaded pack set for
future conflict checks and adoption gates, but they are not serialized into
clone-family law provenance and do not affect analysis. Builtin pack order in
this array follows the compiled registry's stable reporting order; roadmap and
snapshot prose may group packs by migration narrative instead.

## Current limits

The loader validates manifest shape and pack provenance, registers external
producer, contract, and value-law declarations as data-only rows, can report
row-id conflicts with builtin or other external rows, and can run a data-only
influence preflight report. Today that preflight blocks all external rows until
dependency-backed evidence, explicit influence trust gates, and conflict-free
row ids exist. Exact-capable rows also remain blocked until they have passed
declarative executable conformance.
`nose semantic-pack check --format json` exposes that row-level preflight to
providers and integrations, but query, normalize, value-graph, exact, and
detection consumers do not read it. It does not yet:

- execute external evidence producers;
- register external contract rows with exact consumers;
- register external value-law rows with value-graph or exact consumers;
- execute fixture contents, provider commands, recognizers, parser/lowering
  plugins, producer code, or sandboxed code;
- compare semantic version ranges against the installed nose version beyond
  requiring a parseable declared compatibility field;
- install packs from a registry or remote source.

Future loader work should keep this boundary: external pack claims can become
usable only through dependency-backed evidence records and fail-closed kernel
contracts, never through raw selectors, arbitrary recognizer hooks, sandboxed
code execution, parser/lowering plugins, or manifest presence alone.

## See also

- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md)
- [semantic-pack-conformance](semantic-pack-conformance.md)
- [semantic-kernel](semantic-kernel.md)
