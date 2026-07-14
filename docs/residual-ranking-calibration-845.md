# #845 residual ranking calibration

Issue #845 asks whether a transparent, deterministic ranking can make the residual
default head reach 70% precision without sacrificing coverage or a language for an
aggregate gain. The complete dev experiment now closes as a fully judged **no-go**:
none of the 46 pre-registered formulas passes every gate. Product ranking is unchanged.

## Why the first result is evidence-incomplete

The calibration runs the unchanged #843 binary (`f7fcda…fd0`) over every default family
from all 66 pinned dev repositories: 29,348 families, never a truncated candidate pool.
It opens only the explicit v5-dev, v6-dev, and v7-dev label components. Held-out labels,
repositories, judgments, results, and the v7 composite remain closed.

Three independent reviews reproduced the collection, formulas, metrics, and eight-fold
repository CV. They also found that the apparent precision gain of aggressive locality
weights coincided with candidate-dependent label loss. Some formulas could still reach
70% if their unmatched positions were worthy, so rejecting those formulas on label
coverage would confuse missing evaluation evidence with product quality.

The reviews found three more contract gaps, all corrected in the frozen calibration:

- conflicting best-overlap labels now fail closed instead of depending on JSON row order;
- language floors and regression guards apply by reported positions, so a formula cannot
  escape a weak-language gate by losing its labels;
- `current` uses the collected full product order, while experimental formulas have an
  explicit final family-key tie-break. The duplicate baseline grid cell was removed,
  leaving 46 unique IDs and formulas.

The fail-closed baseline is provisionally `380/645 = 58.91%`, with 13 unresolved slots.
That is not a new product result: two formerly counted worthy positions are deliberately
unresolved because their best-overlap labels conflict. Exact panel decisions replace
those ambiguous projections in the second phase.

## Frozen, judgment-blind top-up

The top-up packet is the union of every truth-null family appearing in the top ten of
any of the 46 formulas. It contains 219 distinct dev families:

- every unmatched formula top-10 position;
- every position whose equally strong overlap labels disagree;
- no sampling, coefficient-driven second round, or held-out input.

This deliberately uses the safe upper bound instead of the smaller packet needed by the
current full-dev winners. Once all 219 receive exact candidate-key judgments, every
formula has 100% top-10 label coverage on every repository. The same packet covers every
training and validation fold, so fold-specific selection cannot discover a new unlabeled
denominator.

The selection artifact stores the complete truth-free raw families, proposal membership,
source bounds and hashes, frozen query hashes, exact dev input hashes, and selection
digests. It recursively rejects judgment fields such as `worthy`, `reason`, `votes`, and
`arbiter`. CI re-derives the 219 keys from the 29,348-family calibration, reconstructs
every raw family into its compact facts, and rejects missing, extra, reordered, or mutated
candidates.

## Split-safe judgment protocol

Selection and judgment were separate merges. The blind projection binds selection commit
`6e9a2d08…dbc1`, tree `17468036…aee3`, and selection byte hash `f3b4ec65…058f`.
It hides current rank, formula membership, and prior truth status while exposing the
complete raw family and hash-bound source files. Three subagents then independently read
all 219 families under the same rubric:

| persona | worthy | not worthy |
|---|---:|---:|
| pragmatic | 159 | 60 |
| dedupe | 142 | 77 |
| skeptic | 131 | 88 |

Exact `(worthy, reason)` agreement covered 129 candidates. The independent arbiter
re-read all 90 disagreements, including 34 worthiness splits and 56 reason-only splits;
no majority vote was applied mechanically. The final component contains 137 worthy and
82 not-worthy labels. Raw votes, arbitration, decisions, and component are independently
hash-bound and CI replays their exact ordering. The precision overlay maps only by frozen
candidate key, rejects replacement of known truth, and never propagates through fuzzy
overlap.

## Fully judged result

All 46 formulas now have 100% top-10 truth coverage for every repository and language.
The current order measures `387/658 = 58.81%`. The best formula that preserves coverage
and the five-point regression guard is `grid-s-1.00-same0.65-conn1.00`:

| result | hits / positions | P@10 | status |
|---|---:|---:|---|
| current full dev | 387 / 658 | 58.81% | baseline |
| best coverage-guarded full dev | 449 / 658 | 68.24% | 12 hits short of 70% |
| repository-CV out of fold | 416 / 658 | 63.22% | no generalizing pass |

The best full-dev formula also leaves C at `44/90 = 48.89%`, one hit below the 50%
language floor. Java and Swift rise above 50%, and no language regresses, but aggregate
improvement cannot waive the remaining C failure or the overall 12-hit shortfall. No
formula is eligible, no signal or proposal is retained, and held-out remains unopened.

This is a real no-go rather than a missing-label proxy. Issue #846 may measure and close
the unchanged product, including its frozen held-out and fresh-repository audit, but it
must not turn held-out evidence into another ranking-tuning round.

## Reproduction

```sh
python3 bench/labels/residual_ranking.py validate
python3 bench/labels/residual_ranking.py self-test
python3 bench/labels/residual_ranking_topup.py validate
python3 bench/labels/residual_ranking_topup.py self-test
python3 bench/labels/residual_ranking_panel.py validate-arbitration
python3 bench/labels/residual_ranking_panel.py validate-decisions
python3 bench/labels/residual_ranking_panel.py validate-component
python3 bench/labels/residual_ranking_panel.py self-test
python3 bench/labels/residual_ranking_closeout.py validate
python3 bench/labels/residual_ranking_closeout.py self-test

# With pinned dev repositories and the frozen #843 binary present:
python3 bench/labels/residual_ranking.py collect \
  --nose target/release/nose \
  --output target/issue-845-dev-all-default.v1.json
python3 bench/labels/residual_ranking.py freeze \
  --input target/issue-845-dev-all-default.v1.json
python3 bench/labels/residual_ranking_topup.py freeze \
  --collection target/issue-845-dev-all-default.v1.json
```

Product code, surfaces, family membership, worthy recall (`2716/2849`), determinism,
and the official-v0.19.0 performance contract are unchanged.
