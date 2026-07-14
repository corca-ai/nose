# #845 residual deterministic ranking no-go

Issue #845 tested whether transparent, language-neutral evidence could make the
post-#844 bare-default head reach 70% precision without hiding judgment-deep clone
families. The answer on the complete dev default universe is no. No product ranking,
surface, detector, or output changed, and held-out evidence remains closed for #846.

## Frozen input and current result

The calibration opens only the explicit v5-dev, v6-dev, and v7-dev label components and
the 66 dev repository checkouts pinned by the corpus. It does not resolve the v7
composite manifest or read a held-out label, seal, repository source, judgment, or
result. The input binary is the #843 current product (`f7fcda…fd0`) built from
`a0b2730d`; all 29,348 default-surface families are retained so an aggressive rerank
cannot win because the experiment truncated the candidate pool.

The complete current result reproduces the checked #843 quality artifact:

| metric | current |
|---|---:|
| labeled P@10 | `382/647 = 59.0417%` |
| top-10 label coverage | `647/658 = 98.3283%` |
| worthy slots among all reported slots | `382/658 = 58.0547%` |
| best case if every unmatched slot were worthy | `393/658 = 59.7264%` |

The language P@10 values are C 44.44%, Go 66.25%, Java 33.71%, Python 70.00%,
Ruby 81.11%, Rust 61.54%, Swift 48.57%, and TypeScript 68.75%. C, Java, and Swift
already expose the tracker's language-floor problem; an aggregate-only improvement is
not sufficient.

## Independent review and pre-registered proposals

Three subagents independently reproduced the baseline from dev-only inputs, reviewed
the current score and hard constraints, and did not read one another's work. All three
identified the positive file/module spread reward as the strongest plausible lever and
recommended a full-universe repository-level cross-validation before changing runtime
behavior. They separately warned that apparent gains from aggressive locality weighting
could be unmatched-label denominator loss rather than more worthy results.

The checked calibration therefore freezes 47 proposals before selecting any result:

- a 30-cell grid over spread exponent `{-1, -0.5, 0, 0.5, 1}`, global same-symbol
  weight `{0.65, 0.8, 1}`, and connected-witness weight `{1, 1.15}`;
- bounded module-bonus and `multi-module && !same-symbol` interactions;
- one-axis changes to parameter penalty, tightness, member-span homogeneity,
  implementation-type origin, bounded-window witness, and exact witness;
- the reviewers' conservative composite.

Every formula uses only facts already present on a family. Missing origin is neutral.
There are no repository identifiers, language coefficients, labels, source/path/symbol
allowlists, test-scope inputs, JSX rules, or parallel-by-design verdicts in a proposal.
The experiment retains the product's total order: score, raw value, then first source
anchor.

## Result

| result | labeled P@10 | coverage | worthy slots | best-case slots |
|---|---:|---:|---:|---:|
| current | 59.04% | 98.33% | `382/658` | 59.73% |
| best precision regardless of gates (`spread=-1`, same-symbol `0.65`) | 67.12% | **77.66%** | `343/658` | 74.47% |
| best coverage/regression-guarded (`spread=0`, same-symbol `0.65`, connected `1.15`) | 62.50% | 87.54% | `360/658` | **67.17%** |
| module bonus removed | 60.93% | 95.29% | `382/658` | 62.77% |
| reviewer composite | 61.91% | 93.77% | `382/658` | 64.29% |

The unconstrained winner loses 39 known worthy slots and fails coverage by 7.34
percentage points. The coverage/regression-guarded winner loses 22 known worthy slots;
even if every unmatched slot were worthy, it could reach only 67.17%. It also leaves
Java at 35.90% and Swift at 49.12%. No proposal simultaneously satisfies 70% P@10,
85% coverage, the per-language 50% floor, and the five-point language-regression guard.

The one-axis ablations do not reveal a hidden win. Removing the module bonus leaves the
known worthy-slot count unchanged; stronger homogeneity adds one known hit but reaches
only 59.20%; a parameter change loses ten; exact-witness promotion loses two. This is
consistent with #844's proof/actionability separation.

## Repository cross-validation

The calibration uses a deterministic eight-fold repository split, assigning sorted
repositories round-robin within each language. A repository never appears in both train
and validation for a fold. Training may select only a proposal that satisfies coverage,
language floor, and language-regression guards. No changed proposal is eligible in any
fold, so all eight folds select the current score. The out-of-fold result is therefore
the exact current `382/647 = 59.0417%`, not a post-selected full-dev estimate.

This is a no-go, not an invitation to widen the grid. The failure is large and
structural: the strongest precision-looking direction trades away both coverage and
absolute worthy hits, while the guarded family cannot reach 70% even under the most
optimistic missing-label assumption.

## Preservation and reproduction

The product code is unchanged. Consequently family IDs, members, witnesses, origins,
surfaces, reason codes, accepted-pair coverage, full-universe worthy recall
(`2716/2849`), deterministic order, false-merge/canon invariants, and runtime cost are
the #843 values. This also preserves the performance contract against the published
v0.19.0 binary without introducing a new runtime stage or arithmetic operation.

The compact checked artifact contains the complete 29,348-family dev fact projection,
per-repository query hashes and commits, all proposal definitions/results, fold
assignments, independent review records, and exact input hashes. Its frozen semantic
digest is `f569d4…3a3fb`. CI reconstructs every proposal, gate, and cross-validation
decision; self-tests reject held-out-state, dataset, decision, and review mutations.

```sh
python3 bench/labels/residual_ranking.py validate
python3 bench/labels/residual_ranking.py self-test

# With the pinned dev repositories and frozen #843 binary present:
python3 bench/labels/residual_ranking.py collect \
  --nose target/release/nose \
  --output target/issue-845-dev-all-default.v1.json
python3 bench/labels/residual_ranking.py freeze \
  --input target/issue-845-dev-all-default.v1.json
```

## Next step

#846 owns the one-time blind closeout. It may evaluate the unchanged ranking to quantify
the final tracker residual and fresh-repository behavior, but it must not tune a
coefficient from held-out results. If the epic remains below target, #838 should close
with that exact residual or be explicitly re-scoped rather than manufacture precision
through lower coverage, hidden parallel families, or language-specific behavior.
