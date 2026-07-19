# Semantic packs 0.20 release gate

Status: #863 is closed for the 0.20 contract. The released surface is deliberately
narrow: local data-only Java/Maven collection-factory rows, explicit project
authorization, and no provider execution.

## What users can rely on

| Input | Analysis effect | Assurance and failure behavior |
| --- | --- | --- |
| no pack | existing builtin analysis only | output and runtime stay on the ordinary product path |
| v0 manifest | metadata only, permanently | strict loading may fail, but a valid manifest cannot change families |
| unlocked v1 manifest | compile/check metadata only | no row is authorized to influence analysis |
| valid v1 near lock | selected dependency-backed near rows | each affected result names its pack, row, evidence, and caveats |
| valid v1 external-exact lock | selected receipt-backed collection-factory rows | reported as `external-claim-exact`, never builtin-certified exact |
| missing, stale, incompatible, escaped, unsupported, or conflicting input | no analysis | the complete lock is rejected before source analysis |

The manifest grammar is closed and typed. It can select only kernel-owned Java
Maven import, receiver, member, arity, operation, domain, and profile mechanics.
It cannot add callbacks, regex matchers, parser behavior, value-graph nodes,
fingerprints, or canonicalization algorithms.

## Responsibility and trust

nose owns strict parsing and compilation, content and row digests, project-lock
validation, local dependency evidence, source occurrence checks, conflict
rejection, channel separation, deterministic output, conformance-runner behavior,
and result provenance. Pack providers own the truth and maintenance of their API
claims and fixtures. Users review, pin, enable, limit, update, and remove those
claims for a project.

A passing receipt means that the installed nose kernel reproduced the declared
positive and hard-negative observations for the pinned content. It is not nose
certification of a provider or a universal semantic claim.

## Update, disable, and roll back

Regenerate the receipt and then the lock after changing nose, a manifest, selected
rows or channels, fixtures, dependency evidence, or kernel capability. Review both
diffs and run `nose semantic-pack status LOCK --format json`. Hand-edited or stale
digests fail closed, as do overlapping semantic coordinates; load order never
chooses a winner.

To disable the pack, remove `--semantic-pack-lock` or the
`semantic-pack-lock` configuration entry. To narrow authority, regenerate the
lock with fewer rows or only the `near` channel. No uninstaller, registry cleanup,
provider process shutdown, or network cleanup is needed because none exists.

`base=<ref>` queries reject semantic packs. nose does not pretend that one local
dependency/receipt decision establishes compatible evidence for both revisions.

## Release evidence

The closeout matrix covers no-pack, v0 metadata, unlocked v1, locked near,
receipt-backed external exact, conflicts, stale content, incompatible versions,
`base=` rejection, deterministic replay, disablement, and rollback. The checked
example gate validates every v0/v1 manifest plus every project lock and receipt,
including lock-to-manifest and receipt-to-selected-row bindings.

The official v0.19.0 binary is the performance baseline:

- the nine-repository no-pack comparison is output-identical in semantic and near
  modes; aggregate runtime changes are -0.31% and -0.21%;
- the Vavr consumer comparison is output-identical without a pack and changes
  runtime by +0.04 ms semantic and -0.13 ms near;
- enabling the reference lock on its small evaluation corpus adds +3.13 ms
  semantic and +3.42 ms near, below the 5 ms measurement floor;
- `nose verify crates --max-violations 0` reports zero false merges and preserves
  canonical changed units.

The immutable inputs and measurements are split by purpose:

- [#868 external-exact evidence](../bench/semantic_pack/issue-868-external-exact-closeout-2026-07-19.v1.json) records the lane, no-pack comparison, and nine-repository runtime gate.
- [#869 Vavr evidence](../bench/semantic_pack/issue-869-vavr-reference-pack-closeout-2026-07-19.v1.json) records the real consumer, controlled value, enabled overhead, and rollback.
- [#870 epic closeout](../bench/semantic_pack/issue-870-epic-closeout-2026-07-19.v1.json) records the final contract and test matrix.

## Explicit non-goals

0.20 does not ship provider code or commands, remote registries, downloads,
installation or signing services, dependency resolution, parser/lowering plugins,
sandboxed pack execution, arbitrary matchers, default-enabled external packs, or
broad Vavr/library coverage. Future capabilities require new evidence and explicit
versioned contracts; this closeout does not imply them.

See [loading](semantic-pack-loading.md), [typed manifest v1](semantic-pack-extension-api-v1.md),
[project locks](semantic-pack-project-lock.md), [conformance](semantic-pack-conformance.md),
[compatibility](semantic-pack-compatibility.md), and the
[Vavr reference pack](semantic-pack-reference-vavr.md) for operational details.
