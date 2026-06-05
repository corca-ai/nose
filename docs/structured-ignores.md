# Structured ignores

Structured ignores suppress reviewed clone families without losing the decision
context. Use them when a finding is intentional, generated, framework-imposed, or
owned by a team that is not ready to refactor it yet. For command basics see
[usage](usage.md); for CI gates see [continuous-integration](continuous-integration.md).
Back to [home](home.md).

## Quick start

Run a scan and copy the family ID from the human, markdown, or JSON report:

```sh
nose scan src --format json --top 0
```

Create `nose.ignore.json` at the repository root:

```json
{
  "ignores": [
    {
      "family_id": "479389f590c1234a",
      "reason": "generated-code",
      "note": "Generated from the same template; refactor the generator instead.",
      "owner": "platform",
      "expires_at": "2026-12-31"
    }
  ]
}
```

Then run `nose scan` from that root. nose automatically reads
`nose.ignore.json` when it exists. Use `--ignore-file <file>` or
`ignore-file = "path/to/file.json"` in [configuration](configuration.md) when the
file lives elsewhere.

Ignored families are removed from the active report and do not trip `--fail-on any` or
`--fail-on new`. JSON output still carries them under `ignored_families` with the
ignore metadata, so suppressions remain auditable.

## File shape

The preferred file shape is an object with an `ignores` array:

```json
{
  "ignores": [
    {
      "paths": ["src/generated/**"],
      "languages": ["typescript"],
      "reason": "generated-code",
      "note": "Generated API bindings; source schema is the refactoring point.",
      "owner": "integrations",
      "expires_at": "2026-12-31"
    }
  ]
}
```

A top-level array of entries is also accepted for small files, but the object
shape is easier to extend in review.

Each entry must have:

| field | required | meaning |
|---|---:|---|
| `reason` | yes | Short rationale category, such as `generated-code`, `framework-required`, or `accepted-risk`. |
| `family_id` | one selector required | Stable family ID printed by nose. Best for one exact finding. |
| `paths` | one selector required | Gitignore-style path globs (positive patterns only; a leading `!` negation is rejected as an error) matched against the paths shown in the report. Best for generated directories or templates. |
| `languages` | one selector required | Language names such as `python`, `typescript`, or `rust`. Best as a broad guard combined with another selector. |
| `note` | no | Human review context. Explain where the real refactoring point is. |
| `owner` | no | Team or person responsible for revisiting the decision. |
| `expires_at` | no | `YYYY-MM-DD`. The entry applies through that date; after it, nose reports it as expired and does not apply it. |

When an entry has multiple selectors, all of them must match. For example, an
entry with both `paths` and `languages` suppresses only families that touch one
of those paths and one of those languages. If several entries match the same
family, the first active entry supplies the metadata.

## Family IDs

`family_id` is the same stable key used by baselines. It is derived from the
family members' displayed file paths and symbol names, not line numbers. That
makes it stable across ordinary line movement, but intentionally changes when a
copy is added, removed, renamed, or moved to another displayed path.

Human output includes the ID on each family:

```text
#1  id 479389f590c1234a · 3 copies · 12 of 14 lines shared, 1 spot differs · ~24 lines removable
```

JSON output includes `family_id` on both active `families[]` and
`ignored_families[]`.

## Expired and malformed entries

Malformed ignore files are hard errors. nose fails fast for invalid JSON, unknown
entry fields, missing `reason`, invalid `family_id`, invalid path globs, invalid
dates, or entries with no selector. Silent ignore mistakes would make the report
untrustworthy.

Expired entries are different: they are valid historical decisions whose date has
passed. nose prints a warning, does not apply the entry, and includes the expired
entry in the JSON `ignore.expired` list.

## Which suppression to use

| mechanism | use when | tradeoff |
|---|---|---|
| Inline `// nose-ignore` | One source unit should never participate in detection. | Removes the unit before families are formed; no family metadata exists later. |
| Structured ignore file | A reported family was reviewed and intentionally kept. | Keeps rationale, owner, expiry, and machine-readable ignored-family output. |
| Baseline | You are adopting nose on an existing codebase and want CI to flag only new or changed duplication. | Accepts the current state in bulk; use structured ignores for individual decisions that need explanation. |
