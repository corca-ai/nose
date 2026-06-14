# Scan JSON schema

`nose scan --format json` emits a versioned machine-readable report for CI,
dashboards, editor integrations, and baselines that need clone families as data.
For the command context see [usage](usage.md); for CI-oriented formats see
[continuous-integration](continuous-integration.md). Tools can discover supported
scan JSON schema versions with [capabilities](capabilities.md). An LLM agent
consuming this schema should follow the validated triage protocol in
[agent-recipe](agent-recipe.md). This is the one-shot batch contract; for *interactive*
exploration of the same dataset, [`nose query`](usage.md#nose-query) is its companion
surface (also `--format json`).

## Version 1

The top-level value is always an object:

```json
{
  "schema_version": 1,
  "tool_version": "<version>",
  "scope": {
    "files": 4,
    "languages": [
      { "language": "python", "files": 4 }
    ]
  },
  "semantic_packs": [
    {
      "id": "nose.first_party",
      "hash": "87b19e582546aed9",
      "kind": "LanguagePack",
      "version": "<version>",
      "display_name": "nose first-party semantic kernel",
      "trust": "default-first-party",
      "enabled_by_default": true,
      "source": "compiled-first-party",
      "influence": "evidence-and-contracts",
      "provider": "Corca, Inc.",
      "repository": "https://github.com/corca-ai/nose",
      "license": "MIT",
      "supported_languages": [],
      "counts": {
        "evidence_producers": 0,
        "contracts": 0,
        "value_laws": 0,
        "positive_fixtures": 0,
        "hard_negatives": 0
      }
    },
    {
      "id": "nose.python.stdlib.type_domain",
      "hash": "783a582a461f58f3",
      "kind": "StdlibPack",
      "version": "<version>",
      "display_name": "nose Python stdlib type-domain pack",
      "trust": "default-first-party",
      "enabled_by_default": true,
      "source": "compiled-first-party",
      "influence": "evidence-and-contracts",
      "provider": "Corca, Inc.",
      "repository": "https://github.com/corca-ai/nose",
      "license": "MIT",
      "supported_languages": [
        "python"
      ],
      "counts": {
        "evidence_producers": 1,
        "contracts": 1,
        "value_laws": 0,
        "positive_fixtures": 36,
        "hard_negatives": 2
      }
    }
  ],
  "ranking": {
    "sort": "extractability",
    "total_families": 12,
    "shown_families": 10,
    "limit": 10,
    "surface_counts": {
      "default": 4,
      "review": 1,
      "hidden": 7,
      "debug": 0,
      "generated": 0,
      "declaration": 0,
      "shallow": 0,
      "fragments": {
        "total": 8,
        "default": 1,
        "review": 1,
        "hidden": 6,
        "debug": 0
      }
    }
  },
  "ignore": {
    "path": "nose.ignore.json",
    "active_entries": 2,
    "expired_entries": 0,
    "ignored_families": 0,
    "expired": []
  },
  "families": []
}
```

A checked-in example lives at
[crates/nose-cli/tests/fixtures/scan-json-v1.json](../crates/nose-cli/tests/fixtures/scan-json-v1.json)
and is read by the CLI test suite. `tool_version` is shown above as the `<version>`
placeholder: it always reports the installed binary's own version, so the example does not
pin a release.

> **`--top` truncates machine output too.** `families` contains only the top `--top`
> families (default 30) from the active ranked set, so it is *not* the full set by
> default. `ranking.total_families` vs `ranking.shown_families` (and `ranking.limit`) make
> any truncation explicit; pass **`--top 0`** to emit every family. `ranking.total_families`
> is always the complete post-filter count regardless of `--top`.

The JSON report intentionally keeps diagnostic families that survive ranking but are
omitted from the default human, Markdown, SARIF, and `--fail-on` surfaces: hidden
proof-only fragments, review-surface fragments, and families wholly inside files with
generated-code headers. It is not raw detector output: families wholly in
vendored/generated-looking paths may already have been pruned before serialization.

**Consumer rule:** integrations that want the same first-screen human action surface should
filter `families[]` to `recommended_surface == "default"` and drop generated-header files
according to their own source metadata. Treat `review` and `hidden` as diagnostic surfaces:
useful for audits, review-hazard tooling, and regression checks, but not default
refactoring recommendations.

`ranking.surface_counts` gives the active post-filter family counts by
`recommended_surface` before `--top` truncation. The nested `fragments` object repeats the
same breakdown for families with at least one exact fragment location. That makes a
`--top 0` diagnostic run easy to summarize without scanning every location first.

## Top-level fields

| field | type | meaning |
|---|---|---|
| `schema_version` | integer | The JSON contract version. Version 1 is documented here. |
| `tool_version` | string | The `nose` package version that emitted the report. |
| `scope.files` | integer | Number of supported source files scanned after ignores and excludes. |
| `scope.languages` | array | Per-language file counts, largest first. |
| `semantic_packs` | array, optional in v1 | Active semantic packs for this scan. Binaries that advertise `scan.capabilities.semantic_pack_loading` in [capabilities](capabilities.md) emit it and include compiled first-party packs such as `nose.first_party`, `nose.python.stdlib.type_domain`, and `nose.value_graph.laws`; local `--semantic-pack`/config packs are listed with `metadata-only` influence. Older v1 binaries omit this field. |
| `ranking.sort` | string | Sort key used for `families`: `extractability` (default), `value`, `sites`, or `hazard`. |
| `ranking.total_families` | integer | Active families remaining after rank-time pruning, filters, baseline suppression, and structured ignores, before `--top`. |
| `ranking.shown_families` | integer | Families present in `families`. |
| `ranking.limit` | integer or null | The `--top` limit; `null` means `--top 0` showed every family. |
| `ranking.surface_counts` | object | Active family counts by effective surface before `--top`: `default`, `review`, `hidden`, `debug`, `generated` (families wholly in generated-header source), `declaration` (families whose every member span is only import/include/use/re-export declarations), and `shallow` (unproven families whose extracted helper would be mostly parameters — `params` ≥ a third of `shared_lines`) — the human report omits `generated`, `declaration`, and `shallow` from default output. Plus `fragments.total/default/review/hidden/debug` for families with exact fragment locations. |
| `baseline` | object, optional | Baseline comparison summary when `--baseline` is active. |
| `ignore` | object, optional | Structured ignore summary when an ignore file was read. |
| `families` | array | Active ranked clone families in JSON order, including diagnostic review/hidden families. Empty means no family survived the filters, baseline, and structured ignores. |
| `ignored_families` | array, optional | Suppressed families with the same family fields plus nested ignore metadata. Present when at least one current family was ignored. |

When `--baseline` is active, `families` contains only reportable current families:
new families and changed families. Exact baseline matches are counted in
`baseline.unchanged_families`; accepted families no longer present are counted in
`baseline.resolved_families`.

When structured ignores are active, `families` contains only active findings.
Ignored current families are omitted from `ranking.total_families` and appear in
`ignored_families` instead.

## Semantic pack fields

When `semantic_packs` is present, each entry has:

| field | type | meaning |
|---|---|---|
| `id` | string | Stable manifest pack id. |
| `hash` | string | Stable 16-hex-digit hash derived from the pack id; first-party evidence provenance uses the same id-hash policy. |
| `kind` | string | `LanguagePack`, `StdlibPack`, `LibraryPack`, `ProtocolPack`, or `LawPack`. |
| `version` | string | Pack version from the manifest or the nose package version for compiled first-party packs. |
| `display_name` | string | Human-readable pack name. |
| `trust` | string | `default-first-party`, `first-party-optional`, or `external-opt-in`. Local manifests are rejected unless they use `external-opt-in`; first-party trust comes only from compiled packs. |
| `enabled_by_default` | boolean | Whether the pack is default-enabled. Local manifests are rejected unless this is `false`; compiled first-party packs report `true`. |
| `source` | string | `compiled-first-party` or `local-manifest`. |
| `influence` | string | `evidence-and-contracts` for compiled first-party semantics, `metadata-only` for loaded local external packs today. |
| `path` | string, optional | Local manifest path for loaded manifests. |
| `provider`, `repository`, `license` | string | Manifest provenance fields. |
| `supported_languages` | array | Manifest language ids. |
| `counts` | object | Counts of declared evidence producers, contracts, value laws, positive fixtures, and hard negatives. |

## Baseline fields

The optional `baseline` object has:

| field | type | meaning |
|---|---|---|
| `path` | string | Baseline file path used for the comparison. |
| `mode` | string | Baseline report mode; currently `new-only`. |
| `baseline_families` | integer | Accepted family keys read from the baseline. |
| `new_families` | integer | Current families with no baseline key or member overlap. |
| `changed_families` | integer | Current families whose key changed but overlap a recorded baseline member. |
| `unchanged_families` | integer | Current families whose key exactly matches the baseline. |
| `resolved_families` | integer | Baseline families that are no longer present in the current scan. |

## Ignore fields

The optional `ignore` object has:

| field | type | meaning |
|---|---|---|
| `path` | string | Ignore file path used for the scan. |
| `active_entries` | integer | Non-expired entries available for matching. |
| `expired_entries` | integer | Valid entries whose `expires_at` date has passed. |
| `ignored_families` | integer | Current families suppressed by active entries. |
| `expired` | array | Expired entry metadata: `entry`, `reason`, optional `owner`, and `expires_at`. |

Each `ignored_families[]` item has the same fields as a normal family, plus:

| field | type | meaning |
|---|---|---|
| `ignore.entry` | integer | Zero-based index in the ignore file's `ignores` array. |
| `ignore.reason` | string | Required structured rationale. |
| `ignore.note` | string, optional | Human context for the decision. |
| `ignore.owner` | string, optional | Team or person responsible for the ignore. |
| `ignore.expires_at` | string, optional | `YYYY-MM-DD` expiry date. Expired entries are not applied. |
| `ignore.selectors` | object | Original selectors from the entry: `family_id`, `paths`, and/or `languages`. |
| `ignore.matched_paths` | array, optional | Family member paths that matched `paths`. |
| `ignore.matched_languages` | array, optional | Family member languages that matched `languages`. |

The ignore file format is documented in [structured-ignores](structured-ignores.md).

## Family fields

Each `families[]` item is one refactoring candidate. Field names are stable within
schema version 1:

| field | type | meaning |
|---|---|---|
| `family_id` | string | Stable family key used by baselines and structured ignores. Unique for distinct reported families in one scan; derived from each member's displayed path, language, source span, unit kind, symbol name, and fragment proof metadata. The ID is stable across run order and thread count, but changes when the reported locations or spans change. |
| `value` | number | Raw refactoring value: duplicated volume scaled by similarity and spread. |
| `members` | integer | Number of duplicated sites. |
| `files` | integer | Distinct files spanned by the family. |
| `modules` | integer | Distinct directories spanned by the family (design-level spread). The field name is historical; nose's spatial unit is the directory, and human output says "directories". |
| `languages` | integer | Distinct languages spanned by the family. |
| `mean_score` | number | Mean pairwise clone similarity. |
| `mean_lines` | integer | Mean source-line span per member. |
| `dup_lines` | integer | Approximate removable duplicate lines. |
| `shared_lines` | integer | Lines invariant across **all** copies (up to an 8-member cap), the all-copies anti-unification count — the same shared/removable headline `nose query` reports for the family (#366). Earlier releases counted the representative pair, which over-stated families whose 3rd+ copies diverge. |
| `params` | integer | Varying spots in the **representative** diff, used as extraction parameters; stays representative-pair (so it agrees with `varying_spots`) — the all-copies hole count was gold-set-measured into the ranking and regressed held-out, so only `shared_lines` moved to the all-copies basis. |
| `shared_weight` | number | Shared-line score weighted by how specific those lines are. |
| `locations` | array | Duplicated sites, largest first. |
| `mean_sem` | number | Mean value-graph size across members. |
| `scope` | string | `prod`, `test`, or `mixed` test/production classification. |
| `discount` | number | Refactor-worthiness discount for generated or type-heavy families. |
| `recommended_surface` | string | Product placement hint. Current detector output uses `default`, `review`, or `hidden`; `debug` is reserved for diagnostics/regression tooling; `generated` marks families whose every location sits in a generated-header source; `declaration` marks families whose every member span consists only of import/include/use/re-export declarations — real duplication the language mandates per file, with no extraction action; `shallow` marks an unproven family (not the `exact-value-graph`/`shared-sub-dag` channel) whose extracted helper would be mostly parameters — `params` ≥ a third of `shared_lines`, the decidable `shallow-extraction` non-action class ([default-surface-noise-audit](default-surface-noise-audit-2026-06-14.md)). The human report omits `generated`, `declaration`, and `shallow` from default output. Human-action integrations should keep `default` and treat the others as diagnostic/review surfaces. This is ranking/presentation policy, not detector exactness. |
| `actionability_reason` | string, optional | The decidable, classification-not-verdict reason this family is **not** a clean default-surface refactor candidate — absent for a clean candidate. One of `trivial` (`mean_lines` ≤ 4 — too small to extract), `shallow-extraction` (unproven, helper-mostly-parameters: `params` ≥ a third of `shared_lines`), `declaration-run` (every member span is only import/use/include/re-export), or `generated-source` (every location in generated-header source). Each is a *reason*; `recommended_surface` is the placement it drives (`trivial`→`hidden`, `shallow-extraction`→`shallow`, the other two their like-named surfaces). Proven channels (`exact-value-graph`/`shared-sub-dag`) never carry `trivial`/`shallow-extraction`. See [default-surface-noise-audit](default-surface-noise-audit-2026-06-14.md). |
| `extraction_shape` | string, optional | The **decidable structural shape** of the fix if this family is acted upon — present only for a clean candidate (no `actionability_reason`). NOT a worth-it claim (that judgment is the consumer's, [design](design.md) §2). One of `call-existing-helper` (one named helper + inline copies that recompute it), `extract-helper`, `extract-method-from-block`, `consolidate-type`, `extract-base-class`, `consolidate-cross-language`. The same structural decision the human report's `→` hint renders in prose. |
| `overlap_primary_id` | string, optional | Present when this family is an overlapping slice of another default-surface family (≥2 member pairs overlapping on the same file by ≥ half of the shorter span): the `family_id` of that primary. The human report folds slices under their primary as one opportunity; JSON keeps every family so consumers can group or ungroup freely. |
| `baseline_status` | string, optional | `new` or `changed` when this family is shown because of `--baseline`. |
| `abstraction_witness` | object, optional | Experimental weak-claim witness emitted only for `--mode abstraction` families that share a normalized template with one supported literal leaf hole. |
| `witness` | object, optional | The agent-facing equivalence witness: WHY the members merged. `kind` is `exact-value-graph` (every member strict-exact-safe with one identical value multiset; `value_nodes` carries its size — for proof substance see `semantic_laws` and fragment `reason_code`), `shared-sub-dag` (a common heavy anchor; see each location's `shared_subdag` span), `copy-paste-run` (token-identical contiguous run), or `structural-similarity` (the fuzzy near channel; `mean_value_jaccard` vs `mean_shape_jaccard` distinguish behaviorally-driven convergence from surface likeness). For `structural-similarity` families the optional `graded` object grades that score into "equal except *k* holes" — see [graded-witness](graded-witness.md) and the row below. |
| `witness.graded` | object, optional | The anti-unification grade of a `structural-similarity` family's two representative copies (#315): how nearly equal they really are, beyond the score. `holes` is *k* (the spots where the two value DAGs differ); `spots[]` itemizes them (each with a value `class` — `literal`/`input`/`field`/`call`/`lambda`/`operator`/`expr` are clean parameters, `arity`/`shape`/`unmodeled`/`extra-sink`/`decorator` are structural divergence — an `effect` flag, per-side `a_lines`/`b_lines`, and trimmed `a_text`/`b_text`). `equal_modulo_holes` is true when every hole is a small value leaf, the behavior sinks aligned, no name mismatched, and the copies' decorators match — the strongest near-channel grade. `patterns[]` names recognized divergence shapes (`effects-reordered`, `sink-superset-a`/`-b`, `fragment-containment`, `low-substance`, `referent-mismatch`, `decorator-differs`). `referent_mismatches[]` lists names both copies consume that resolve to *different* definitions (which **demotes** the claim — fail-closed); `caveat_names[]` lists names unresolved on one side (the claim is scoped past them). `modeled_caveat` is true when either copy passed lossy lowering, so "equal" means equal in the modeled fraction. Scope: the unit *body* the value graph models plus a source-level decorator/attribute comparison (a `@deco(x)` vs `@deco(y)` difference fires `decorator-differs` and demotes the claim); the parameter *signature* is still not modeled (a differing unused parameter is invisible). Same-language near families only; absent for cross-language, fragments, and pathological files. Near-channel evidence, not an exact-channel proof. See [graded-witness](graded-witness.md). |
| `varying_spots` | array, optional | WHAT differs between the two representative copies — one entry per varying spot the extracted helper would parameterize (same representative pair as `params`, so the counts agree). Each spot carries `param` (1-based), `a_lines`/`b_lines` (absolute inclusive `[start, end]` per side; one side absent for pure insertions), and trimmed, length-capped `a_text`/`b_text`. Combined with the witness's shape-vs-value Jaccard, an all-literal spot list identifies a data-table family without reading source. Absent for cross-language families and until the presentation layer reads source. |
| `semantic_laws` | array, optional | Deduped pack-facing law provenance for value-graph laws that actually rewrote or bridged this family. Current first-party rows include `pack_id`, `pack_hash`, `law_id`, `channel`, `proof_status`, and `proof_obligation_id`. External local LawPack manifests are `metadata-only` today and do not appear here unless future producer execution admits them through the kernel. |

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
| `span_lines` | integer | Inclusive source-line span for this location. |
| `span_tokens` | integer | Normalized-token span used by the detector's size gates. |
| `in_test_module` | boolean, optional | Present (and `true`) when the location sits inside an inline test module (`mod tests` / `mod test`) — Rust keeps tests inside production files, and these count as test scope even when the path looks like production. |
| `looks_generated` | boolean, optional | Present (and `true`) when the file's head carries a generated-code marker (`Generated by …`, `@generated`, `Code generated … DO NOT EDIT`). A family whose every location looks generated reports `recommended_surface: "generated"` (the families the human report omits from default output); a partly-generated family stays on its ranked surface with generated members flagged. |
| `shared_subdag` | array, optional | `[start_line, end_line]` inclusive range of the heavy shared computation at this site when the family is grouped by a shared sub-DAG. |
| `is_fragment` | boolean | `true` when this location is an exact sub-function fragment; `false` for whole units and syntax-channel copy-paste spans. |
| `fragment_kind` | string, optional | Exact fragment proof shape, present only when `is_fragment` is `true`; examples include `direct-return`, `conditional-guard`, and `self-field-body`. |
| `reason_code` | string, optional | Stable exact-fragment proof reason derived from `fragment_kind`, present only when `is_fragment` is `true`; examples include `exact-direct-return` and `exact-conditional-guard`. |
| `enclosing_unit` | object, optional | Enclosing function/method/class recovered from the extracted unit set when available — for exact fragments AND plain `Block` locations (structural and copy-paste alike), so a block family names its host. Absent when a copy-paste run crosses unit boundaries, and in syntax-only scans (no unit set is extracted). |

### Fragment metadata

Exact semantic fragments are reported as ordinary family locations plus additive metadata.
`is_fragment` is always present; `fragment_kind`, `reason_code`, and `enclosing_unit` are
present only when the detector has exact data to serialize. A fragment with no
`enclosing_unit` is still a valid exact fragment; it only means no containing
function/method/class was recovered without guessing.

The optional `enclosing_unit` object has:

| field | type | meaning |
|---|---|---|
| `file` | string | Enclosing unit path, rewritten with the same relative-path policy as `locations[].file`. |
| `start_line` | integer | 1-based inclusive start line. |
| `end_line` | integer | 1-based inclusive end line. |
| `kind` | string | Enclosing `Function`, `Method`, or `Class`. |
| `name` | string, optional | Enclosing symbol name when recoverable. |
| `unit_key` | string | Stable key built from file, kind, span, and name for grouping/review context. |

Do not confuse fragment `reason_code` with the family-level `actionability_reason`. Fragment
`reason_code` answers why this sub-function fragment was accepted as exact-safe.
`actionability_reason` answers why a clone family is **not** a clean refactor candidate
(`trivial` / `shallow-extraction` / `declaration-run` / `generated-source`), and
`extraction_shape` names the structural shape of the fix for the families that **are** clean —
both **decidable classifications, not verdicts**. The judgment-deep half (worthy-fixture vs
intentional scaffolding, "is this worth coupling?") is deliberately NOT a code: it is left to
the consumer's own model and carried as evidence (`witness.kind`, `scope`, `params`/`shared_lines`,
`varying_spots`, `reinvented_helpers`, `graded`), never a `refactorability_score`/`confidence`
verdict — see [design](design.md) §2 and the
[default-surface-noise-audit](default-surface-noise-audit-2026-06-14.md). The experimental
`abstraction_witness.reason_code` below is a weak-template reason, not exact-fragment proof.

### Abstraction witnesses

`abstraction_witness` is present only for the hidden experimental
`scan --mode abstraction[:T]` surface. It is a sibling claim to `semantic`, not a
relaxed semantic clone verdict: the family is a refactoring-template candidate whose
members have identical normalized structure except for one shared supported literal
leaf position. Operator swaps, call-shape differences, and multi-hole or inconsistent
family diffs do not receive a witness.

The object has:

| field | type | meaning |
|---|---|---|
| `claim` | string | Claim class for this object. Current value: `weak-refactoring-template`. |
| `basis` | string | Scope of evidence used to build the witness. Current scan output value: `family`, meaning every reported family member was checked against the same template hole. |
| `members_checked` | integer | Number of family members checked when building the witness. |
| `reason_code` | string | Stable weak-template reason. Current values are `type-parametric` for int/float literal holes and `literal-abstracted` for same-class int, float, or string literal holes. |
| `template_format` | string | Encoding used by `template`. Current value: `normalized-il-preorder`. |
| `template` | array of strings | Pre-order normalized IL template tokens with `<hole 1: literal>` at the abstracted leaf. This is intentionally internal and machine-oriented, not source text. |
| `holes` | array | Typed hole metadata for the family witness. v1 emits exactly one hole. |
| `caveats` | array of strings | Caveats attached to the weak claim. `numeric-domain-sensitive` is emitted for int/float literal holes. |

Each `holes[]` item has:

| field | type | meaning |
|---|---|---|
| `index` | integer | 1-based hole index in the template. |
| `template_index` | integer | 0-based index into the `template` array, so tooling can join the hole metadata back to the machine template. |
| `kind` | string | Hole kind; currently `literal`. |
| `role` | string | Structural role of the hole. Current value: `leaf`. |
| `left` | string | Left representative leaf class, such as `int-literal`, `float-literal`, or `string-literal`. |
| `right` | string | Right representative leaf class. |
| `observed` | array of strings | Unique literal classes observed at this hole across the checked family. |
| `left_line` | integer | Source line for the left representative leaf when known. |
| `right_line` | integer | Source line for the right representative leaf when known. |

`proof_facts` are not part of the stable scan JSON contract. They remain internal
diagnostic facts unless a future schema explicitly adds an unstable diagnostics namespace.

## Reinvented helpers

An additive, experimental top-level array `reinvented_helpers` — omitted when empty —
carrying the [containment findings](reinvented-helpers.md): a unit that reimplements an
existing pure helper inline instead of calling it. One entry per finding:

| field | type | meaning |
| --- | --- | --- |
| `helper_file` / `helper_name` | string / string? | The existing helper being reinvented. |
| `helper_start_line` / `helper_end_line` | integer | The helper's source range. |
| `container_file` / `container_name` | string / string? | The unit containing the reimplementation. |
| `container_start_line` / `container_end_line` | integer | The container's source range. |
| `site_start_line` / `site_end_line` | integer | Lines INSIDE the container where the helper's computation lives (falls back to the container range when the matched node carries no span). |
| `site_approximate` | boolean | The site is the whole-container fallback (the match is a synthesized loop fold with no precise span); the helper's computation is a sub-part of those lines, so do not mechanically replace the exact range. |
| `container_in_test` | boolean | The container is a test file. Such findings are judgment-deep (a test asserting the helper's value as a literal is circular to "fix") and are excluded from the bare-default human surface, but kept here and under `--show reinvented`. |
| `weight` | integer | Value-graph size of the matched computation — the ranking key. |

The claim is exact-grade (both units pass the strict exact gate; the match is a
hash-consed value-graph equality, not similarity). Whether a test or vendored container
is worth changing is the consumer's judgment call — filter by path.

## Compatibility

Consumers should branch on `schema_version` before parsing. In version 1, new
fields may be added to existing objects without changing `schema_version`, so
parsers should ignore unknown fields. Changing a documented field's type,
meaning, required presence, or path requires a new `schema_version`.
