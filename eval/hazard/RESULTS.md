# Hazard tuning — measured results

Evidence for calibrating `hazard()`. Pipeline + analysis in this directory
(`mine.py`, `analyze.py`, `tune.py`, `run_corpus.sh`); methodology in
[../../docs/hazard-benchmark.md](../../docs/hazard-benchmark.md); on each nose release
follow [../../docs/hazard-release-checklist.md](../../docs/hazard-release-checklist.md).

## Corpus (v1)

Monthly snapshots over the most recent ~60 months, `--mode semantic,near`, nose 0.5.0.
Labels from git over nose-identified family member spans (Kim Inconsistent-Change):
G1 = some siblings edited not all; G0c = all together; G0s = none; **G2 = a G1 whose
changed sibling was modified by a bug-fix commit that did not propagate** (gold; loose
file-level/interval proxy — see limits).

| repo | lang | stratum | G1 | G2 |
|---|---|---|---|---|
| pandas | Python | S | 1248 | 580 |
| kafka | Java | S | 800 | 95 |
| django | Python | S | 709 | 390 |
| terraform | Go | S | 648 | 80 |
| hugo | Go | S | 434 | 139 |
| tokio | Rust | S | 266 | 34 |
| grpc | C++ | **X** | 181 | 25 |
| redis | C | S | 132 | 58 |
| thrift | C++ | **X** | 119 | 48 |
| ripgrep | Rust | S | 60 | 22 |
| vue-core | TypeScript | S | 37 | 17 |
| express | JavaScript | S | 5 | 2 |
| **total** | **8 langs** | | **4,639** | **1,490** |

462,569 family-interval events; 15,199 distinct families (24.8% ever G1, 8.2% ever G2).
**Meets the benchmark floors:** repos ≥ 12 (=12), G1 ≥ 1000 (4,639), G2 ≥ 80 (1,490),
G2-per-stratum ≥ 40 (X = thrift 48 + grpc 25 = 73). Still missing: human-audited gold
subset; deeper X stratum (2 repos).

## Headline finding — the literature-derived formula was mis-specified

Leave-one-repo-out logistic weights (stable, low variance across 12 held-out repos):

| feature | weight | direction |
|---|---|---|
| `mean_lines` | **+0.43** | ↑ hazard (strongest) |
| `modules` (dispersion) | **+0.28** | ↑ hazard |
| `mean_sem` (semantic size) | **−0.27** | **↓ hazard (anti-predictive)** |
| `invisibility` (1−tightness) | **+0.14** | ↑ hazard |
| `members` (copies) | +0.13 | ↑ (redundant with lines/modules) |
| `params` | +0.04 | ~noise (flipped sign from −0.06 at 7 repos) |
| `languages` | +0.03 | ↑ weak |

The pre-data design led with `mean_sem` as the **primary multiplier** — but semantic
size is *anti-predictive* for divergent-edit ranking (typical divergences are in
smaller-fingerprint families; the mean is a large-tail artifact). Source **line** span
is the real magnitude signal. `invisibility` is robustly positive and is the **top
signal in the cross-language stratum** (per-family AUC 0.67, P@10 0.80) — the Type-4
"invisible sibling" hypothesis held exactly where predicted.

## Candidate formulas (leave-one-repo-out test AUC)

| formula | vs G1 | vs G2 (gold) |
|---|---|---|
| **v5 = mean_lines × spread(files,modules,langs) × invisibility × scope** | 0.644 | **0.682** |
| v7 = v5 × 1/(1+0.5·params) | 0.659 | 0.669 |
| v1 = the original size-led design | 0.609 | 0.644 |
| value (raw-volume baseline) | 0.611 | 0.671 |
| random | ~0.49 | ~0.49 |

The param-dampening term (v7) helps G1 marginally but hurts the gold label and rests on
a sign-unstable weight — **dropped**. v5 is simplest and best on the gold label.

## Decision: the formula to implement

```
hazard = mean_lines
       × spread(files, modules, languages)   // dispersion (existing helper)
       × invisibility                        // 0.3 + 0.7·(1 − tightness)
       × scope_weight                        // prod 1.0 / mixed 0.5 / test 0.25
```

Beats the pre-data size-led design on both labels (G1 0.644 vs 0.609; G2 0.682 vs
0.644). All terms reuse existing `RefactorFamily` fields; **`mean_sem` is dropped** as a
magnitude term, **`params` is not used** (noise).

## Honest limits

- AUC ≈ 0.65–0.68 is a useful *ranking* signal, not a precise predictor — divergent-edit
  propensity is inherently noisy from static features.
- **G2 is a loose proxy**: bug-fix attribution is file-level and interval-aggregated, so
  G2-among-G1 rates (13–46%) are far above the literature's ~1–3% release-level — an
  upper bound. Tightening needs line-level/commit-level (SZZ) attribution and a
  human-audited gold subset (benchmark Tier-1 quality controls).
- X stratum is 2 repos (thrift, grpc); needs more cross-language repos.
- Re-run on a new nose version: `run_corpus.sh` then `tune.py all-events.jsonl`
  (see the release checklist).
