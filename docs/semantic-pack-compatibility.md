# Semantic pack compatibility

Status: compatibility policy and machine-readable report for semantic-pack v0.

This page defines how semantic packs stay compatible as the semantic kernel
changes. The current path is builtin-first: nose rehearses vocabulary and
migration changes with shipped builtin packs before any external pack can affect
analysis.

## Compatibility Dimensions

Compatibility is checked across these dimensions:

- manifest API version, currently `nose.semantic-pack.v0`;
- `compatibility.nose`, which must include the installed nose binary version;
- trust lane and default enablement;
- stable pack ids, including builtin-reserved ids;
- evidence, contract, law, dependency, fixture, and unsupported-boundary
  vocabulary;
- external influence and execution capability.

Local manifests are strict v0 JSON documents. Unknown fields, unsupported enum
values, unsupported API versions, unsupported nose version ranges, builtin trust
claims, default-enabled external packs, and duplicate or reserved pack ids fail
before analysis starts.

## Command

Use the compatibility report to inspect the installed binary's policy:

```sh
nose semantic-pack compatibility
nose semantic-pack compatibility --format json
```

The command reads only compiled policy and builtin inventory metadata. It does
not load external manifests, execute provider code, run queries, parse source
files, or enable external influence.

JSON schema version 1 reports:

- the current nose version;
- supported semantic-pack manifest API versions;
- supported compatibility report sources;
- the manifest, kernel, and migration requirements;
- fail-closed failure modes and their action;
- compiled builtin inventory status;
- external influence blockers.

Important fields:

| Field | Values |
|---|---|
| `schema_version` | `1` |
| `status` | `ok` or `blocked` |
| `current_nose_version` | Installed binary package version. |
| `supported.manifest_api_versions[]` | Supported manifest API versions, currently `nose.semantic-pack.v0`. |
| `supported.report_sources[]` | Report data sources, currently `policy`. |
| `policy.manifest_nose_version` | `must-include-installed-version` |
| `policy.manifest_schema_changes` | `breaking-change-requires-new-api-version` |
| `policy.kernel_vocabulary_changes` | `document-and-rehearse-with-builtin-packs-first` |
| `policy.capabilities_changes` | `additive-or-schema-versioned` |
| `policy.external_pack_influence` | `metadata-only` |
| `policy.external_pack_execution` | `none` |
| `policy.external_packs_enabled_by_default` | `false` |
| `requirements.manifest[]` | Static manifest compatibility requirements. |
| `requirements.kernel[]` | Static kernel admission and influence requirements. |
| `requirements.migration[]` | Static rules for schema/vocabulary migration PRs. |
| `failure_modes[].code` | Stable failure label. |
| `failure_modes[].action` | `reject-before-analysis` or `block-external-influence`. |
| `checks.builtin_inventory_status` | Current builtin inventory status. |
| `checks.external_metadata_only` | Whether local external rows remain metadata-only. |
| `checks.external_influence_blockers[]` | Stable blocker labels preventing external influence. |

## Version Policy

`api_version` selects the manifest schema family. v0 is strict: changing a
documented field's type or meaning, removing a field, adding required fields, or
changing exact-channel admission semantics requires a new API version.

`compatibility.nose` is a semver requirement for the installed binary. A local
manifest whose range does not include the current nose version is rejected before
query analysis. This is intentionally fail-closed: a provider may support older
or newer nose versions, but the current binary must not silently interpret a
manifest outside the range the provider declared.

`nose capabilities` lists supported manifest API versions and compatibility
report schema versions. Integrations should branch on these machine-readable
fields and ignore unknown additive fields.

## Kernel Vocabulary Migration

Kernel vocabulary changes must keep builtin packs ahead of external influence:

- add or migrate builtin descriptors, tests, conformance refs, and docs first;
- keep external manifests metadata-only while the new vocabulary is being proven;
- add compatibility notes for renamed fields, aliases, or enum values;
- use additive capabilities fields for additive reporting;
- require a new manifest API version for breaking manifest changes.

External packs can use the same evidence and contract vocabulary as builtin
packs, but they do not gain product influence until dependency-backed evidence,
conflict handling, executable conformance, trust gates, and compatibility gates
all exist.

## Product And Performance Invariants

Compatibility checks happen at load/check/report time. They must not add work to
normalize, detect, value-graph, fragment, or oracle loops. Metadata-only external
packs must not change clone families, ranking, witnesses, surfaces, exact/near
results, or query JSON family contents.

Any PR that changes analysis behavior must use the product behavior and
performance gates in [semantic-pack-architecture](semantic-pack-architecture.md).
Descriptor-only or reporting-only compatibility changes should report that
product output is unchanged except for additive metadata.

## Related

- [semantic-pack-architecture](semantic-pack-architecture.md)
- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md)
- [semantic-pack-conformance](semantic-pack-conformance.md)
- [semantic-pack-loading](semantic-pack-loading.md)
- [capabilities](capabilities.md)
