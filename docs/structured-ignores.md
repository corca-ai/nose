# Structured ignores

Structured ignores suppress reviewed clone families without losing the decision
context. Use them when a finding is intentional, generated, framework-imposed, or
owned by a team that is not ready to refactor it yet. For command basics see
[usage](usage.md); for CI gates see [continuous-integration](continuous-integration.md).

## Inline marker vs structured file — which to use

nose has two ways to say "this clone is fine"; they serve different needs, so pick by
*who* the suppression is for:

| | inline `// nose-ignore` | structured `nose.ignore.json` |
|---|---|---|
| Lives | next to the code, travels with it | one file in the scan working directory |
| Carries | nothing — just "skip this site" | reason, owner, expiry, note |
| Audit | invisible in reports | listed under `ignored_families` in JSON |
| Best for | a quick, local, self-evident exception | team-level, reviewable, expiring debt |

Rule of thumb: reach for the **inline marker** when the reason is obvious to anyone
reading the line; reach for the **structured file** when the suppression is a decision
someone else should be able to find, question, and revisit later. When in doubt — or for
anything going through CI — prefer the structured file, because it stays auditable.

## Quick start

Run a scan and copy the family ID from the human, markdown, or JSON report:

```sh
nose scan src --format json --top 0
```

Create `nose.ignore.json` in the directory where you invoke nose, or pass an explicit
path with `--ignore-file`:

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

Then run `nose scan` from that directory. nose automatically reads
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
| `expires_at` | no | `YYYY-MM-DD`. The entry applies through that date; after it, nose reports it as expired and does not apply it. The date is evaluated against the current **UTC** day (deterministic across machines), so near a boundary an entry may expire up to one local day earlier or later than local midnight. |

When an entry has multiple selectors, all of them must match. For example, an
entry with both `paths` and `languages` suppresses only families that touch one
of those paths and one of those languages. If several entries match the same
family, the first active entry supplies the metadata.

## Family IDs

`family_id` is the same key used by baselines. It is derived from the sorted
reported location identities: displayed file path, language, start/end line span,
unit kind, symbol name, and fragment proof metadata. That makes IDs unique for
distinct reported families in one scan, including hidden exact fragments that
share the same file and enclosing symbol but live on nearby lines. It also means
IDs intentionally change when a copy is added, removed, renamed, moved, or when
the reported span changes.

Baseline comparison records member identities and can classify overlapping
re-keyed families as `changed`. Structured ignores that select by `family_id` are
more exact: refresh them after large code motion or after upgrading from older
nose versions whose IDs omitted span and fragment metadata. Use `paths` and
`languages` selectors when the review decision should survive routine movement
inside a file.

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
