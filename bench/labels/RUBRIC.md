# Refactoring-family labeling rubric

This defines what nose's **product** is supposed to surface: *refactoring
candidates*, not "similar code". A label is attached to a **family** (a clone
group — N duplicated sites), the same unit nose reports. The label set it
produces (`refactoring_families.v1.json`) is the ground truth for the product
metric: **precision@K** (of the top-K families, how many are worth acting on) and
**worthy-recall** (of all worthy families in the pooled candidate set, how many
nose ranks high enough to be seen).

This rubric is the contract. If a judgment is hard, record `confidence: low` and a
`note` — never force a clean label onto a messy case.

## The core question

> **Would extracting one shared abstraction from these sites reduce duplication
> *without* coupling unrelated concerns or leaking per-variant quirks into the
> abstraction?**

If yes → `worthy: true`. If no → `worthy: false`. Worthiness is judged on this
question **regardless of where the code lives** — test-code duplication counts the
same as production code (duplication in tests is a real smell; do not down-weight
it). `scope` is recorded separately as context, not as a worthiness input.

## `worthy: true` — a real opportunity

Pick the `reason` that best fits:

| reason | what it is |
|---|---|
| `extract-helper` | ≥2 sites share a coherent computation/sequence → extract a function/method |
| `extract-base` | near-identical classes/structs → a shared base class, mixin, or trait |
| `extract-data-table` | repeated literal/config/mapping structures → one data-driven definition |
| `parameterize` | copies differing only in a constant, type, or operator → one parameterized version |

A family is worthy if a competent maintainer, shown it, would plausibly act —
even if they'd bikeshed *how*. Test scaffolding duplicated across many tests
(shared setup/arrange blocks) **is** worthy (`extract-helper`): a test helper is
the refactor.

**Location never excuses duplication.** Code in `examples/`, `demo/`, `template/`,
or docs directories is judged by the same core question — being "meant to be
standalone" does not auto-make duplication `parallel-by-design`. If the sites are
*near-identical extractable code*, they are worthy (a shared example fixture is the
refactor). They stay not-worthy only when the variants genuinely differ and merely
share boilerplate (e.g. demos of *different* operations sharing setup) — that is
`parallel-by-design` or `coincidental-shape` on its own merits, not because of where
they live.

## `worthy: false` — surfaced but a reviewer dismisses it

| reason | what it is | why not |
|---|---|---|
| `parallel-by-design` | per-grammar / per-platform / per-backend variants that are intentionally parallel (e.g. one lowering fn per language) | merging couples independent variants and leaks quirks; the parallelism is the design |
| `coincidental-shape` | structurally similar but semantically unrelated (different domains) | no shared abstraction exists to extract |
| `type-def` | pure data/type definitions matched on field shape, no behavior | nothing behavioral to extract |
| `generated` | generated, vendored, or third-party code | not the maintainer's to dedupe |
| `trivial` | real but too small / language-mandated boilerplate (getters, ctors, simple delegations) | extracting costs more than it saves |

## Worked examples (calibration)

These are the recurring judgment calls; apply them consistently.

| family | label | reason | why |
|---|---|---|---|
| `httpx` `get`/`post`/`put`/… each delegating to `request("GET", …)` | worthy | `parameterize` | copies differing only in a constant (the verb string) → one parameterized helper |
| `commons-lang` `MutableInt`/`MutableLong`/… (per-primitive classes) | worthy | `extract-base` | shared structure across primitives → a base/generic; the primitive is the only axis |
| `zod` `locales/fr.ts`/`de.ts`/… (per-language message maps) | not | `parallel-by-design` | same structure, but the translated strings ARE the per-variant payload — merging is pointless |
| `rich` `_unicode_data/unicode13.py`/`unicode14.py` (version tables) | not | `generated` | generated data tables; not the maintainer's to dedupe |
| two unrelated `match`/`switch` tables (e.g. token→op vs ext→lang) | not | `coincidental-shape` | identical shape, unrelated domains → no shared abstraction |
| identical mock handler copied across 4 `examples/*/api/data.ts` | worthy | `extract-helper` | near-identical extractable code; location (examples/) does not excuse it |
| curl `docs/examples/imap-*.c` `main()` (each a *different* IMAP command) | not | `parallel-by-design` | only the curl-setup preamble is shared; the bodies genuinely differ per demo |
| a 3-line getter duplicated across DTOs | not | `trivial` | extracting costs more than it saves |
| a `Class` of only field declarations, no methods, matched across modules | not | `type-def` | no behavior to extract |

When a family mixes signals (e.g. a real helper duplicated alongside generated
copies), label by what *dominates* the members and record `confidence: low` + a note.

## Fields

One JSON object per family (schema in `schema.json`):

- `family_id` — stable id: FNV-1a over the sorted member `(file, start_line, end_line)` tuples.
- `repo`, `members` (`[{file, start_line, end_line, name}]`).
- `channel` — `structural` | `contiguous` | `both` (which detector(s) surfaced it; for pool-source analysis).
- `worthy` — bool.
- `reason` — one of the categories above.
- `scope` — `prod` | `test` | `mixed` (context only; not a worthiness input).
- `confidence` — `high` | `medium` | `low`.
- `note` — free text; required when `confidence != high` or `reason` is borderline.
- `labeler` — `draft` (Claude initial), `panel` (LLM panel consensus), or `human` (user-corrected; authoritative).

## Process & anti-bias rules

1. **Pool from multiple sources.** Candidates are the *union* of nose
   (low-threshold, both channels) and jscpd-weak, so families nose misses are
   still in the pool — otherwise worthy-recall can't be measured. Record `channel`.
2. **Draft → panel → human.** Claude drafts with rationale; a 3-persona LLM panel
   (re-using §H) labels independently; disagreements (panel split, or panel vs
   draft) are escalated to the user, whose call is authoritative (`labeler: human`).
3. **Freeze the split.** Honor the corpus `dev`/`heldout` split. Tune only against
   `dev`; check `heldout` rarely. Never tune against a label you then report on.
4. **No metric gaming.** Don't relabel to make nose look better. A worthy family
   nose ranks low is a recall miss we *want* recorded.
