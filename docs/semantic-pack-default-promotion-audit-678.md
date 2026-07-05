# Semantic pack default-promotion audit 678

Status: issue #678 closeout audit for builtin semantic-pack lane readiness,
coverage state, documentation consistency, and rollback clarity.

The machine-readable [`bench/semantic_pack/default_promotion_audit_678.v1.json`](../bench/semantic_pack/default_promotion_audit_678.v1.json) record preserves the row-level positive fixture refs, hard-negative refs, unsupported refs, lane policy, example-check totals, and rollback gates used for this audit.

## Evidence

Commands run against commit `ada4b0862b2a377ec0630bca75ac639c21c0986f`:

```sh
target/release/nose semantic-pack inventory --format json
target/release/nose semantic-pack adoption-gates --format json
target/release/nose semantic-pack compatibility --format json
target/release/nose semantic-pack check docs/examples/semantic-packs/v0 --format json
```

Results:

| Check | Status | Key result |
| --- | --- | --- |
| inventory | `ok` | 49 builtin packs, 39 exact-capable packs, 0 needing coverage |
| adoption gates | `ok` | 49 builtin-default packs, 0 builtin-optional packs, 0 blockers |
| compatibility | `ok` | external influence remains `metadata-only`; external execution remains `none` |
| examples check | `ok` | 5 manifests, 18 fixture refs, 14 influence rows all blocked |

The audit is descriptor, documentation, and example-check work only. It does
not add rows, default-enable rows, widen exact admission, or add query hot-path
work, so product query-regression and runtime measurement are not required for
this closeout. The no-degradation decision is recorded in the artifact.

## Lane State

Current compiled builtin lane:

| Lane | Count | Decision |
| --- | ---: | --- |
| `builtin-default` | 49 | mechanically valid and enabled by default |
| `builtin-optional` | 0 | no optional-to-default candidate exists in this audit |
| `external-opt-in` | not counted here | local manifests remain metadata-only and off by default |

No follow-up issues were opened from this audit because the current inventory
has no concrete row-level coverage gaps, no blocked builtin rows, and no
optional-to-default promotion candidates. Future follow-ups should be opened
only when a changed row introduces a specific coverage gap, promotion candidate,
or rollback risk.

## Exact-Capable Rows

Every exact-capable builtin pack row is classified as `covered`. The artifact
contains the full positive, hard-negative, and unsupported reference lists for
each row.
`Refs` is positive refs plus hard-negative refs; `Unsupported` is a named subset
of hard negatives and is not counted a second time.

| Pack | Classification | Positives | Hard negatives | Refs | Unsupported |
| --- | --- | ---: | ---: | ---: | ---: |
| `nose.lang.c` | `covered` | 2 | 2 | 4 | 0 |
| `nose.python.builtins.collection_factories` | `covered` | 4 | 2 | 6 | 0 |
| `nose.python.stdlib.collection_factories` | `covered` | 3 | 2 | 5 | 0 |
| `nose.python.stdlib.math` | `covered` | 1 | 2 | 3 | 0 |
| `nose.ruby.stdlib.set` | `covered` | 3 | 3 | 6 | 0 |
| `nose.rust.stdlib.vec` | `covered` | 2 | 2 | 4 | 0 |
| `nose.rust.stdlib.option` | `covered` | 3 | 3 | 6 | 0 |
| `nose.rust.stdlib.result` | `covered` | 4 | 5 | 9 | 0 |
| `nose.rust.stdlib.integer_methods` | `covered` | 4 | 2 | 6 | 1 |
| `nose.rust.stdlib.collection_factories` | `covered` | 3 | 2 | 5 | 0 |
| `nose.rust.stdlib.map_factories` | `covered` | 2 | 2 | 4 | 0 |
| `nose.swift.stdlib.collection_factories` | `covered` | 3 | 4 | 7 | 0 |
| `nose.java.stdlib.math` | `covered` | 3 | 3 | 6 | 1 |
| `nose.java.stdlib.map_factories` | `covered` | 4 | 4 | 8 | 0 |
| `nose.java.stdlib.map_entries` | `covered` | 1 | 2 | 3 | 0 |
| `nose.java.stdlib.collection_factories` | `covered` | 7 | 5 | 12 | 0 |
| `nose.java.ecosystem.guava.immutable_collection_factories` | `covered` | 3 | 4 | 7 | 0 |
| `nose.java.stdlib.collection_constructors` | `covered` | 2 | 3 | 5 | 0 |
| `nose.java.stdlib.static_collection_adapters` | `covered` | 1 | 2 | 3 | 0 |
| `nose.protocols.map_get` | `covered` | 3 | 2 | 5 | 1 |
| `nose.protocols.map_get_default` | `covered` | 3 | 2 | 5 | 1 |
| `nose.protocols.free_function_builtins` | `covered` | 6 | 4 | 10 | 1 |
| `nose.protocols.iterator_builtins` | `covered` | 7 | 9 | 16 | 2 |
| `nose.protocols.receiver_membership` | `covered` | 10 | 3 | 13 | 1 |
| `nose.protocols.map_key_views` | `covered` | 5 | 6 | 11 | 1 |
| `nose.protocols.property_builtins` | `covered` | 4 | 3 | 7 | 1 |
| `nose.protocols.builtin_method_calls` | `covered` | 7 | 3 | 10 | 1 |
| `nose.protocols.string_affix_predicates` | `covered` | 18 | 36 | 54 | 5 |
| `nose.protocols.sequence_hof_adapters` | `covered` | 15 | 22 | 37 | 3 |
| `nose.go.stdlib.namespace_calls` | `covered` | 3 | 2 | 5 | 0 |
| `nose.protocols.iterator_identity_adapters` | `covered` | 3 | 2 | 5 | 1 |
| `nose.javascript.builtins.promise` | `covered` | 4 | 4 | 8 | 0 |
| `nose.javascript.builtins.array` | `covered` | 7 | 11 | 18 | 1 |
| `nose.javascript.builtins.boolean` | `covered` | 1 | 2 | 3 | 1 |
| `nose.javascript.builtins.regex` | `covered` | 1 | 2 | 3 | 1 |
| `nose.javascript.builtins.static_index_membership` | `covered` | 2 | 2 | 4 | 0 |
| `nose.javascript.builtins.collection_constructors` | `covered` | 2 | 3 | 5 | 0 |
| `nose.python.stdlib.type_domain` | `covered` | 36 | 2 | 38 | 0 |
| `nose.value_graph.laws` | `covered` | 2 | 4 | 6 | 0 |

## Tracked Non-Exact Rows

These builtin-default packs are tracked in inventory but have no exact-capable
descriptor rows in the current compiled inventory.

| Pack | Classification | Reason |
| --- | --- | --- |
| `nose.first_party` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.python` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.javascript-typescript` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.go` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.rust` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.java` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.ruby` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.swift` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.css` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |
| `nose.lang.html` | `tracked-no-exact-rows` | no exact-capable descriptor rows in the current compiled inventory |

## Promotion And Rollback

There are no default-promotion candidates in this audit because every compiled
builtin pack is already in `builtin-default` and no `builtin-optional` pack
exists.

The next concrete promotion candidate must attach:

- product query-regression with no unexplained family-output drift;
- runtime measurement against `main` with no unexplained degradation;
- default-surface noise review on representative repositories;
- positive fixtures and hard negatives for every changed exact-capable row;
- capabilities, query JSON, docs, and release-note updates for changed default
  behavior.

Smallest rollback actions remain, in order:

- disable only the risky row when the rest of the pack remains useful;
- demote the pack from `builtin-default` to `builtin-optional`;
- tighten admission requirements with dependency-backed evidence or hard
  negatives;
- revert the descriptor or producer change when the issue is systemic.

## Documentation Consistency

The checked terminology is consistent across capabilities, query JSON,
architecture, adoption, compatibility, conformance, usage, and the changelog:

- `builtin-default` means shipped with nose and enabled by default;
- `builtin-optional` means shipped with nose but off by default;
- `external-opt-in` means provider/user owned and explicit opt-in only;
- `compiled-builtin` is the source label for builtin descriptors;
- external local manifests remain `metadata-only`, never enabled by default,
  and execute no provider code.

The only drift found by this audit was the static builtin-inventory totals
example in [semantic-pack-conformance](semantic-pack-conformance.md), which was
updated to the current inventory counts.
