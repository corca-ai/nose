# nose query JSON (schemas v8 and v10)

`nose query <path> [terms…] --format json` emits a structured, versioned contract over the
duplicated-code family dataset — the **machine** form of the
[exploration surface](usage.md#nose-query). The query contract is *view-shaped*: it mirrors what the human surface shows, so a
caller drives the same dashboard → slice → open-family loop programmatically.
For multi-root analysis, use repeated roots:
`nose query --root <path> --root <path> [terms…] --format json`.

Discover support with [`nose capabilities`](capabilities.md): `schemas.query_json` lists the
versions the installed binary emits (currently `[8, 10]`). CI wrappers for the
divergent-edit gate should also require `query.capabilities.query_base_json_v8`,
`query.capabilities.query_base_gate_fail_default`, and, for SARIF uploads,
`query.capabilities.query_base_sarif`.

Validate a binary against the current non-`base` contract with
`python3 bench/labels/live_query_schema.py --self-test --nose <nose>`. The older
`query_schema.py` and `default_head_query_schema.py` adapters remain byte-stable inputs to
the frozen v0.19.0 quality evidence; they are not the current-product compatibility gate.

## Envelope

Every response is an object with:

| field | meaning |
|---|---|
| `schema_version` | `10` for the non-`base` query views; `8` for `base=<ref>` |
| `tool` | `"nose"` |
| `view` | which surface produced it: `dashboard` \| `list` \| `group` \| `family` \| `reinvented` \| `base` |
| `path` | the analyzed path expression, as given; multi-root commands render the repeated `--root`/`-r` flags |
| `semantic_packs` | active builtin packs plus local external packs loaded through unlocked paths or a validated project lock |

plus the view-specific body below. Like the human surface, a result is a pure function of
(repo state, command); an unknown field or enum value is a hard error.

Schema v10 adds nullable `families[].review_key` and nullable
`locations[].region` / `locations[].region_key`. These versioned SHA-256 content
signatures preserve multiplicity and exclude location/ranking. They can be shared
by distinct occurrences and never authorize review reuse by themselves. `region`
contains `{source_digest,start_byte,end_byte,content_digest}` with half-open byte
bounds into the analyzed original source. Missing or inconsistent source/proof provenance returns null; all detector
witness kinds, abstraction templates, and locked pack lanes have review projections. See [region identity](region-identity.md) for exact
inputs, limitations, snapshot commands, and correspondence policy.

Schema v9 adds explicit `generated_provenance` to generated families so callers can
distinguish caller path assertions from nose inference without silently extending the strict
v7 contract. Schema v8 adds the divergent-edit `base=<ref>` tier contract. Schema v7 adds
on-demand family-level `graded` and `graded_pair` evidence for
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
| `influence` | string | `evidence-and-contracts` for compiled builtin semantics, `metadata-only` for unlocked/v0 external packs, `near-only` for valid near authorization, or `external-claim-exact` for a receipt-backed exact authorization. |
| `path` | string or null | Canonical local manifest path for loaded manifests; `null` for compiled builtin packs. |
| `provider`, `repository`, `license` | string | Pack provenance fields. |
| `supported_languages` | array | Language ids declared by the pack. |
| `counts` | object | Counts of declared `evidence_producers`, `contracts`, `value_laws`, `positive_fixtures`, and `hard_negatives`. |
| `lock` | object, locked v1 only | Valid project-lock API/decision digest, allowed channels, selected rows, dependency pins, and optional receipt. |
| `near_influence` | object, near-authorized v1 only | `{lane,trust,selected_rows,admitted_rows,rejected_rows,admitted_occurrences,influential_occurrences,caveats[]}`. Counts distinguish authorization/evidence from occurrences that contributed to retained near families. |
| `external_exact_influence` | object, exact-authorized v1 only | Separately assured `{lane,trust,assurance,selected_rows,admitted_rows,rejected_rows,admitted_occurrences,influential_occurrences,caveats[]}` counts. `lane` is `external-claim-exact`; this is never builtin certification. |

Local external packs must not change families, ranking, witnesses, surfaces, or
exact/near results while their `influence` is `metadata-only`. `near-only` rows
may support an existing near candidate but cannot create candidates or change
fingerprints, exact results, or `nose verify`.
`external-claim-exact` requires a matching kernel receipt and exact lock. Only
the closed collection-factory kernel operation can affect fingerprints, and
every affected family/member exposes its external provenance and caveats.

For a locked v1 pack, `lock.status` is `valid`; invalid locks fail before a query
report exists. `lock.decision_digest` is independent of workspace location and
document/load order. Dependency and receipt paths are lock-root-relative, never
machine-absolute. See [semantic-pack-project-lock](semantic-pack-project-lock.md).

## Views

**`dashboard`** (no terms) — `summary` (`scanned_files`, `families`, `by_confidence`
`{exact,subdag,connected,bounded_window,copy_paste,similar}`, `reinvented` = production-surface reinvented-helper findings,
`shown` = displayed family count).
Note the copy-paste bucket key is `copy_paste` (underscore), while the per-family `witness`
enum value spells it `copy-paste` (hyphen) — so don't index `by_confidence[family.witness]`
for that one channel.
`families[]` (the top 5 code-clone families ranked by extractability — scope-blind, so test and
production are ranked alike; each a *family object*), `top_candidates[]` (compatibility alias
for the same array), `markdown[]` (Markdown near-duplicate prose families from the separate
prose engine; additive dashboard-only field), and `next[]` (runnable follow-up commands).
Headline confidence buckets count visible representatives and include `connected` separately.
Advertised witness drilldown counts use the actual filtered selection: filtering can expose
an overlapping slice whose representative is outside that selection. Witness aliases and
canonical detector kinds share one validation/matching contract.
Dashboard commands include production/test/mixed-scope routes and discovered evaluation or
fixture directory hints; JSON dashboard commands preserve `--format json`. These are explicit
filters over the existing population, not new semantic scope classifications.
`markdown[]` is not counted in `summary.families`, is not mirrored into `families[]`, and does
not participate in `--fail-on` gates.

**`list`** (filters / `sort=` / `top=`) — `summary` (`families`, `shown`, `widened`),
`families[]` (the selection, each a *family object*), `next[]`.
List `next[]` commands preserve filters, shell quoting and JSON format.
Display sorts preserve the selected IDs and overlap-fold relationships. Folding uses a
canonical extractability order independent of user/configured ordering. Same-language
families with zero source invariants receive zero extractability even when normalized
ranking lines still share structure; a structural extraction shape is not a recommendation.

**`group`** (`group=FIELD`) — `field` and `groups[]` of `{key, count, removable, exemplar_id}`,
ranked by **removable lines** (so `group=dir`/`group=file` is the duplication hotspot map).

**`family`** (`id=` / `at=`) — `hint` (the prose `→` recommendation),
`hint_reasons[]` (short human-readable facts behind that hint, when unit-origin metadata is
available), and a single `family` object; with `full`, that object carries `skeleton`.

**`reinvented`** (`reinvented`) — `summary` (`findings`, `shown`, `in_test`, `test_helper`) and `items[]` of
`{helper {name,file,start,end,in_test}, site {file,container,container_start,container_end,start,end,container_in_test},
value, approximate}` — a computation inside a container matches a helper body. This is
not proof that the helper can be called: language compatibility, visibility, imports,
dependencies and calling conventions remain review obligations. Human output distinguishes
cross-language comparison from same-language reuse investigation. The detector prefers a
same-language helper when available; cross-language relations remain visible.
`test_helper` counts production containers whose only existing helper is in test code; those are
omitted from `items[]` because production code should rehome/extract a helper before calling it.

**`base`** (`base=<git-ref>`, schema v8) — the divergent-edit view. `base` (the ref), `summary` (`changed_files`, `divergences`,
`shown_divergences`, `limit`, `fire_eligible`, `strict`), and `items[]` of `{family_id, lane,
base_family_id, similarity, complexity, scope, witness_kind, fire_eligible, tier,
tier_reasons[], taxonomy_hint, gate, suppression, graded, changed[], not_updated[],
current_only[], targets[]}` — each site carries `{file, name, start_line, end_line, lang, kind,
span_lines, span_tokens, is_fragment, fragment_kind, reason_code, enclosing_unit,
touches_shared, semantic_change, tree}`. `semantic_change` is `null` for skipped/current-only
sites and otherwise carries the bounded base-to-current evidence described below.
`divergences` is the total before `top=N` truncation; `shown_divergences` is `items.length`;
`limit` is the numeric row limit or `null` for `top=0`. `fire_eligible` is the legacy v1
conservative gate verdict. In the current implementation it means the diff provably touches
shared logic in the family and the family is not all-test scaffolding. The #852 development
qualification found no non-degenerate v3 candidate, so direct target evidence does not alter
this compatibility verdict. `strict` counts the v2 default-failing items, but it is only a
summary count; CI decisions should read each emitted item's
`gate.fail_default`, ideally from a `top=0` run.

The v8 base-view additions for divergent-edit v2 are intentionally schema-breaking
instead of silent v7 additions: CI wrappers and agents need stable enums for gate behavior.
The v7 evidence fields remain available in the base item, but `fire_eligible` stays a
compatibility verdict rather than a raw input. The raw inputs are `scope`, `witness_kind`,
`graded`, and per-site `touches_shared`.

`changed[].semantic_change` is an evidence-only v8 extension. Its `status` is `complete`,
`advisory`, or `unavailable`; `change_kind` is `no-semantic-delta`, `replacement`,
`deletion`, `insertion`, `mixed`, or `unknown`. `facets[]` identifies changed `value`,
`return`, `control`, and `effect` behavior. `coverage` records affected base/current value
nodes, how many affected base nodes occur in a skipped sibling, and how many siblings were
checked. `sink_deltas[]`, base/current projection status, alignment method, deterministic
caps, and explicit `caveats[]` make partial evidence inspectable. Pure source insertions,
mixed changes, lossy/unsupported lowering, missing or ambiguous units, heuristic alignment,
unresolved referents, and capped mappings cannot be `complete`. This field does not alter
`fire_eligible`, `tier`, or `gate.fail_default`; #849 emits and measures the evidence before
policy qualification. #852 found no admissible non-degenerate v3 consumer, so it remains
review evidence under schema v8.

`semantic_change.region_matches`, when present, adds the independently versioned
`nose.changed-region-candidates/v1` evidence contract. It reports original-byte matches
for a changed base unit in already projected current changed files. `base` and each
candidate's `source` use the [source-region contract](region-identity.md);
candidates also carry file, language, unit kind, name and line range. `status` is
`unique-content-candidate`, `ambiguous`, `partial`, or `budget-exceeded`.
`search_scope="already-projected-changed-files"`, `files_in_scope`, `files_examined`,
`max_files=64`, `max_candidates=64`, and `complete` describe the bounded search.
Completeness applies only to units in those changed files, not all repository code.
Projection failures and excluded files make the search partial. Candidate overflow
emits an empty list with `budget-exceeded`, never a truncated unique candidate.

Candidate evidence remains available when the original current unit or file is missing,
provided the base source and candidate region were projected. The semantic comparison
can remain `unavailable` while `region_matches` identifies an original-byte candidate.

The field is omitted when no eligible cross-file competition was found or base source
was unavailable; omission is not proof that a unit disappeared. A unique content
candidate is not proof of movement, ancestry, semantic equivalence or review approval.
A competing occurrence downgrades a formerly complete `changed-range` alignment to
`advisory` with `heuristic-alignment`; the selected pair's deltas remain inspectable.
An actual edit aligned by exact span or stable name retains its existing evidence.
Human output labels how many candidate locations are shown (first three), the changed-file
search scope, and file coverage. The `inspect-evidence` action opens every candidate in
JSON with `top=0`, preserving workspace, roots, explicit mode/thresholds, config, excludes,
cache and pack inputs. It does not inherit a gate flag. SARIF carries the same
structured evidence. `query_base_region_candidates_v1` advertises this optional
extension; the existing v8 enums and gate policy are unchanged.

`targets[]` is an evidence-only v8 extension for target-level policy development. A target is
`{target_id, changed, skipped, direct_witness, variant_evidence}`. `target_id` is the 16-hex-digit FNV identity of
the directed pair's repo-relative base coordinates and unit metadata; temporary base-worktree
paths and enclosing-family membership are not inputs. `direct_witness` is the pair-local
`{kind, similarity}` recorded when the detector accepted that edge, before transitive family
closure. `changed.touches_shared` is computed against only that target's `skipped` site, and
`changed.semantic_change` maps the changed behavior against only that skipped site. The
top-level `changed[]` / `not_updated[]` arrays remain the complete family review context, so a
bridge member reachable only through another clone stays visible there but is not silently
turned into a propagation target.

`variant_evidence` is pair-local and uses closed enums. `status` is `none`, `advisory`, or
`disqualifying`; `signals[]` entries contain `{code, strength, changed[], skipped[]}` and
`caveats[]` contain `{code, details[]}`. Strong signal codes are `referent-mismatch`,
`decorator-mismatch`, `async-role-mismatch`, `effect-role-mismatch`,
`protocol-role-mismatch`, and `disjoint-platform-guard`. `name-mismatch`, `path-mismatch`,
and `version-label-mismatch` are always `strength="weak"`. Caveat codes are
`source-unavailable`, `projection-unavailable`, `alignment-unavailable`,
`lossy-projection`, `unresolved-referent`, `truncated`, and
`conflicting-platform-guard`. Missing or contradictory evidence is advisory, and weak signals
cannot disqualify a target. This field does not change the v2 `tier` or `gate.fail_default`;
#852 did not qualify or activate a v3 policy that consumes it.

The #854 closeout keeps this additive evidence on schema v8 and leaves
`divergent-edit-v2-strict` active. It did not open the blind population or introduce a new
threshold/capability; integrations must continue to treat the evidence as explanatory and
`gate.fail_default` as authoritative.
The evidence compares the current changed result to the skipped base-tree endpoint; the stable
`target_id` and `direct_witness` remain anchored to the accepted base-tree edge.

JSON keeps one `items[]` finding per family, with zero or more direct targets. SARIF likewise
keeps one result per family: `properties.targets` mirrors JSON's `targets`, and every
target-backed primary/related location carries the same `target_id` in its `properties`. A base
finding with no retained direct target remains review context and falls back to family
locations; consumers must not reconstruct targets from those fallback locations. This
extension does not make target evidence a v2 gate input by itself. #852 left the v2 contract
active after the admissible v3 class had zero development support.

The v8 `base.items[]` object adds:

| field | type | meaning |
|---|---|---|
| `lane` | string enum | `base-divergence` for the base-tree propagation lane; `new-copy` for current-tree clone evidence introduced by an added/copied/renamed path in a small source diff and kept report-only. |
| `base_family_id` | string or null | The base-tree family id for `base-divergence`; `null` for `new-copy`. |
| `tier` | string enum | `strict`, `review`, `report-only`, or `suppressed`. `strict` is the only default CI-failing tier; `review` and `report-only` are visible but non-failing; `suppressed` is emitted only by an explicit future suppressed/debug surface. |
| `tier_reasons[]` | array of strings | Closed reason-code enum: `shared_logic_touched`, `shared_logic_not_touched`, `shared_logic_unproven`, `non_test_scope`, `test_scope`, `variant_signal`, `test_scaffolding`, `grouping_artifact`, `new_copy_no_base_member`, `structured_ignore`, or `unclassified`. |
| `taxonomy_hint` | string or null | Closed evidence/routing bucket for UI copy: `missed_propagation`, `no_propagation_needed`, `intentional_variant`, `test_scaffolding`, `grouping_artifact`, or `unclear`. This guides inspection; it is not a harm or correctness verdict. |
| `gate` | object | `{eligible, fail_default, policy}` where `eligible` is informational (`true` for `strict` and `review`, false for `report-only` and `suppressed`), `fail_default` is the authoritative default CI decision and is true only for unsuppressed `strict`, and `policy` names the policy version such as `divergent-edit-v2-strict`. |
| `suppression` | object or null | Structured-ignore match metadata when a future suppressed/debug view asks for it: `{kind, reason, owner, expires_at}`. `kind` is the closed enum `structured-ignore` for v8. Active human/SARIF output omits suppressed findings by default. |
| `targets[]` | array | Direct detector-accepted changed→skipped propagation edges. Each target has stable `target_id`, pair-local `direct_witness`, base-tree `changed` / `skipped` sites carrying pair-specific shared-line and semantic-change mapping, and closed `variant_evidence`. Empty for `new-copy` and when no direct edge survives. |

Composition rules for v8:

- `family_id` keeps identifying the emitted finding's family. For `base-divergence`
  it is the base-tree family id and equals `base_family_id`; for `new-copy` it is
  the current-tree family id and `base_family_id` is `null`.
- `changed[]` and `not_updated[]` remain base-tree coordinates for
  `base-divergence`; each site adds `tree: "base"` to make the coordinate origin
  explicit. `new-copy` emits `current_only[]` sites with `tree: "current"`:
  the added/copied/renamed current member plus its current-tree clone siblings.
  The `new-copy` pass is bounded to diffs touching at most two source files so
  report-only evidence does not add broad-PR runtime cost.
- `targets[]` never uses family reachability as pair evidence. A direct accepted edge must cross
  the changed/unchanged cut; edges with two changed or two unchanged endpoints do not produce a
  target. Duplicate obligations collapse deterministically to the strongest pair witness.
- `strict` requires `fire_eligible=true`, `taxonomy_hint="missed_propagation"`,
  `scope="prod"`, and no higher-priority suppression or report-only reason. Missing
  proof or mixed/test scope fails closed to `review` or `report-only`.
- CI consumers should use each item's `gate.fail_default` as the authoritative
  default pass/fail decision. `summary.strict` is a count, `gate.policy` names
  the policy that produced the decision, `gate.eligible` is informational, and
  `fire_eligible` is retained compatibility evidence rather than the v2 gate.
- `report-only` is for advisory lanes: `test_scaffolding`, `grouping_artifact`,
  `test_scope`, or `new_copy_no_base_member`.
- `suppressed` wins over all other tiers and must never set `gate.fail_default=true`.
- The current policy emits `gate.fail_default=true` only for unsuppressed production
  `strict` findings. Mixed/test, `new-copy`, `review`, `report-only`, and `suppressed`
  findings remain non-failing advisory evidence.
- Active v8 output counts only unsuppressed items: `summary.divergences` is the
  unsuppressed total before `top=N`, and `summary.shown_divergences` is
  `items.length`. A future suppressed/debug surface must expose suppressed rows
  with `tier="suppressed"`, `tier_reasons[]` containing `structured_ignore`, a
  non-null `suppression` whose `suppression.kind` is `structured-ignore`, and a
  separate `summary.suppressed_divergences` count.

## The family object

| field | meaning |
|---|---|
| `id` | family id (the `id=` handle; any unique prefix opens it) |
| `scope` | `prod` \| `test` \| `mixed` (context, never a worthiness penalty; conventional test paths such as `tests/`, `spec/`, `__tests__/`, `*_test.go`, `*.test.*`, `*.spec.*`, `conftest.py`, and Rust modular `test.rs`/`tests.rs` count as test scope, as do Rust inline `mod test`/`mod tests` spans) |
| `witness` | why the copies merged: `exact` (same unit behavior) \| `subdag` (shared computation inside each site) \| `connected` (pair-local mapped cross-unit region) \| `bounded-window` (two disjoint mapped regions in one enclosing unit; a near/refactoring witness, not exact-fragment proof) \| `copy-paste` \| `similar` |
| `surface` | `default` \| `divergence` \| `hidden` \| `shallow` \| `generated` \| `declaration` \| `debug` (curation tier; `debug` is a reserved diagnostic tier normal runs don't emit) |
| `generated_provenance` | schema v9, generated families only: `{basis,sources[]}`. `basis` is `all-members` or `compiled-css-pipeline`; sorted `sources[]` contains `caller-path`, `nose-inferred`, or both, so integrations do not confuse a caller assertion with nose-derived evidence. |
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
| `spotclass` | (only on enriched near/shared-core families whose value DAGs can be aligned) `leaf-only` (varying spots are clean value-leaves and `graded.equal_modulo_holes=true`) \| `structural` (a demoted witness, async/sync transformation, shape/arity/referent divergence, or other genuine logic difference). Cross-language families may be enriched when their value DAGs align, but remain `source_comparable: false` and do not get source-line decorator comparison. Omitted unless the query filters/groups by `spotclass` (the graded-witness enrichment runs on demand) |
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
| `semantic_pack_near` | affected near families only: deduplicated provenance rows with pack/row semantic digests, lane/trust/operation, dependency coordinate and pinned source digests, occurrence span, and caveats |
| `semantic_pack_external_exact` | affected exact families only: receipt-backed external provider-claim provenance with pack/row/receipt digests, dependency evidence, lane `external-exact`, assurance `external-claim-exact`, occurrence span, and non-certification caveats |
| `locations[]` | every copy: `{id, file, start, end, name, lang}` where `id` is the member id used by baseline diagnostics; when the frontend knows source-origin facts the location also carries `origin` (domains/body/region facets such as `type-contract`, `style`, `markup`, `declaration-only`, or `vue-sfc`); influenced members may carry `semantic_pack_near` or `semantic_pack_external_exact`; the `existing_helper` member also carries `role: "existing-helper"`; a sub-dag clone's member carries `shared_subdag: [start, end]` — where the proven shared computation lives at that site |
| `skeleton` | (only with `full`) the all-copies extraction-skeleton lines, each varying spot a `⟨param N: class⟩` placeholder (`class` = `literal`/`name`/`call`/`expr`/`block` — a coarse value-class hint for the helper signature) |

`metrics` carries the raw `RefactorFamily` features before query's view-specific display fields
such as `shared`, `rep_lines`, and `removable` are computed: `mean_sem`, `members`, `modules`,
`files`, `languages`, `mean_score`, `mean_lines`, `shared_weight`, `params`, `scope`, `value`,
`dup_lines`, and `shared_lines`.

`surface` uses the default-surface curation policy. `generated` includes families wholly
in generated/distributed output, self-declared generated HTML documents, and CSS
source-plus-compiled/minified build pipelines. Bounded generator provenance includes source
headers; complete HTML documents with a non-empty `meta[name=generator][content]`; and
source-coherent Jazzy HTML whose first 64 KiB carries both a Jazzy asset and an Apple/Dash
symbol anchor. Missing or partial provenance fails open: a default family may still contain
a generated-looking location when the hand-written or unproven copies remain actionable.

`declaration` includes both declaration runs and strict declaration-only type contracts.
The latter requires every location to be one non-fragment, whole type unit whose existing
origin facets prove type-only/declaration-only and carry no runtime, data, implementation,
default-body, extension, enum, schema, partial, or unknown evidence. Java interfaces and
annotation types, TypeScript interfaces and aliases, Rust traits, and Swift protocols use
the same language-neutral rule. Missing or contradictory evidence fails open to the
family's ranked surface. The human omission subtype `declaration-only-type-contract` maps
to JSON `surface: "declaration"`; it is not a separate JSON field. Use `all top=0`,
`surface=declaration`, or `id=` to recover the family.

Fragment proof metadata is intentionally scoped to views that need it. Dashboard/list/family
`locations[]` stay compact and do not repeat `is_fragment`, `fragment_kind`, `reason_code`,
or `enclosing_unit`; the divergent-edit `base` view serializes those fields on
`changed[]`/`not_updated[]` sites because fragment context affects the gate explanation.
For `bounded-window`, the two compact locations are already the actionable windows; they do not
carry exact-fragment metadata. Their enclosing function or method is context retained by the
detector, never substituted for either location.

Evidence, never a verdict: there is no `worth_it`/`confidence` field — the worthy-vs-parallel
judgment is the caller's ([design §2](design.md)). See the [agent-recipe](agent-recipe.md) for
the loop, and [usage › nose query](usage.md#nose-query) for the grammar.

## Family selection failures

`id=` must be nonempty and uniquely identify a family. Human, JSON, Markdown
and SARIF share the same resolver: unknown or ambiguous prefixes fail with a
nonzero exit status. JSON emits a versioned `view: "family"` envelope with
`family: null` and `error.message`; it never emits human text as successful JSON.
Use a longer prefix when an ID is ambiguous.

Dashboard `summary.skipped_sources` lists deliberately excluded source artifacts
as `{ "path": "...", "reason": "unsupported-cpp-header" }` records. Binary
source artifacts and ANSI-highlight output have their own reason codes. These
records are identical for clean, cold-cache and warm-cache queries.

## Saved analysis comparison

`query --save-analysis FILE` writes the complete admitted code-family population as
`nose.analysis/v1`; `query --before FILE --after FILE --format json` explores it through
`nose.analysis-changes/v1`. These explicitly selected schemas are separate from ordinary
query JSON v10 and base JSON v8. Dashboard/list JSON cannot be substituted for a complete
capture. The [analysis comparison contract](region-identity.md#explore-changes-between-saved-analyses)
owns their profiles, completeness, reasons, filters and executable navigation.

The comparison's `coverage.before/after` reports scanned files, skipped sources,
members lacking source evidence, diagnostic availability and the saved diagnostics.
`candidate_search_complete` reports search budget availability independently.
`summary.retained/recheck/total` count the full comparison;
`selected_retained/selected_recheck/selected` count the current selection, and `shown`
counts displayed observations. Rows use `order="recheck-first-then-observation-id"`.
`actions[]` supplies named actions alongside the compatible `next[]` command strings.
`member_changes` in detailed rows supplies before/after member counts and location-only
correspondence summaries under the statuses advertised in capabilities. Additional
fields are additive to these v1 output schemas; consumers must ignore unknown fields.
