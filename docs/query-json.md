# nose query JSON (schema v3)

`nose query <path> [terms…] --format json` emits a structured, versioned contract over the
same family dataset [`scan`](scan-json.md) computes — the **machine** form of the
[exploration surface](usage.md#nose-query). Unlike scan-JSON v1 (a one-shot `families[]`
dump), the query contract is *view-shaped*: it mirrors what the human surface shows, so a
caller drives the same dashboard → slice → open-family loop programmatically.

Discover support with [`nose capabilities`](capabilities.md): `schemas.query_json` lists the
versions the installed binary emits (currently `[3]`).

## Envelope

Every response is an object with:

| field | meaning |
|---|---|
| `schema_version` | `3` |
| `tool` | `"nose"` |
| `view` | which surface produced it: `dashboard` \| `list` \| `group` \| `family` \| `reinvented` \| `base` |
| `path` | the scanned path, as given |

plus the view-specific body below. Like the human surface, a result is a pure function of
(repo state, command); an unknown field or enum value is a hard error.

## Views

**`dashboard`** (no terms) — `summary` (`scanned_files`, `families`, `by_confidence`
`{exact,subdag,copy_paste,similar}`, `reinvented` = non-test reinvented-helper findings).
Note the copy-paste bucket key is `copy_paste` (underscore), while the per-family `witness`
enum value spells it `copy-paste` (hyphen) — so don't index `by_confidence[family.witness]`
for that one channel.
`top_candidates[]` (the top 5 families ranked by extractability — scope-blind, so test and
production are ranked alike; each a *family object*), and `next[]` (runnable follow-up
commands).

**`list`** (filters / `sort=` / `top=`) — `summary` (`families`, `shown`, `widened`),
`families[]` (the selection, each a *family object*), `next[]`.

**`group`** (`group=FIELD`) — `field` and `groups[]` of `{key, count, removable, exemplar_id}`,
ranked by **removable lines** (so `group=dir`/`group=file` is the duplication hotspot map).

**`family`** (`id=` / `at=`) — `hint` (the prose `→` recommendation),
`hint_reasons[]` (short human-readable facts behind that hint, when unit-origin metadata is
available), and a single `family` object; with `full`, that object carries `skeleton`.

**`reinvented`** (`reinvented`) — `summary` (`findings`, `shown`, `in_test`) and `items[]` of
`{helper {name,file,start,end}, site {file,container,container_start,container_end,start,end},
value, approximate}` — code that reimplements an existing helper; the action is "call it".

**`base`** (`base=<git-ref>`) — the divergent-edit view (the [`nose review`](review.md)
pipeline). `base` (the ref), `summary` (`changed_files`, `divergences`, `fire_eligible`), and
`items[]` of `{family_id, similarity, complexity, scope, witness_kind, fire_eligible, graded,
changed[], not_updated[]}` — each `changed`/`not_updated` site carries `{file, name,
start_line, end_line, …, touches_shared}`. `fire_eligible` is the conservative
proven-shared-logic verdict the gate fires on. This is the same per-finding shape as the
deprecated `nose review --format json`.

## The family object

| field | meaning |
|---|---|
| `id` | family id (the `id=` handle; any unique prefix opens it) |
| `scope` | `prod` \| `test` \| `mixed` (context, never a worthiness penalty) |
| `witness` | why the copies merged: `exact` \| `subdag` (behavior-proven) \| `copy-paste` \| `similar` |
| `surface` | `default` \| `review` \| `hidden` \| `shallow` \| `generated` \| `declaration` \| `debug` (curation tier; `debug` is a reserved diagnostic tier normal runs don't emit) |
| `members` | number of copies |
| `files` / `dirs` | distinct files / directories the copies span |
| `shared` | lines invariant across **all** copies (the all-copies anti-unification count) |
| `rep_lines` | the representative copy's line count (`shared` of `rep_lines` are shared) |
| `params` | varying spots the extracted helper would parameterize |
| `removable` | `(members − 1) × shared` — lines a clean extraction would delete (so `removable=0` when `shared=0`: the copies match structurally but no literal line survives all of them) |
| `value` | the raw duplicated-volume score (mean span × copies × similarity × spread). Ranks by repeated *volume*, independent of `removable` — under `sort=value` a structural family can top the list with `removable=0` |
| `extraction_shape` | the decidable fix shape (`extract-helper`, `call-existing-helper`, …) |
| `same_symbol` | every copy is the same named symbol (the parallel-variant signal) |
| `existing_helper` | (only for `call-existing-helper`) the member to call — `{name, file, start, end}`; the inline copies recompute it, so the fix is "call it", not a fresh extraction |
| `spotclass` | (only on enriched near families) `leaf-only` (varying spots are clean value-leaves) \| `structural` (a shape/arity/referent divergence — genuine logic difference). Omitted unless the query filters/groups by `spotclass` (the graded-witness enrichment runs on demand) |
| `value_nodes` | (exact families) the size of the shared value multiset proven identical — *how much* is proven, not just that it is |
| `status` | (only with `since=`) `new` \| `changed` \| `unchanged` against the snapshot — the temporal lens |
| `folds` | count of overlapping slice families folded under this one |
| `subsumes` | (present when this family has folded slices, in any view) the `id=` handles of the slice families this one subsumes — open any to inspect |
| `subsumed_by` | (present when this family is a slice, in any view) the `id=` handle of the fuller overlapping family this one is a slice of |
| `locations[]` | every copy: `{file, start, end, name, lang}`; when the frontend knows source-origin facts the location also carries `origin` (domains/body/region facets such as `type-contract`, `style`, `markup`, `declaration-only`, or `vue-sfc`); the `existing_helper` member also carries `role: "existing-helper"`; a sub-dag clone's member carries `shared_subdag: [start, end]` — where the proven shared computation lives at that site |
| `skeleton` | (only with `full`) the all-copies extraction-skeleton lines, each varying spot a `⟨param N: class⟩` placeholder (`class` = `literal`/`name`/`call`/`expr`/`block` — a coarse value-class hint for the helper signature) |

Evidence, never a verdict: there is no `worth_it`/`confidence` field — the worthy-vs-parallel
judgment is the caller's ([design §2](design.md)). See the [agent-recipe](agent-recipe.md) for
the loop, and [usage › nose query](usage.md#nose-query) for the grammar.
