# nose query JSON (schema v2)

`nose query <path> [terms…] --format json` emits a structured, versioned contract over the
same family dataset [`scan`](scan-json.md) computes — the **machine** form of the
[exploration surface](usage.md#nose-query). Unlike scan-JSON v1 (a one-shot `families[]`
dump), the query contract is *view-shaped*: it mirrors what the human surface shows, so a
caller drives the same dashboard → slice → open-family loop programmatically.

Discover support with [`nose capabilities`](capabilities.md): `schemas.query_json` lists the
versions the installed binary emits (currently `[2]`).

## Envelope

Every response is an object with:

| field | meaning |
|---|---|
| `schema_version` | `2` |
| `tool` | `"nose"` |
| `view` | which surface produced it: `dashboard` \| `list` \| `group` \| `family` |
| `path` | the scanned path, as given |

plus the view-specific body below. Like the human surface, a result is a pure function of
(repo state, command); an unknown field or enum value is a hard error.

## Views

**`dashboard`** (no terms) — `summary` (`scanned_files`, `families`, `by_confidence`
`{exact,subdag,copy_paste,similar}`), `top_candidates[]` (curated production-first families,
each a *family object*), and `next[]` (runnable follow-up commands).

**`list`** (filters / `sort=` / `top=`) — `summary` (`families`, `shown`, `widened`),
`families[]` (the selection, each a *family object*), `next[]`.

**`group`** (`group=FIELD`) — `field` and `groups[]` of `{key, count, exemplar_id}`,
ranked by count.

**`family`** (`id=` / `at=`) — `hint` (the prose `→` recommendation) and a single `family`
object; with `full`, that object carries `skeleton`.

## The family object

| field | meaning |
|---|---|
| `id` | family id (the `id=` handle; any unique prefix opens it) |
| `scope` | `prod` \| `test` \| `mixed` (context, never a worthiness penalty) |
| `witness` | why the copies merged: `exact` \| `subdag` (behavior-proven) \| `copy-paste` \| `similar` |
| `surface` | `default` \| `review` \| `hidden` \| `shallow` \| `generated` \| `declaration` (curation tier) |
| `members` | number of copies |
| `files` / `dirs` | distinct files / directories the copies span |
| `shared` | lines invariant across **all** copies (the all-copies anti-unification count) |
| `rep_lines` | the representative copy's line count (`shared` of `rep_lines` are shared) |
| `params` | varying spots the extracted helper would parameterize |
| `removable` | `(members − 1) × shared` — lines a clean extraction would delete |
| `value` | the raw duplicated-volume score |
| `extraction_shape` | the decidable fix shape (`extract-helper`, `call-existing-helper`, …) |
| `same_symbol` | every copy is the same named symbol (the parallel-variant signal) |
| `existing_helper` | (only for `call-existing-helper`) the member to call — `{name, file, start, end}`; the inline copies recompute it, so the fix is "call it", not a fresh extraction |
| `spotclass` | (only on enriched near families) `leaf-only` (varying spots are clean value-leaves) \| `structural` (a shape/arity/referent divergence — genuine logic difference). Omitted unless the query filters/groups by `spotclass` (the graded-witness enrichment runs on demand) |
| `folds` | overlapping slice families folded under this one |
| `locations[]` | every copy: `{file, start, end, name, lang}`; the `existing_helper` member also carries `role: "existing-helper"` |
| `skeleton` | (only with `full`) the all-copies extraction-skeleton lines, `⟨param N⟩` for varying spots |

Evidence, never a verdict: there is no `worth_it`/`confidence` field — the worthy-vs-parallel
judgment is the caller's ([design §2](design.md)). See the [agent-recipe](agent-recipe.md) for
the loop, and [usage › nose query](usage.md#nose-query) for the grammar.
