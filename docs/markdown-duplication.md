# Markdown duplication

`nose query` reports **same-language near-duplicate prose** across Markdown documents as one of its
domains — sections that are copied or near-copied across files (drifting copy-paste, repeated
boilerplate, single-source-of-truth candidates). Per [capabilities over features](design.md),
duplication has **one entry point** (`nose query`); markdown is surfaced there exactly as the
CSS/HTML declarative track is, not as a separate command.

It is a deliberately **separate engine** from the code-clone pipeline: prose is not code, so it does
not go through the value-graph IL. Instead it runs the character-n-gram pipeline validated by the
[algorithm survey](markdown-dup-detection-algorithm-survey-2026-06-18.md).

Scope (fixed): **same-language only.** Cross-lingual / translation duplication is out of scope (it
needs an LLM). Paraphrase / Type-4 semantic equivalence is also out of scope for the same reason.

## Usage

```
nose query <path>                     # dashboard: a "markdown near-duplicates" section
nose query <path> --format json       # a top-level "markdown" array of families
```

`nose query` discovers `.md`/`.markdown` under the path (respecting `.gitignore` and the same
`exclude` globs as code) and reports ranked near-duplicate **families** alongside the code clones,
each with:

- a **relation tier** (`exact` / `near-high` / `near-med` / `near-low` / `partial`) + score,
- a **span witness** — the exact duplicated line range in each file (local alignment),
- **orthogonal evidence** you filter on: `commonness` (how ubiquitous the shared content is —
  high ⇒ likely boilerplate), `removable` (lines saved if single-sourced), `files`, `members`.

## What it does — and deliberately does NOT do

nose **detects, witnesses, and surfaces evidence**; per the design principles it does **not**
judge whether a repetition is intentional, acceptable, or worth removing — that is judgement-deep
and the maintainer's call (see [design](design.md)). Consequences:

- **Boilerplate copies (license / code-of-conduct / templates) are true duplicates** — reported
  with high `commonness`, never silently suppressed.
- The honesty contract: output says **"near-duplicate (score + witness + commonness)"**, never
  "same meaning" and never "you should remove this".
- Precision targets the **duplication relation** (don't call unrelated or merely same-topic
  sibling sections duplicates); `commonness`/IDF is used for measurement correctness and as a
  filterable evidence field, not as a hidden verdict.

## Pipeline (three stages)

1. **Candidate generation** — character-n-gram (Latin q5 / CJK q3) MinHash-LSH + winnowing +
   containment; order-invariant (robust to block reorder), incremental, sub-quadratic. A
   stop-shingle DF cap suppresses boilerplate-driven candidate floods.
2. **Verify / rank** — IDF-weighted (TF-IDF) cosine relation score; containment rescues
   small-section-inside-large-document. IDF down-weights ubiquitous grams (topical-FP resistance).
3. **Witness** — line-level Smith-Waterman local alignment on confirmed pairs → exact duplicated
   span in each file's coordinates.

## Measurement

Quality is measured against frozen, **LLM-built golden sets** (no human in the loop;
3 heterogeneous judges, majority vote, self-calibrated on construction-truth anchors). Headline:
detector **PR-AUC 0.995** single-genre (CoC) / **0.944** multi-domain, candidate-recall 1.0,
deterministic (byte-identical across runs). See [`bench/markdown/`](../bench/markdown/README.md)
for the corpora, the deterministic build procedure, the dev `mddup` example commands, and the full
κ / PR-AUC / ROC-AUC / recall tables.

## Related

- [algorithm survey & first report](markdown-dup-detection-algorithm-survey-2026-06-18.md) — the
  19-algorithm comparison, LLM rubric evaluation, and architecture rationale.
- [clone-types](clone-types.md) — the Type-1..4 taxonomy for code; this is the prose analog,
  limited to Type-1/2/3 (no LLM ⇒ no Type-4/paraphrase).
- [languages](languages.md) — the code-clone language frontends.
