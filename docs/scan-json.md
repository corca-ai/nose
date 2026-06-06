# Scan JSON schema

`nose scan --format json` emits a versioned machine-readable report for CI,
dashboards, editor integrations, and baselines that need clone families as data.
For the command context see [usage](usage.md); for CI-oriented formats see
[continuous-integration](continuous-integration.md). Tools can discover supported
scan JSON schema versions with [capabilities](capabilities.md). Back to
[home](home.md).

## Version 1

The top-level value is always an object:

```json
{
  "schema_version": 1,
  "tool_version": "<version>",
  "scope": {
    "files": 4,
    "languages": [
      { "language": "python", "files": 4 }
    ]
  },
  "ranking": {
    "sort": "extractability",
    "total_families": 12,
    "shown_families": 10,
    "limit": 10
  },
  "ignore": {
    "path": "nose.ignore.json",
    "active_entries": 2,
    "expired_entries": 0,
    "ignored_families": 0,
    "expired": []
  },
  "families": []
}
```

A checked-in example lives at
[`crates/nose-cli/tests/fixtures/scan-json-v1.json`](../crates/nose-cli/tests/fixtures/scan-json-v1.json)
and is read by the CLI test suite. `tool_version` is shown above as the `<version>`
placeholder: it always reports the installed binary's own version, so the example does not
pin a release.

> **`--top` truncates machine output too.** `families` contains only the top `--top`
> families (default 30) from the active ranked set, so it is *not* the full set by
> default. `ranking.total_families` vs `ranking.shown_families` (and `ranking.limit`) make
> any truncation explicit; pass **`--top 0`** to emit every family. `ranking.total_families`
> is always the complete post-filter count regardless of `--top`.

The JSON report intentionally keeps diagnostic families that the default human,
Markdown, SARIF, and `--fail-on` surfaces omit: hidden proof-only fragments,
review-surface fragments, and families wholly inside files with generated-code
headers. Consumers that want the same first-screen surface as humans should filter
for `recommended_surface == "default"` and drop generated-header files according
to their own source metadata.

## Top-level fields

| field | type | meaning |
|---|---|---|
| `schema_version` | integer | The JSON contract version. Version 1 is documented here. |
| `tool_version` | string | The `nose` package version that emitted the report. |
| `scope.files` | integer | Number of supported source files scanned after ignores and excludes. |
| `scope.languages` | array | Per-language file counts, largest first. |
| `ranking.sort` | string | Sort key used for `families`: `extractability` (default), `value`, `sites`, or `hazard`. |
| `ranking.total_families` | integer | Active families remaining after filters, baseline suppression, and structured ignores, before `--top`. |
| `ranking.shown_families` | integer | Families present in `families`. |
| `ranking.limit` | integer or null | The `--top` limit; `null` means `--top 0` showed every family. |
| `baseline` | object, optional | Baseline comparison summary when `--baseline` is active. |
| `ignore` | object, optional | Structured ignore summary when an ignore file was read. |
| `families` | array | Active ranked clone families in JSON order, including diagnostic review/hidden families. Empty means no family survived the filters, baseline, and structured ignores. |
| `ignored_families` | array, optional | Suppressed families with the same family fields plus nested ignore metadata. Present when at least one current family was ignored. |

When `--baseline` is active, `families` contains only reportable current families:
new families and changed families. Exact baseline matches are counted in
`baseline.unchanged_families`; accepted families no longer present are counted in
`baseline.resolved_families`.

When structured ignores are active, `families` contains only active findings.
Ignored current families are omitted from `ranking.total_families` and appear in
`ignored_families` instead.

## Baseline fields

The optional `baseline` object has:

| field | type | meaning |
|---|---|---|
| `path` | string | Baseline file path used for the comparison. |
| `mode` | string | Baseline report mode; currently `new-only`. |
| `baseline_families` | integer | Accepted family keys read from the baseline. |
| `new_families` | integer | Current families with no baseline key or member overlap. |
| `changed_families` | integer | Current families whose key changed but overlap a recorded baseline member. |
| `unchanged_families` | integer | Current families whose key exactly matches the baseline. |
| `resolved_families` | integer | Baseline families that are no longer present in the current scan. |

## Ignore fields

The optional `ignore` object has:

| field | type | meaning |
|---|---|---|
| `path` | string | Ignore file path used for the scan. |
| `active_entries` | integer | Non-expired entries available for matching. |
| `expired_entries` | integer | Valid entries whose `expires_at` date has passed. |
| `ignored_families` | integer | Current families suppressed by active entries. |
| `expired` | array | Expired entry metadata: `entry`, `reason`, optional `owner`, and `expires_at`. |

Each `ignored_families[]` item has the same fields as a normal family, plus:

| field | type | meaning |
|---|---|---|
| `ignore.entry` | integer | Zero-based index in the ignore file's `ignores` array. |
| `ignore.reason` | string | Required structured rationale. |
| `ignore.note` | string, optional | Human context for the decision. |
| `ignore.owner` | string, optional | Team or person responsible for the ignore. |
| `ignore.expires_at` | string, optional | `YYYY-MM-DD` expiry date. Expired entries are not applied. |
| `ignore.selectors` | object | Original selectors from the entry: `family_id`, `paths`, and/or `languages`. |
| `ignore.matched_paths` | array, optional | Family member paths that matched `paths`. |
| `ignore.matched_languages` | array, optional | Family member languages that matched `languages`. |

The ignore file format is documented in [structured-ignores](structured-ignores.md).

## Family fields

Each `families[]` item is one refactoring candidate. Field names are stable within
schema version 1:

| field | type | meaning |
|---|---|---|
| `family_id` | string | Stable family key used by baselines and structured ignores. |
| `value` | number | Raw refactoring value: duplicated volume scaled by similarity and spread. |
| `members` | integer | Number of duplicated sites. |
| `files` | integer | Distinct files spanned by the family. |
| `modules` | integer | Distinct directories/modules spanned by the family. |
| `languages` | integer | Distinct languages spanned by the family. |
| `mean_score` | number | Mean pairwise clone similarity. |
| `mean_lines` | integer | Mean source-line span per member. |
| `dup_lines` | integer | Approximate removable duplicate lines. |
| `shared_lines` | integer | Invariant source lines between representative copies when comparable. |
| `params` | integer | Varying spots in the representative diff, used as extraction parameters. |
| `shared_weight` | number | Shared-line score weighted by how specific those lines are. |
| `locations` | array | Duplicated sites, largest first. |
| `mean_sem` | number | Mean value-graph size across members. |
| `scope` | string | `prod`, `test`, or `mixed` test/production classification. |
| `discount` | number | Refactor-worthiness discount for generated or type-heavy families. |
| `recommended_surface` | string | Product placement hint. Current detector output uses `default`, `review`, or `hidden`; `debug` is reserved for diagnostics/regression tooling. This is ranking/presentation policy, not detector exactness. |
| `baseline_status` | string, optional | `new` or `changed` when this family is shown because of `--baseline`. |

Each `locations[]` item has:

| field | type | meaning |
|---|---|---|
| `file` | string | Path relative to the current working directory when possible. |
| `start_line` | integer | 1-based start line. |
| `end_line` | integer | 1-based inclusive end line. |
| `lang` | string | Lowered source language. |
| `kind` | string | Unit kind, such as `Function`, `Method`, `Class`, or `Block`. |
| `name` | string, optional | Symbol name when the frontend can recover one. |
| `sem` | integer | Value-graph size for the site. |
| `span_lines` | integer | Inclusive source-line span for this location. |
| `span_tokens` | integer | Normalized-token span used by the detector's size gates. |
| `is_fragment` | boolean | `true` when this location is an exact sub-function fragment; `false` for whole units and syntax-channel copy-paste spans. |
| `fragment_kind` | string, optional | Exact fragment proof shape, present only when `is_fragment` is `true`; examples include `direct-return`, `conditional-guard`, and `self-field-body`. |
| `reason_code` | string, optional | Stable exact-fragment proof reason derived from `fragment_kind`, present only when `is_fragment` is `true`; examples include `exact-direct-return` and `exact-conditional-guard`. |
| `enclosing_unit` | object, optional | Exact enclosing function/method/class recovered from the same extracted unit set when available. |

### Fragment metadata

Exact semantic fragments are reported as ordinary family locations plus additive metadata.
`is_fragment` is always present; `fragment_kind`, `reason_code`, and `enclosing_unit` are
present only when the detector has exact data to serialize. A fragment with no
`enclosing_unit` is still a valid exact fragment; it only means no containing
function/method/class was recovered without guessing.

The optional `enclosing_unit` object has:

| field | type | meaning |
|---|---|---|
| `file` | string | Enclosing unit path, rewritten with the same relative-path policy as `locations[].file`. |
| `start_line` | integer | 1-based inclusive start line. |
| `end_line` | integer | 1-based inclusive end line. |
| `kind` | string | Enclosing `Function`, `Method`, or `Class`. |
| `name` | string, optional | Enclosing symbol name when recoverable. |
| `unit_key` | string | Stable key built from file, kind, span, and name for grouping/review context. |

Do not confuse fragment `reason_code` with future family/actionability reason codes from
[#11](https://github.com/corca-ai/nose/issues/11). Fragment `reason_code` answers why this
sub-function fragment was accepted as exact-safe. Future family/actionability reason codes
answer why a clone family is worth refactoring or reviewing. They intentionally do not
share one field.

`proof_facts` are not part of the stable scan JSON contract. They remain internal
diagnostic facts unless a future schema explicitly adds an unstable diagnostics namespace.

## Compatibility

Consumers should branch on `schema_version` before parsing. In version 1, new
fields may be added to existing objects without changing `schema_version`, so
parsers should ignore unknown fields. Changing a documented field's type,
meaning, required presence, or path requires a new `schema_version`.
