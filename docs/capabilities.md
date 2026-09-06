# Capabilities contract

`nose capabilities` emits the stable machine-readable contract for the installed
binary. Use it from installers, editor integrations, CI wrappers, and doctor
commands before invoking `nose query`. For the human command guide see
[usage](usage.md); for the result JSON see [query-json](query-json.md).

## Why this is not help text

`nose --help` is a human interface. It can change wording, examples, wrapping,
and ordering to improve readability. Tools should not scrape it.

`nose capabilities` is an integration interface. It is JSON-only, has its own
`schema_version`, and reports what the binary supports as data: stable commands,
detection modes, output formats, schema versions, config keys, and capability flags.

Integration rule: branch on `schema_version`, ignore unknown fields, and test capability
flags before passing optional query arguments. A wrapper that does this can run against older
and newer nose binaries without scraping help text or guessing from the package version.

Analysis capture/comparison support is advertised by `query_analysis_capture_v1` and
`query_analysis_changes_v1`. `schemas.analysis` lists the three versioned capture/artifact/
comparison contracts; `query.analysis` describes its commands, views, fields, enum values,
formats and limits. This is separate from baseline `since=` and region-snapshot schemas.
`query_analysis_diagnostics_v1` advertises saved skipped-source details and input-specific
coverage. `query_analysis_navigation_v1` advertises labeled `actions`, format-preserving
next commands and recheck-first presentation. `query_analysis_member_changes_v1`
advertises detailed member correspondence summaries. `query_base_evidence_navigation_v1`
advertises the base view's context-preserving full JSON evidence action.
See [analysis comparison](region-identity.md#explore-changes-between-saved-analyses).

## Example

```sh
nose capabilities
```

```json
{
  "schema_version": 8,
  "tool": {
    "name": "nose",
    "version": "<version>"
  },
  "platform": {
    "os": "linux",
    "arch": "x86_64",
    "family": "unix"
  },
  "interfaces": {
    "capabilities_json": true,
    "version_json": false,
    "doctor_json": false
  },
  "commands": {
    "stable": [
      "cache",
      "capabilities",
      "il",
      "query",
      "regions",
      "semantic-pack",
      "stats"
    ],
    "deprecated": []
  },
  "schemas": {
    "capabilities": [
      8
    ],
    "cache_status": [
      "nose.cache-status/v1"
    ],
    "cache_prune": [
      "nose.cache-prune/v1"
    ],
    "cache_clear": [
      "nose.cache-clear/v1"
    ],
    "query_json": [
      8,
      10
    ],
    "query_watch_jsonl": [
      "nose.query-watch/v1"
    ],
    "analysis": [
      "nose.analysis/v1",
      "nose.analysis-capture/v1",
      "nose.analysis-changes/v1"
    ],
    "semantic_packs": [
      "nose.semantic-pack.v0",
      "nose.semantic-pack.v1"
    ],
    "semantic_pack_locks": [
      "nose.semantic-pack-lock.v1"
    ],
    "semantic_pack_receipts": [
      "nose.semantic-pack-conformance-receipt.v1"
    ],
    "semantic_pack_lock_status": [
      1
    ],
    "semantic_pack_conformance": [
      4
    ],
    "semantic_pack_inventory": [
      1
    ],
    "semantic_pack_adoption_gates": [
      2
    ],
    "semantic_pack_compatibility": [
      2
    ]
  },
  "query": {
    "modes": [
      "syntax",
      "semantic",
      "near"
    ],
    "default_modes": [
      "syntax",
      "semantic",
      "near"
    ],
    "output_formats": [
      "human",
      "json",
      "jsonl",
      "markdown",
      "sarif"
    ],
    "sort_keys": [
      "extractability",
      "value",
      "sites",
      "hazard"
    ],
    "config_keys": [
      "cache-max-bytes",
      "exclude",
      "generated-paths",
      "ignore-file",
      "min-lines",
      "min-members",
      "min-size",
      "min-value",
      "mode",
      "semantic-packs",
      "semantic-pack-lock",
      "sort"
    ],
    "capabilities": {
      "base_divergence": true,
      "baseline": true,
      "baseline_changed_detection": true,
      "baseline_member_digest": true,
      "cache": true,
      "caller_generated_paths": true,
      "ci_fail_gate": true,
      "family_drilldown": true,
      "inline_suppression": true,
      "multi_root": true,
      "query_analysis_capture_v1": true,
      "query_analysis_changes_v1": true,
      "query_analysis_diagnostics_v1": true,
      "query_analysis_member_changes_v1": true,
      "query_analysis_navigation_v1": true,
      "query_analysis_verified_source_v1": true,
      "query_base_evidence_navigation_v1": true,
      "query_base_gate_fail_default": true,
      "query_base_json_v8": true,
      "query_base_region_candidates_v1": true,
      "query_base_sarif": true,
      "query_base_structured_ignores": true,
      "query_extraction_assessment_v1": true,
      "query_source_evidence_v1": true,
      "query_member_navigation_v1": true,
      "query_region_identity_v1": true,
      "query_review_key_v1": true,
      "query_review_records_v1": true,
      "query_scope_evidence_v1": true,
      "query_analysis_context_v1": true,
      "query_source_boundaries_v1": true,
      "query_candidate_budget_v1": true,
      "query_watch": true,
      "query_watch_full_snapshot": true,
      "query_watch_jsonl_v1": true,
      "region_correspondence_v1": true,
      "region_snapshots_v1": true,
      "reinvented_view": true,
      "semantic_pack_dependency_evidence": true,
      "semantic_pack_external_claim_exact": true,
      "semantic_pack_kernel_conformance_receipt": true,
      "semantic_pack_loading": true,
      "semantic_pack_locked_near_influence": true,
      "semantic_pack_project_lock": true,
      "structured_ignores": true
    },
    "analysis": {
      "actions": [
        "resume-selection",
        "reset-filters",
        "increase-budget",
        "recheck",
        "group-reason",
        "group-evidence",
        "expand-view",
        "return-selection",
        "inspect-source",
        "review-selection",
        "inspect-review"
      ],
      "capture": "nose query <path> --save-analysis FILE",
      "compare": "nose query --before FILE --after FILE [terms...]",
      "correspondence_values": [
        "matched",
        "candidate",
        "ambiguous",
        "unresolved",
        "unmatched-current",
        "budget-exceeded"
      ],
      "default_max_candidates": 100000,
      "evidence_values": [
        "retained",
        "recheck"
      ],
      "fields": [
        "reason",
        "correspondence",
        "evidence",
        "scope",
        "lang",
        "path",
        "witness"
      ],
      "formats": [
        "human",
        "json"
      ],
      "max_input_bytes": 134217728,
      "member_change_statuses": [
        "same-content",
        "same-content-new-location",
        "candidate",
        "ambiguous",
        "unresolved",
        "unmatched-current",
        "budget-exceeded",
        "unavailable"
      ],
      "order": "recheck-first-then-observation-id",
      "population": "admitted-query-families",
      "reason_values": [
        "profile-changed",
        "incomplete-coverage",
        "membership-changed",
        "evidence-population-changed",
        "member-content-changed",
        "source-address-changed",
        "scope-changed",
        "witness-changed",
        "analysis-changed",
        "packs-changed",
        "laws-changed",
        "abstraction-changed",
        "review-evidence-changed",
        "evidence-unavailable",
        "review-evidence-retained",
        "candidate",
        "ambiguous",
        "unresolved",
        "unmatched-current",
        "budget-exceeded"
      ],
      "review_decisions": [
        "keep-separate",
        "refactor",
        "defer"
      ],
      "review_filter": "review=applicable|recheck|unreviewed",
      "review_options": [
        "--reviews FILE",
        "--write-review FILE --decision VALUE --reason TEXT"
      ],
      "review_schema": "nose.review/v1",
      "review_statuses": [
        "applicable",
        "recheck",
        "unreviewed"
      ],
      "source_bodies": "not-stored",
      "source_limits": {
        "diff_lines_per_side": 120,
        "file_bytes": 16777216,
        "region_bytes": 65536,
        "total_bytes_per_side": 67108864
      },
      "source_options": [
        "--before-source DIR",
        "--after-source DIR"
      ],
      "source_verification": "containing-buffer-and-selected-content-sha256",
      "terms": [
        "group=FIELD",
        "change=ID",
        "FIELD=VALUE",
        "FIELD!=VALUE",
        "path~TEXT",
        "path!~TEXT",
        "top=N",
        "full",
        "all",
        "review=STATUS"
      ],
      "views": [
        "dashboard",
        "list",
        "group",
        "change"
      ],
      "witness_values": [
        "exact-value-graph",
        "shared-sub-dag",
        "copy-paste-run",
        "structural-similarity",
        "connected-mapped-sub-dag",
        "bounded-same-unit-window",
        "unavailable"
      ]
    },
    "member_navigation": {
      "default_top": 30,
      "full_source": {
        "scope": "selected-members",
        "source": "live-unverified",
        "member_limit": 8,
        "line_limit_per_member": 120
      },
      "formats": [
        "human",
        "json"
      ],
      "metrics_scope": "complete-family",
      "requires": [
        "id=ID",
        "at=FILE:LINE"
      ],
      "terms": [
        "member-id=ID",
        "member-group=dir|lang|scope",
        "member-dir=DIR",
        "member-path~TEXT",
        "member-lang=LANG",
        "member-scope=prod|test",
        "top=N",
        "full"
      ]
    }
  },
  "semantic_packs": {
    "api_versions": [
      "nose.semantic-pack.v0",
      "nose.semantic-pack.v1"
    ],
    "lock_api_versions": [
      "nose.semantic-pack-lock.v1"
    ],
    "loading": [
      "compiled-builtin",
      "local-manifest-file",
      "local-manifest-directory",
      "local-project-lock"
    ],
    "project_lock": [
      "create",
      "status"
    ],
    "project_lock_output_formats": [
      "human",
      "json"
    ],
    "conformance": [
      "local-manifest-file",
      "local-manifest-directory",
      "v0-fixture-metadata",
      "v1-kernel-source-analysis",
      "receipt-output"
    ],
    "conformance_output_formats": [
      "human",
      "json"
    ],
    "inventory": [
      "compiled-builtin"
    ],
    "inventory_output_formats": [
      "human",
      "json"
    ],
    "adoption_gates": [
      "compiled-builtin"
    ],
    "adoption_gate_output_formats": [
      "human",
      "json"
    ],
    "compatibility": [
      "policy"
    ],
    "compatibility_output_formats": [
      "human",
      "json"
    ],
    "trust": [
      "builtin-default",
      "builtin-optional",
      "external-opt-in"
    ],
    "external_packs_enabled_by_default": false,
    "external_pack_influence": "metadata-or-locked-near-or-receipt-backed-external-claim-exact",
    "external_exact_operations": [
      "collection-factory"
    ],
    "external_influence_blockers": [
      "data-only-registration",
      "dependency-backed-evidence-unavailable",
      "explicit-influence-trust-gate-missing",
      "executable-conformance-unavailable",
      "row-conflict"
    ],
    "external_pack_execution": "none"
  },
  "il": {
    "output_formats": [
      "sexpr",
      "json"
    ],
    "normalized": true,
    "cfg_norm_toggle": true
  },
  "stats": {
    "output_formats": [
      "human",
      "json"
    ]
  }
}
```

`query.modes` lists stable detection modes only. Hidden experimental modes such as
`abstraction` may be accepted by a development binary without appearing here; wrappers
should treat absence from `query.modes` as "not stable for automation."

`tool.version` is shown as the `<version>` placeholder because the field always reports the
installed binary's own version (`nose --version`); the example deliberately does not pin a
release so it can't drift.

The JSON example above is compared against `nose capabilities` by the CLI integration test;
only `tool.version` and the platform values are normalized for the local build.

## Version 8 Fields

| field | type | meaning |
|---|---|---|
| `schema_version` | integer | Capabilities contract version. Version 8 is documented here. |
| `tool.name` | string | Always `nose`. |
| `tool.version` | string | Package version of the installed binary. |
| `platform.os` | string | Rust target OS name, such as `linux`, `macos`, or `windows`. |
| `platform.arch` | string | Rust target architecture, such as `x86_64` or `aarch64`. |
| `platform.family` | string | Rust target family, such as `unix` or `windows`. |
| `interfaces.capabilities_json` | boolean | Whether `nose capabilities` is the supported capability query interface. |
| `interfaces.version_json` | boolean | Whether `nose --version --json` is supported. Version 1 reports `false`. |
| `interfaces.doctor_json` | boolean | Whether `nose doctor --json` is supported. Version 1 reports `false`. |
| `commands.stable` | array | Stable user-facing commands that integrations may invoke (incl. `query`, the interactive exploration surface — see [usage › nose query](usage.md#nose-query), with its versioned [query-JSON](query-json.md) contract). Hidden research commands are intentionally omitted. |
| `commands.deprecated` | array | Commands that still work but are being retired. Version 8 reports an empty array. |
| `schemas.capabilities` | array | Supported capabilities schema versions. |
| `schemas.cache_status` | array | Supported `nose cache status --format json` schemas. |
| `schemas.cache_prune` | array | Supported `nose cache prune --format json` schemas. |
| `schemas.cache_clear` | array | Supported `nose cache clear --format json` schemas. |
| `schemas.query_json` | array | Supported `nose query --format json` schema versions ([query-json](query-json.md)). |
| `schemas.query_watch_jsonl` | array | Supported long-lived query stream schemas ([query-watch](query-watch.md)). |
| `schemas.semantic_packs` | array | Supported semantic-pack manifest API versions, currently `nose.semantic-pack.v0` and `nose.semantic-pack.v1`. |
| `schemas.semantic_pack_locks` | array | Supported project-lock API versions, currently `nose.semantic-pack-lock.v1`. |
| `schemas.semantic_pack_receipts` | array | Supported kernel source-conformance receipt APIs, currently `nose.semantic-pack-conformance-receipt.v1`. |
| `schemas.semantic_pack_lock_status` | array | Supported `nose semantic-pack status --format json` schemas. |
| `schemas.semantic_pack_conformance` | array | Supported `nose semantic-pack check --format json` schemas. Version 4 adds bounded v1 kernel source-conformance observations and optional receipt output. |
| `schemas.semantic_pack_inventory` | array | Supported `nose semantic-pack inventory --format json` schema versions. Version 1 reports compiled builtin pack declarations, conformance refs, coverage status, and audit gaps. |
| `schemas.semantic_pack_adoption_gates` | array | Supported `nose semantic-pack adoption-gates --format json` schemas. Version 2 updates the external-influence policy for receipt-backed exact claims. |
| `schemas.semantic_pack_compatibility` | array | Supported `nose semantic-pack compatibility --format json` schemas. Version 2 reports receipt-backed external-claim exact support. |
| `query.modes` | array | Supported `--mode` values. |
| `query.default_modes` | array | Modes used by `nose query` when `--mode` is omitted. |
| `query.output_formats` | array | Supported `nose query --format` values. |
| `query.sort_keys` | array | Supported `sort=` values. |
| `query.config_keys` | array | Supported `[query]` keys in `nose.toml` / `.nose.toml`. |
| `query.capabilities` | object | Stable boolean capability flags for query workflows. |
| `semantic_packs.api_versions` | array | Supported semantic-pack manifest API versions. |
| `semantic_packs.lock_api_versions` | array | Supported content-pinned project-lock API versions. |
| `semantic_packs.loading` | array | Supported loading sources, including validated local project locks. |
| `semantic_packs.project_lock` | array | Supported local project-lock operations: `create` and `status`. |
| `semantic_packs.project_lock_output_formats` | array | Supported lock/status report formats. |
| `semantic_packs.conformance` | array | Supported conformance input sources: local manifest files/directories. |
| `semantic_packs.conformance_output_formats` | array | Supported `nose semantic-pack check --format` values. |
| `semantic_packs.inventory` | array | Supported inventory sources. Version 8 reports `compiled-builtin`. |
| `semantic_packs.inventory_output_formats` | array | Supported `nose semantic-pack inventory --format` values. |
| `semantic_packs.adoption_gates` | array | Supported adoption-gate report sources. Version 8 reports `compiled-builtin`. |
| `semantic_packs.adoption_gate_output_formats` | array | Supported `nose semantic-pack adoption-gates --format` values. |
| `semantic_packs.compatibility` | array | Supported compatibility report sources. Version 8 reports `policy`. |
| `semantic_packs.compatibility_output_formats` | array | Supported `nose semantic-pack compatibility --format` values. |
| `semantic_packs.trust` | array | Supported trust policy labels. |
| `semantic_packs.external_packs_enabled_by_default` | boolean | Always `false`; external packs require explicit CLI/config opt-in. |
| `semantic_packs.external_pack_influence` | string | Current external boundary: unlocked/v0 metadata, dependency-backed locked v1 near influence, or receipt-backed external-claim exact. |
| `semantic_packs.external_exact_operations` | array | Closed external-exact kernel operations; version 8 contains only `collection-factory`. |
| `semantic_packs.external_influence_blockers` | array | Stable blocker labels that currently prevent external rows from influencing analysis. |
| `semantic_packs.external_pack_execution` | string | Current external pack execution support. Version 8 reports `none`; nose analyzes fixture source but never executes fixture programs, recognizers, parser/lowering plugins, producer code, or sandboxed code. |
| `il.output_formats` | array | Supported `nose il --format` values. |
| `il.normalized` | boolean | Whether `nose il --normalized` is supported. |
| `il.cfg_norm_toggle` | boolean | Whether `nose il --no-cfg-norm` is supported. |
| `stats.output_formats` | array | Supported stats output formats. |

Known unsupported capabilities or query interfaces should be represented as
`false` when nose has a stable key for them. Unknown keys should be ignored by
consumers. New fields may be added to existing objects without changing
`schema_version`; changing a documented field's type or meaning requires a new
capabilities schema version.

The blocker list describes rows outside the admitted locked-near or
receipt-backed exact slices, especially opaque v0/unlocked rows. A locked typed v1 query
can build the dependency/occurrence index advertised by
`semantic_pack_dependency_evidence`; `semantic_pack_locked_near_influence`
means its admitted near rows may support existing near candidates. Exact
influence additionally requires `semantic_pack_kernel_conformance_receipt` and
is limited to the operation advertised by `external_exact_operations`.

## Query Capability Flags

Version 8 defines these `query.capabilities` keys:

| key | meaning |
|---|---|
| `base_divergence` | `base=<ref>` divergent-edit analysis is supported. |
| `baseline` | `--baseline` and `--write-baseline` are supported. |
| `baseline_changed_detection` | Baseline comparisons can classify changed and resolved families. |
| `baseline_member_digest` | Baselines use accepted member source digests, so reshaped accepted families stay hidden while edited members report as changed. |
| `cache` | `--cache-dir` file analysis caching is supported. |
| `caller_generated_paths` | Repeatable `--generated-path` and `[query].generated-paths` assertions classify all-member families without deleting them; non-base JSON distinguishes caller and nose provenance. |
| `ci_fail_gate` | `--fail-on any|new` gate behavior is supported. |
| `family_drilldown` | Opening a family with `id=<fam>` / `at=FILE:LINE` is supported. |
| `inline_suppression` | Source-level `nose-ignore` markers are supported. |
| `multi_root` | `nose query --root <path>` / `-r <path>` repeatable root analysis is supported. |
| `query_base_gate_fail_default` | `base=<ref>` emits `gate.fail_default` and uses it for the default divergent-edit CI gate. |
| `query_base_json_v8` | `base=<ref> --format json` emits schema v8. Wrappers should also verify `schemas.query_json` contains `8`. |
| `query_base_region_candidates_v1` | Optional `semantic_change.region_matches` reports bounded original-byte candidates under `nose.changed-region-candidates/v1`; it cannot establish ancestry or change the gate. |
| `query_base_sarif` | `base=<ref> --format sarif` emits divergent-edit SARIF results. Wrappers should also verify `query.output_formats` contains `sarif`. |
| `query_base_structured_ignores` | Structured ignores are applied before the `base=<ref>` divergent-edit gate. |
| `query_watch` | `nose query <root> --watch --format jsonl` is supported. |
| `query_watch_full_snapshot` | Every watch revision embeds a complete dashboard query snapshot. |
| `query_watch_jsonl_v1` | Watch records use `nose.query-watch/v1`; wrappers should also verify `schemas.query_watch_jsonl`. |
| `reinvented_view` | The `reinvented` query view is supported. |
| `semantic_pack_dependency_evidence` | A content-pinned v1 lock can build a local-only immutable Maven dependency/import/symbol/receiver/effect occurrence index; lane authorization and a consumer are still required for influence. |
| `semantic_pack_external_claim_exact` | Receipt-backed, dependency-backed, user-authorized v1 collection-factory rows may enter the existing exact kernel and report distinct external-claim provenance. |
| `semantic_pack_kernel_conformance_receipt` | `semantic-pack check --receipt-out` can bind bounded product source-analysis observations for later exact lock validation. |
| `semantic_pack_locked_near_influence` | Near-authorized dependency-backed v1 occurrences may support existing near candidates with family/member provenance. |
| `semantic_pack_loading` | local v0 manifests can be loaded as metadata and typed v1 manifests can be compiled for metadata/digest reporting. |
| `semantic_pack_project_lock` | local v1 project locks can be created, validated, and supplied to query before analysis. |
| `structured_ignores` | `nose.ignore.json` / `--ignore-file` audited suppressions are supported. |

## Region identity and correspondence

`query_region_identity_v1` and `query_review_key_v1` advertise the nullable source
and content-signature fields in query JSON v10. `region_snapshots_v1` and
`region_correspondence_v1` advertise `nose regions snapshot` and `nose regions
compare`. These capabilities do not promise historical certainty or automatic
review approval. See [region identity](region-identity.md).

The additive flags `query_analysis_verified_source_v1`, `query_review_records_v1`,
`query_extraction_assessment_v1`, `query_member_navigation_v1` and `query_scope_evidence_v1`
advertise the source/review and family-inspection surfaces. `query.analysis` publishes
source options/limits, review options, decision/status values and the `nose.review/v1`
record schema. See [region identity](region-identity.md) and [query JSON](query-json.md).

`query_source_evidence_v1` advertises bounded live-source coverage, member-bound pair diffs,
and selected-family graded enrichment on `id=ID full`; see [query JSON](query-json.md).

`query.member_navigation.full_source` documents selected-member source scope and limits
for `full`. Basic filter operators and `at=FILE:LINE` are also discoverable in
`nose query --help`; unknown numeric syntax fails before detection.

`query_analysis_context_v1` exposes the effective live analysis population in every
exploration view. `query_source_boundaries_v1` adds known member-boundary metadata, and
`query_candidate_budget_v1` advertises explicit `--max-candidate-pairs` work limits.
