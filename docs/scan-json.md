# Scan JSON schema

`nose scan --format json` emits a versioned machine-readable report for CI,
dashboards, editor integrations, and baselines that need clone families as data.
For the command context see [usage](usage.md); for CI-oriented formats see
[continuous-integration](continuous-integration.md). Back to [home](home.md).

## Version 1

The top-level value is always an object:

```json
{
  "schema_version": 1,
  "tool_version": "0.4.0",
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
  "families": []
}
```

A checked-in example lives at
[`crates/nose-cli/tests/fixtures/scan-json-v1.json`](../crates/nose-cli/tests/fixtures/scan-json-v1.json)
and is read by the CLI test suite.

## Top-level fields

| field | type | meaning |
|---|---|---|
| `schema_version` | integer | The JSON contract version. Version 1 is documented here. |
| `tool_version` | string | The `nose` package version that emitted the report. |
| `scope.files` | integer | Number of supported source files scanned after ignores and excludes. |
| `scope.languages` | array | Per-language file counts, largest first. |
| `ranking.sort` | string | Sort key used for `families`: `extractability`, `value`, or `sites`. |
| `ranking.total_families` | integer | Families remaining after filters and baseline suppression, before `--top`. |
| `ranking.shown_families` | integer | Families present in `families`. |
| `ranking.limit` | integer or null | The `--top` limit; `null` means `--top 0` showed every family. |
| `families` | array | Ranked clone families in display order. Empty means no family survived the filters. |

## Family fields

Each `families[]` item is one refactoring candidate. Field names are stable within
schema version 1:

| field | type | meaning |
|---|---|---|
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

## Compatibility

Consumers should branch on `schema_version` before parsing. In version 1, new
fields may be added to existing objects without changing `schema_version`, so
parsers should ignore unknown fields. Changing a documented field's type,
meaning, required presence, or path requires a new `schema_version`.
