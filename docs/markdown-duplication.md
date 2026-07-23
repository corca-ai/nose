# Markdown duplication

`nose query` reports **same-language near-duplicate prose** across Markdown documents:
sections copied or nearly copied across files, including drifting copy-paste,
repeated boilerplate, and possible single-source-of-truth candidates. There is
no separate Markdown command.

Prose is analyzed separately from code. Markdown findings still appear in the
same dashboard so one query covers the repository.

The scope is **same-language only**. nose does not claim that translations or
paraphrases have the same meaning.

## Usage

```
nose query <path>                     # dashboard: a "markdown near-duplicates" section
nose query <path> --format json       # dashboard JSON includes a top-level "markdown" array
```

`nose query` discovers `.md`/`.markdown` under the query root or roots (respecting `.gitignore` and the same
`exclude` globs as code) and reports ranked near-duplicate **families** alongside the code clones.
The dashboard JSON uses the normal [query JSON](query-json.md) envelope and reports Markdown
findings in an additive top-level `markdown[]` array, separate from code-clone `families[]`.
Markdown findings are a dashboard domain today: list/group/family views, non-dashboard report
formats, baselines, and `--fail-on` gates continue to operate on code-clone families.
Each Markdown family carries:

- a **relation tier** (`exact` / `near-high` / `near-med` / `near-low` / `partial`) + score,
- a **span witness** — the exact duplicated line range in each file (local alignment),
- **orthogonal evidence** exposed in JSON: `commonness` (how ubiquitous the shared content is —
  high ⇒ likely boilerplate), `removable` (lines saved if single-sourced), `files`, `members`.

## How to interpret a finding

nose shows repeated text and its evidence. You decide whether it is intentional,
acceptable, or worth consolidating:

- **Boilerplate copies (license / code-of-conduct / templates) are true duplicates** — reported
  with high `commonness`, never silently suppressed.
- A `near-duplicate` label means text overlap, not “same meaning” or “you should
  remove this.”
- `commonness` helps distinguish widely repeated boilerplate from a more local
  duplication candidate; it is evidence, not a hidden verdict.

## How matching works

At a high level, nose finds likely overlapping sections, ranks the confirmed
matches, then identifies the duplicated line span in each file. The
[algorithm survey and evaluation](markdown-dup-detection-algorithm-survey-2026-06-18.md) records
the matching pipeline, corpora, and measurements.

## Related

- [clone-types](clone-types.md) — the Type-1..4 taxonomy for code; this is the prose analog,
  limited to Type-1/2/3 (no LLM ⇒ no Type-4/paraphrase).
- [languages](languages.md) — the code-clone language frontends.
