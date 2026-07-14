# #845 residual ranking calibration

Issue #845 asks whether a transparent, deterministic ranking can make the residual
default head reach 70% precision without sacrificing coverage or a language for an
aggregate gain. This first phase freezes the complete dev experiment and its judgment
frontier. It does not change product ranking or declare go/no-go yet.

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

Selection and judgment are separate merges. This merge contains only the unjudged packet.
The next merge must bind this exact selection commit and byte hash, then record three
independent rubric-based votes for all 219 candidates. Unanimous votes become panel
decisions; disagreements require a separate arbiter. The final precision-only overlay
maps by exact candidate key and must never propagate through fuzzy overlap.

After the panel is frozen, the evaluator will rerun the same 46 formulas and the same
eight repository folds. A passing formula must reach 70% P@10, preserve the coverage and
language gates, and be frozen before #846 opens held-out. If none passes, #845 can close
with a genuine fully judged no-go rather than a missing-label proxy.

## Reproduction

```sh
python3 bench/labels/residual_ranking.py validate
python3 bench/labels/residual_ranking.py self-test
python3 bench/labels/residual_ranking_topup.py validate
python3 bench/labels/residual_ranking_topup.py self-test

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
and the official-v0.19.0 performance contract are unchanged in this phase.
