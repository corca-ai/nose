# Bounded same-unit windows (#832)

Issue #832 closes the smallest source-coherent tranche left after the pair-local connected
witness work: two actionable, disjoint regions inside one enclosing function or method.

## What shipped

- Product mode considers at most two eligible functions or methods per file and 4,096 globally.
  Each unit compares at most 256 pairs of whole normalized-IL subtrees with at least 20 nodes and
  three source lines.
- A pair must preserve subtree kind, arity, statement order, control exits, observable call/effect
  roles, and a consistent mapping of at most eight value holes. The two source spans must be
  disjoint. Bare `Block` roots are excluded because a scope container can erase the construct that
  owns it, such as a switch arm.
- The route has separate work, ranking, deduplication, and output budgets from the #821 cross-unit
  routes. It keeps one strongest witness per unit, one per file, and at most 32 globally, so it
  cannot displace an existing connected result.
- Query output uses the `bounded-window` witness and reports exactly two bounded locations. The
  enclosing unit is ownership context, not a location. These rows are `near` refactoring
  candidates: they never set exact-fragment metadata or claim exact behavioral equivalence.
- Existing families remain authoritative. A same-unit row can be suppressed only at an identical
  pair of sites and cannot alter the existing opportunity-folding forest.

## Frozen dev review

The proposal was frozen at `539fd680b62a4cc9f829c9724ed3d59ada6ff39b` before reviewing
additional dev recoveries. The three required IDs recover as bounded pairs:

| ID | product locations | result |
| --- | --- | --- |
| `git:1d9b1c7f444d15d0` | `remote-curl.c:74-82`, `99-107` | coherent option branches |
| `gorm:b2cd9c61ce98289a` | `joins_test.go:19-29`, `30-40` | coherent JOIN rows |
| `chi:154bd73cf15a6de5` | `route_headers_test.go:93-98`, `163-168` | coherent callback regions |

The mandatory `tmux:6c7a09c2a9c5c2b2` control remains absent because its two label members
overlap and are shifted views of one cursor-advancement site. The only independent extra dev
family is a coherent pair of clap assertion rows. Gorm's second credited ID is an evaluator alias
for the already-reviewed JOIN output, not another product family. A provisional Gson switch-arm
recovery came from the frozen `unrecovered` lane; the post-freeze review therefore tightened the
general root contract and removed it without a language, repository, file, or symbol exception.
The checked review ledger is
`bench/labels/bounded_same_unit_dev_review_issue_832_2026_07_13.v1.json`.

Held-out source was never opened. Only the evaluator's mechanical ID comparison is used below.

## Quality and price

`FINAL_EVAL`

`FINAL_OUTPUT`

The official v0.18.0 Darwin arm64 release asset remains the primary performance baseline.
`FINAL_RUNTIME`

## Evidence

- `bench/labels/product_quality_evaluation_post_821_2026_07_13.v1.json`
- `bench/labels/recall_ceiling_probe_post_821_2026_07_13.v1.json`
- `bench/labels/missed_worthy_stage_audit_post_821_2026_07_13.dev.v1.json`
- `bench/labels/bounded_same_unit_dev_review_issue_832_2026_07_13.v1.json`
- `bench/labels/product_quality_evaluation_issue_832_2026_07_13.v1.json`
- `bench/labels/bounded_same_unit_pricing_2026_07_13.release-primary.v1.json`
- `bench/labels/bounded_same_unit_pricing_2026_07_13.post-821.v1.json`
- `bench/labels/bounded_same_unit_pricing_2026_07_13.control.v1.json`

## Next decision

After merge, rerun the frontier from merged `main` and select from the remaining measured misses.
Do not automatically broaden same-unit search to every residual row. If the refreshed same-unit
cohort is not coherent under this exact root/overlap contract, select one construct-specific
extraction cohort instead of mixing unrelated mechanisms.
