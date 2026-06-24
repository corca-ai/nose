# nose query JSON (schema v7)

`nose query <path> [terms…] --format json` emits a structured, versioned contract over the
duplicated-code family dataset — the **machine** form of the
[exploration surface](usage.md#nose-query). The query contract is *view-shaped*: it mirrors what the human surface shows, so a
caller drives the same dashboard → slice → open-family loop programmatically.
For multi-root analysis, use repeated roots:
`nose query --root <path> --root <path> [terms…] --format json`.

Discover support with [`nose capabilities`](capabilities.md): `schemas.query_json` lists the
versions the installed binary emits (currently `[7]`).

## Envelope

Every response is an object with:

| field | meaning |
|---|---|
| `schema_version` | `7` |
| `tool` | `"nose"` |
| `view` | which surface produced it: `dashboard` \| `list` \| `group` \| `family` \| `reinvented` \| `base` |
| `path` | the analyzed path expression, as given; multi-root commands render the repeated `--root`/`-r` flags |
| `semantic_packs` | active builtin packs plus any local metadata-only packs loaded through `--semantic-pack` or `[query].semantic-packs` |

plus the view-specific body below. Like the human surface, a result is a pure function of
(repo state, command); an unknown field or enum value is a hard error.

Schema v7 adds on-demand family-level `graded` and `graded_pair` evidence for
`spotclass` enrichment. Schema v6 added the top-level `semantic_packs`
reporting field and renamed pack-facing trust/source values from legacy
first-party spelling to builtin spelling.

## Semantic packs

`semantic_packs[]` is assembled once per query response, not per family/member.
Each entry has:

| field | type | meaning |
|---|---|---|
| `id` | string | Stable pack id. |
| `hash` | string | Stable 16-hex-digit hash derived from the pack id. |
| `kind` | string | `LanguagePack`, `StdlibPack`, `LibraryPack`, `ProtocolPack`, or `LawPack`. |
| `version` | string | Pack version from the manifest or the nose package version for compiled builtin packs. |
| `display_name` | string | Human-readable pack name. |
| `trust` | string | `builtin-default`, `builtin-optional`, or `external-opt-in`. Local manifests must still use `external-opt-in`; builtin trust is reserved for packs shipped and gated with nose. |
| `enabled_by_default` | boolean | Whether the pack is default-enabled. Local manifests are rejected unless this is `false`; compiled builtin packs report `true`. |
| `source` | string | `compiled-builtin` for compiled builtin packs, or `local-manifest` for local manifest opt-ins. |
| `influence` | string | `evidence-and-contracts` for compiled builtin semantics, `metadata-only` for loaded local external packs today. |
| `path` | string or null | Canonical local manifest path for loaded manifests; `null` for compiled builtin packs. |
| `provider`, `repository`, `license` | string | Pack provenance fields. |
| `supported_languages` | array | Language ids declared by the pack. |
| `counts` | object | Counts of declared `evidence_producers`, `contracts`, `value_laws`, `positive_fixtures`, and `hard_negatives`. |

Local external packs are reported for provenance and validation only. They must
not change families, ranking, witnesses, surfaces, or exact/near results while
their `influence` is `metadata-only`.

## Views

**`dashboard`** (no terms) — `summary` (`scanned_files`, `families`, `by_confidence`
`{exact,subdag,copy_paste,similar}`, `reinvented` = production-surface reinvented-helper findings,
`shown` = displayed family count).
Note the copy-paste bucket key is `copy_paste` (underscore), while the per-family `witness`
enum value spells it `copy-paste` (hyphen) — so don't index `by_confidence[family.witness]`
for that one channel.
`families[]` (the top 5 families ranked by extractability — scope-blind, so test and
production are ranked alike; each a *family object*), `top_candidates[]` (compatibility alias
for the same array), and `next[]` (runnable follow-up commands).

**`list`** (filters / `sort=` / `top=`) — `summary` (`families`, `shown`, `widened`),
`families[]` (the selection, each a *family object*), `next[]`.

**`group`** (`group=FIELD`) — `field` and `groups[]` of `{key, count, removable, exemplar_id}`,
ranked by **removable lines** (so `group=dir`/`group=file` is the duplication hotspot map).

**`family`** (`id=` / `at=`) — `hint` (the prose `→` recommendation),
`hint_reasons[]` (short human-readable facts behind that hint, when unit-origin metadata is
available), and a single `family` object; with `full`, that object carries `skeleton`.

**`reinvented`** (`reinvented`) — `summary` (`findings`, `shown`, `in_test`, `test_helper`) and `items[]` of
`{helper {name,file,start,end,in_test}, site {file,container,container_start,container_end,start,end,container_in_test},
value, approximate}` — code that reimplements an existing helper; the action is "call it".
`test_helper` counts production containers whose only existing helper is in test code; those are
omitted from `items[]` because production code should rehome/extract a helper before calling it.

**`base`** (`base=<git-ref>`) — the divergent-edit view. `base` (the ref), `summary` (`changed_files`, `divergences`,
`shown_divergences`, `limit`, `fire_eligible`), and `items[]` of `{family_id, similarity,
complexity, scope, witness_kind, fire_eligible, graded, changed[], not_updated[]}` — each
`changed`/`not_updated` site carries `{file, name, start_line, end_line, …, touches_shared}`.
`divergences` is the total before `top=N` truncation; `shown_divergences` is `items.length`;
`limit` is the numeric row limit or `null` for `top=0`. `fire_eligible` is the conservative
proven-shared-logic verdict the gate fires on.

## The family object

| field | meaning |
|---|---|
| `id` | family id (the `id=` handle; any unique prefix opens it) |
| `scope` | `prod` \| `test` \| `mixed` (context, never a worthiness penalty) |
| `witness` | why the copies merged: `exact` (same unit behavior) \| `subdag` (shared computation inside each site) \| `copy-paste` \| `similar` |
| `surface` | `default` \| `divergence` \| `hidden` \| `shallow` \| `generated` \| `declaration` \| `debug` (curation tier; `debug` is a reserved diagnostic tier normal runs don't emit) |
| `members` | number of copies |
| `files` / `dirs` / `languages` | distinct files / directories / languages the copies span |
| `source_comparable` | `false` for cross-language families, where source lines cannot be anti-unified directly; those rows display repeated semantic volume rather than shared/removable source lines |
| `metrics` | raw detector feature object for evaluation/ranking integrations; see below |
| `shared` | lines invariant across **all** copies (the all-copies anti-unification count) |
| `rep_lines` | the representative copy's line count (`shared` of `rep_lines` are shared) |
| `params` | varying spots the extracted helper would parameterize |
| `removable` | same-language: `(members − 1) × shared`, lines a clean extraction would delete (so `removable=0` when `shared=0`: the copies match structurally but no literal line survives all of them). Cross-language: span-based repeated source volume, because there is no shared source-line basis. |
| `value` | the raw duplicated-volume score (mean span × copies × similarity × spread). Ranks by repeated *volume*, independent of `removable` — under `sort=value` a structural family can top the list with `removable=0` |
| `extraction_shape` | the decidable fix shape (`extract-helper`, `call-existing-helper`, …) |
| `same_symbol` | every copy is the same named symbol (the parallel-variant signal) |
| `existing_helper` | (only for `call-existing-helper`) the member to call — `{name, file, start, end}`; the inline copies recompute it, so the fix is "call it", not a fresh extraction |
| `spotclass` | (only on enriched same-language near/shared-core families) `leaf-only` (varying spots are clean value-leaves and `graded.equal_modulo_holes=true`) \| `structural` (a demoted witness, async/sync transformation, shape/arity/referent divergence, or other genuine logic difference). Omitted unless the query filters/groups by `spotclass` (the graded-witness enrichment runs on demand) |
| `graded` | (only when `spotclass` enrichment has run and a witness was computed) the same anti-unification object described by [graded-witness](graded-witness.md): `holes`, `spots[]`, `patterns[]` such as `async-mirror`, `referent_mismatches[]`, `caveat_names[]`, `equal_modulo_holes`, and `modeled_caveat`. This is presentation evidence for near/shared-core families, not an exact-channel proof. |
| `graded_pair` | (only with `graded`) the two `locations[]` members whose value graphs produced the grade: `{a_index,b_index,a_member_id,b_member_id}`. The indices are zero-based into this family object's `locations[]`; the ids match the corresponding location `id` fields, so consumers can tie `graded.spots[].a_text`/`b_text` back to the represented files even when a multi-member shared-core family contains decoys. |
| `value_nodes` | (exact families) the size of the shared value multiset proven identical — *how much* is proven, not just that it is |
| `status` | (only with `since=`) `new` \| `changed` \| `unchanged` against the snapshot — the temporal lens |
| `baseline_status` | (only with `--baseline`, and only for reported families) `new` \| `changed`; accepted unchanged families are hidden by `--baseline` |
| `baseline_match` | (only with `baseline_status`) `none` \| `partial-members` \| `member-locations`, explaining whether the current family matched accepted members by digest or only by exact member location |
| `matched_baseline_ids` | (only with `baseline_status`) baseline family ids that contributed accepted members or matching member locations |
| `accepted_member_count` / `new_member_count` | (only with `baseline_status`) how many current members were already accepted by source digest vs newly unaccepted |
| `folds` | count of overlapping slice families folded under this one |
| `subsumes` | (present when this family has folded slices, in any view) the `id=` handles of the slice families this one subsumes — open any to inspect |
| `subsumed_by` | (present when this family is a slice, in any view) the `id=` handle of the fuller overlapping family this one is a slice of |
| `locations[]` | every copy: `{id, file, start, end, name, lang}` where `id` is the member id used by baseline diagnostics; when the frontend knows source-origin facts the location also carries `origin` (domains/body/region facets such as `type-contract`, `style`, `markup`, `declaration-only`, or `vue-sfc`); the `existing_helper` member also carries `role: "existing-helper"`; a sub-dag clone's member carries `shared_subdag: [start, end]` — where the proven shared computation lives at that site |
| `skeleton` | (only with `full`) the all-copies extraction-skeleton lines, each varying spot a `⟨param N: class⟩` placeholder (`class` = `literal`/`name`/`call`/`expr`/`block` — a coarse value-class hint for the helper signature) |

`metrics` carries the raw `RefactorFamily` features before query's view-specific display fields
such as `shared`, `rep_lines`, and `removable` are computed: `mean_sem`, `members`, `modules`,
`files`, `languages`, `mean_score`, `mean_lines`, `shared_weight`, `params`, `scope`, `value`,
`dup_lines`, and `shared_lines`.

`surface` uses the default-surface curation policy. `generated` includes families wholly
in generated/distributed output and CSS source-plus-compiled/minified build pipelines; a
default family may still contain a generated-looking location when the hand-written copies
remain actionable.

Evidence, never a verdict: there is no `worth_it`/`confidence` field — the worthy-vs-parallel
judgment is the caller's ([design §2](design.md)). See the [agent-recipe](agent-recipe.md) for
the loop, and [usage › nose query](usage.md#nose-query) for the grammar.
