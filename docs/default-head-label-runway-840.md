# #840 split-safe v7 default-head label runway

Issue #840 extends the v6 product-label pipeline with a dev-only precision overlay and a
source-free held-out commitment. It changes measurement coverage, not query behavior,
ranking, detection, or surface policy.

## Frozen inputs and selection

The published nose 0.19.0 binary (`0f73ea…0f3`) queried the 120 repositories pinned by
`bench/goldens/corpus.json` at their recorded commits. The initial collector was run twice
with byte-identical candidate content. After independent review tightened the deep-sample
wording and fail-closed validators, the same selection was re-frozen from clean commit
`af3ea50c`; only contract/provenance records and their dependent hashes changed.
The checked artifacts are:

- `default_head_label_runway_2026_07_13.dev.v1.json` (`2b9947…3d18`), with dev
  candidates, member locations, source hashes, query hashes, and repository revisions;
- `default_head_label_runway_2026_07_13.heldout.seal.v1.json` (`b99c39…3004`), with
  candidate commitments and selection metadata but no members, source paths, votes, or
  judgments.

Selection was frozen independently in each split. It includes every v6-unmatched current
default rank 1–10 family plus one v6-unmatched rank 11–30 family for every repository
that has at least one eligible family, chosen by the seed
`nose-issue-840-default-head-v7-rank-11-30`. Repositories without an eligible unmatched
rank 11–30 family contribute no deep sample.

| split | default positions | v6 matched | unmatched selected | rank 11–30 selected | total selected |
|---|---:|---:|---:|---:|---:|
| dev | 658 | 437 | 221 | 65 | 286 |
| held-out seal | 538 | 375 | 163 | 51 | 214 |

The selection covers all 15 Swift repositories: eight dev repositories and seven sealed
held-out repositories. No held-out member location or source excerpt is present in the
seal.

## Independent panel and arbitration

Three subagents independently reviewed all 286 dev selections against `RUBRIC.md`, without
reading one another's votes or any held-out material:

| persona | worthy | not worthy |
|---|---:|---:|
| pragmatic | 133 | 153 |
| dedupe | 158 | 128 |
| skeptic | 96 | 190 |

Exact worthiness-and-reason agreement covered 105 candidates. A fresh arbiter re-read the
dev context for all 181 disagreements rather than applying majority vote mechanically:
100 were resolved worthy and 81 not worthy, with 137 high-, 42 medium-, and 2
low-confidence arbitration decisions. The final 286-label component contains 146 worthy
and 140 not-worthy judgments; its 63 Swift labels cover every dev Swift repository and
both worthiness classes. Raw votes, arbitration, decisions, and the resulting component
are separately hash-checked artifacts. CI replays all three raw vote files through the
disagreement queue, arbitration, decisions, and generated component; deletion,
mutation, missing or extra votes, and queue drift fail closed.

Every v7 label is eligible only for `precision_at_10`. The v5 worthy-recall denominator is
unchanged, and the v7 loader fails closed if the frozen v5/v6 bytes or flattened family
projections drift.

## Measured result

`refactoring_families.v7.json` composes frozen v6, the 286-label dev overlay, and the
held-out seal. It contains 9,862 families: 5,790 dev and 4,072 held-out. On the official
v0.19.0 binary:

| split | labeled P@10 | top-10 coverage | full-universe worthy recall |
|---|---:|---:|---:|
| dev | 382/658 = 58.05% | 658/658 = 100.00% | 2716/2849 = 95.33% |
| held-out | 222/375 = 59.20% | 375/538 = 69.70% | 2005/2091 = 95.89% |

The lower dev point estimate is not a product regression: no output changed. v6 measured
271/437 labeled positions and omitted 221 unmatched positions from the precision
denominator; v7 labels all of them and exposes the complete dev head. Held-out stays on
the frozen v6 judgments until #846.

An explicit v6 replay reproduced the #839 configuration, all 120 repository results, and
all metrics exactly (stable core SHA-256 `17c013…a9`). The checked v7 report is
`product_quality_evaluation_v7_dev_runway_2026_07_13.v1.json` (`771cf6…ef22`). Bootstrap
streams are derived independently for each split, language/overall scope, and metric, so
dev sample growth cannot perturb an unchanged held-out interval. The corrected held-out
P@10 interval is again `[54.13%, 64.00%]`, identical to the v6 result for the same 222/375
flags.

The held-out seal uses an exact allowlist at every object level, rejects unknown fields,
and is validated by both the labelset loader and the runway gate. The v7 loader accepts
exactly one dev precision overlay plus the bound held-out seal; it rejects any held-out
judgment component.

## Validation and next step

```sh
python3 bench/labels/label_refresh.py validate-runway \
  --dev-candidates bench/labels/default_head_label_runway_2026_07_13.dev.v1.json \
  --heldout-seal bench/labels/default_head_label_runway_2026_07_13.heldout.seal.v1.json \
  --labelset bench/labels/refactoring_families.v7.json \
  --evaluation bench/labels/product_quality_evaluation_v7_dev_runway_2026_07_13.v1.json
```

The next step is #841: classify every now-visible matched dev head position into a complete
failure taxonomy, freeze disjoint mechanical cohorts and hard negatives, and leave
judgment-deep residue visible. Held-out source remains closed.
