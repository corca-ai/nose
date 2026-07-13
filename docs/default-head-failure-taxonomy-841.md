# #841 default-head failure taxonomy and bounded levers

Issue #841 classifies every labeled dev position in the bare-default first ten and freezes
the mechanically decidable cohorts that #842–#844 may implement. It changes no query
output, ranking, detector threshold, or surface policy. Held-out source remains closed.

## Reproduced input and taxonomy

The published nose 0.19.0 binary was replayed on the 66 dev repositories pinned by the
corpus. All 66 query stdout hashes and all 658 first-ten family IDs, ranks, raw-family
hashes, and source bounds match the split-safe [#840 runway](default-head-label-runway-840.md).
The collector opens only the hash-bound `refactoring_families.v5.dev.json`, `v6.dev`, and
`v7.dev` components; it never parses the v5 mixed-split file, resolves the composite v7
manifest, or opens a held-out component. The v5 dev projection contains exactly 5,445 dev
rows and rejects non-dev repositories, member paths, or line bounds.

The checked core (`default_head_taxonomy_2026_07_13.dev.core.v1.json`, semantic SHA-256
`d39648…a036`) gives every position one truth bucket and a separate mechanical bucket.
This separation prevents a label such as `generated` from masquerading as product proof,
and prevents a strong `exact` or `subdag` witness from masquerading as an actionability
verdict.

| truth bucket | positions |
|---|---:|
| worthy: extract helper | 251 |
| worthy: parameterize | 95 |
| worthy: extract base | 19 |
| worthy: extract data table | 17 |
| non-action: parallel by design | 148 |
| non-action: trivial | 48 |
| non-action: generated | 41 |
| non-action: coincidental shape | 30 |
| non-action: type definition | 9 |

The core stores the full raw query family, primary and actual member languages, witness,
scope and extraction shape, all/partial/none origin coverage and origin facets, generated
provenance, distinct top-level and ranking `shared`/`params` values, same-language ranking
tightness, population member-span CV, path/module relation, unknown ownership, and
repository breadth. Cross-tabs reproduce from the 658 rows in CI. Origin coverage is all
for 257 positions, none for 395, and partial for 6; partial is never collapsed into
unknown or all.

## Frozen mechanical decisions

### Generated documentation provenance

`generated-provenance.v1` requires every unique member file to be HTML and its bounded
first 64 KiB to contain both a Jazzy asset marker and an Apple/Dash generated-symbol
anchor. A repository, language, path, or symbol allowlist is not part of the predicate.

- Dev head: 10/10 positives are non-action, all in one Alamofire Jazzy output corpus.
- Independent deep audit: each of three reviewers inspected all 20 positives and all
  2,818 member bounds; each returned 20/20 premise-held, non-actionable judgments.
- Hard negatives: three worthy HTML families demonstrate that an `.html` suffix is not
  provenance.
- Estimated head movement: 10 vacated positions. Replacement ranks are deliberately not
  projected before #842 implements and measures the product transition.
- Cost bound: read at most 64 KiB once per unique candidate HTML file. The family must
  remain recoverable through `all top=0` with a reason-coded surface.

This is a narrow, one-generator/one-repository dev proof, not a claim that every generated
family is recognized. #842 may implement only this frozen provenance contract and must
price it against the published v0.19.0 binary.

### Declaration-only type contracts

`declaration-only-type.v1` requires complete origin evidence for every member: whole-unit
`type-contract`, `declaration-only` body, both declaration-only and type-only evidence,
and no runtime, implementation, data, runtime-body, or reusable-body evidence. Missing or
mixed origin fails closed.

- Dev head: 1/1 positive is non-action.
- Independent deep audit: each reviewer inspected all four positives and ten member
  bounds; each returned 4/4 premise-held, non-actionable judgments.
- Hard negatives: eight worthy rows bind partial origin, missing origin, and reusable
  implementation-body boundaries.
- Estimated head movement: one vacated position.
- Cost bound: one all-member predicate over origin facets already present in query data.

The predicate intentionally does not stretch to catch the nine truth-labeled `type-def`
rows. #843 may implement this narrow contract; enums, runtime schemas, default
implementations, extensions, mixed families, and unknown evidence remain visible.

### Proof/actionability is a no-go

The `exact|subdag` cohort contains 57 head positions: only 22 are non-action and 35 are
worthy. Removing the existing protection would therefore have 38.60% non-action
precision. All 35 worthy rows are bound as hard negatives. The narrower existing
trivial/shallow predicates also have worthy failures, so #844 is frozen as a no-product-
change result unless a new, pre-registered cohort independently clears the same gate.

The dominant judgment-deep residue remains visible: 131 parallel-by-design and 29
coincidental-shape positions after proof-backed rows are separated. No automatic
parallel-by-design verdict, test-scope penalty, same-symbol/file rule, parameter-density
threshold, span-CV threshold, or ranking-tightness threshold is selected. Their head and
65-row deterministic deep-sample results and worthy hard negatives are checked as
rejected heuristics.

## Independent audit and validation

The standalone `default_head_taxonomy_audit_packets_2026_07_13.dev.v1.json` contains
source bounds and frozen mechanical evidence but no truth, worthy, reason, or label
fields. Exact nested schemas reject aliases and unknown fields. The core stores only its
24 unique keys and per-lever packet-set hashes, so reviewers do not need access to the
truth-bearing core. Three subagents independently reviewed the packet artifact without
reading one another's work or existing labels. Every reviewer returned 24/24
`premise_holds=true` and `non-actionable`, so both selected classifiers clear the 90%
gate independently.

The validator-hardening pass changed only packet identity bindings; a checked rebind
accepted the existing judgments only after proving every reviewer-visible packet
projection unchanged. The compact decision overlay
`default_head_taxonomy_2026_07_13.dev.v1.json` has artifact SHA-256 `45ac11…cddb` and
binds the 8.1 MB core, 1.8 MB blind packet artifact, three raw vote files, predicates,
source-bound hard negatives, costs, rejected alternatives, and audit summaries without
duplicating the core rows.

```sh
python3 bench/labels/default_head_taxonomy.py --self-test

python3 bench/labels/default_head_taxonomy.py validate \
  bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json \
  --pragmatic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.pragmatic.v1.json \
  --dedupe bench/labels/default_head_taxonomy_votes_2026_07_13.dev.dedupe.v1.json \
  --skeptic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.skeptic.v1.json

# Optional local-source and pinned-commit verification:
python3 bench/labels/default_head_taxonomy.py validate \
  bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json --live-sources
```

Validation reconstructs the final overlay from the bound core, blind packet artifact,
and votes. It exact-schema checks nested packets/votes; binds every row to the #840
candidate, raw-family, and source hashes; recomputes label truth, origin/ranking facets,
predicates, mechanical buckets, selected cohorts, cross-tabs, rejected heuristics, hard
negatives, and per-reviewer precision; and checks every provenance input against the
current files. `--live-sources` additionally resolves every path beneath its exact dev
repository and rechecks file bytes, hashes, commits, bounded generator evidence, and
signal locations. Duplicate audit keys, a worthy selected row, held-out path, unknown
packet field, missing or changed vote, incomplete origin, cohort overlap, or sub-90%
review fails closed. Self-tests preserve the reviewed invalid-artifact reproductions.

Next, #842 implements and prices the frozen generated-provenance transition, #843 handles
the declaration-only contract, and #844 records the proof/actionability no-go. They may
run independently, but #845 cannot tune residual ranking until all three close.
