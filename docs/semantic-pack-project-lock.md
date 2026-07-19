# Semantic-pack project locks

Status: project lock v1, local creation/status commands, pre-analysis
validation, a locked Maven evidence reader, near influence, and receipt-backed
external-claim exact are implemented. v0 can never become influential through
a lock.

## Why a separate lock exists

A manifest is a provider claim. A project lock is the user's reviewed decision
to pin particular claim content, rows, channels, and local dependency evidence.
The distinction prevents a changed file, a newly added row, or config/load order
from silently gaining authority.

The [semantic-pack lock v1 schema](schemas/semantic-pack-lock-v1.schema.json)
defines the machine-readable contract.
A [checked-in Guava lock example](examples/semantic-pack-lock-v1.json) pins the
typed v1 example and a local Maven dependency file.
The [Vavr reference lock](examples/vavr-list-project-lock-v1.json) additionally
pins an external-exact conformance receipt and demonstrates a mixed near/exact
decision.
A lock pins:

- the manifest's project-relative coordinate, API version, pack id/version,
  nose compatibility range, and canonical semantic-content digest;
- an explicit non-empty set of allowed `near` and/or `external-exact` channels;
- the exact selected row ids;
- one or more checked-in dependency manifest/lock files by relative path and
  SHA-256 content digest;
- an optional exact-conformance receipt path and digest.

All paths are resolved below the lock file's directory. Absolute paths,
`..` traversal, symlink escapes, missing files, and files outside that root are
rejected. This keeps the decision relocatable without allowing the lock to
reach arbitrary machine state.

## Create and inspect

Create a lock entirely from local files:

```sh
nose semantic-pack lock semantic-packs/guava.json \
  --dependency pom.xml \
  --channel near \
  --output nose.semantic-pack-lock.json
```

Omit `--row` to select every declared row whose requested channel is allowed.
Repeat `--row ROW_ID` for one pack. With multiple packs, qualify each selection
as `PACK_ID/ROW_ID`. Repeat `--dependency` for every checked-in input whose
change must invalidate the decision. To authorize selected `external-exact`
rows, first create a passing receipt with `semantic-pack check --receipt-out`,
then pass it with `--exact-receipt`. Exact selection without that matching
receipt fails.

```sh
nose semantic-pack check semantic-packs/example.json \
  --receipt-out semantic-packs/example.receipt.json
nose semantic-pack lock semantic-packs/example.json \
  --dependency pom.xml \
  --channel external-exact \
  --exact-receipt semantic-packs/example.receipt.json
```

Inspect or automate the decision:

```sh
nose semantic-pack status nose.semantic-pack-lock.json
nose semantic-pack status nose.semantic-pack-lock.json --format json
```

Status JSON schema v1 reports the decision digest, packs, selected rows,
channels, dependency pins, optional receipts, and zero conflicts. Both commands
are local-only: they do not fetch, install, invoke build tools, execute provider
code, or contact a registry.

## Use from a project query

Commit the lock and configure it instead of unlocked manifest paths:

```toml
[query]
semantic-pack-lock = "nose.semantic-pack-lock.json"
```

For one run:

```sh
nose query src --semantic-pack-lock nose.semantic-pack-lock.json
```

`semantic-pack-lock` is mutually exclusive with `semantic-packs` and
`--semantic-pack`. The lock owns the complete external manifest set, so mixing
unlocked paths cannot append an unreviewed pack. If the configured lock is
missing, stale, incompatible, altered, conflicting, or path-escaped, nose fails
before source analysis. Without a configured lock, ordinary v0/v1 manifest
loading remains available for metadata/check workflows and cannot influence
analysis.

Query analysis builds a query-local dependency/occurrence evidence index from
the validated lock and builtin frontend facts. It rereads no path outside the
pin set and rechecks content digests before using dependency facts. Missing,
ambiguous, invalid, or out-of-range versions keep selected rows closed rather
than consulting Maven or a registry.

Query JSON reports `influence: "near-only"` for a near-only lock and
`influence: "external-claim-exact"` when an exact receipt-backed row is
authorized. It adds a `lock` object containing `status: "valid"`, lock API and
decision digest, authorized channels, selected rows, dependency pins, and the
receipt pin. Its
`near_influence` object reports admitted/rejected rows and admitted/influential
occurrences. Removing the lock or narrowing a regenerated selection removes the
authorization, evidence, and supported near results without changing the
manifest. `external_exact_influence` reports separately assured row and
occurrence counts; affected families and members carry
`semantic_pack_external_exact` provenance. These fields describe a provider
claim tested by the kernel and authorized by the user, not builtin
certification.

## Determinism and conflicts

The decision digest is computed from a canonical projection: pack entries,
dependency pins, channels, and selected rows are sorted. JSON key order, input
file order, process state, and workspace location do not change it.

Selected rows from different packs conflict when their language, package and
overlapping version range, import/callee/member, call shape, receiver role, and
arity overlap. nose rejects the whole lock before analysis instead of choosing
the newest pack, provider, config order, or load order. Operation and profile
differences do not create precedence; an overlap is still ambiguous and fails
closed. Selecting non-overlapping rows or removing one entry restores a valid
decision.

## Update and rollback

Regenerate the receipt and then the lock after an intentional manifest,
fixture, dependency, row, channel, nose version, or kernel-capability change.
Review both diffs and run `semantic-pack status`; stale fixture content is
rehashed even though it is not a separate lock entry. Do not hand-edit digests.
To roll back, remove `semantic-pack-lock` from config or restore the previous
lock; external packs then return to unlocked metadata-only behavior.

## See also

- [typed manifest v1](semantic-pack-extension-api-v1.md) defines the closed row
  grammar and semantic digest.
- [semantic-pack loading](semantic-pack-loading.md) defines explicit local
  opt-in and execution boundaries.
- [semantic-pack compatibility](semantic-pack-compatibility.md) defines
  fail-closed version behavior.
