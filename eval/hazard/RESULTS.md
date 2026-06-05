# Hazard tuning — first measured results

Evidence for calibrating `hazard()` before implementation. Pipeline + analysis in this
directory (`mine.py`, `analyze.py`, `tune.py`); methodology in
[../../docs/hazard-benchmark.md](../../docs/hazard-benchmark.md).

## Corpus (v0 slice)

Monthly snapshots over the most recent ~60 months, `--mode semantic,near`,
nose 0.5.0. Label = Kim Inconsistent-Change computed from git (G1 = some siblings
edited, not all; G0c = all together; G0s = none).

| repo | lang | stratum | G1 | G0c | G0s |
|---|---|---|---|---|---|
| django | Python | S | 709 | 2003 | 137250 |
| hugo | Go | S | 434 | 343 | 19882 |
| tokio | Rust | S | 266 | 193 | 26480 |
| redis | C | S | 129 | 136 | 14501 |
| thrift | C++ | **X** | 119 | 120 | 34396 |
| vue-core | TypeScript | S | 37 | 48 | 3869 |
| express | JavaScript | S | 5 | 6 | 594 |
| **total** | 7 langs | | **1699** | **2849** | **236972** |

241,520 family-interval events; 6,986 distinct families (21.1% ever had a G1).
Meets the benchmark **G1 ≥ 1000** floor; below the **repos ≥ 12** and **G2 (bug-linked)**
floors — those are the next phase.

## Headline finding — the literature-derived formula was mis-specified

Per-signal rank-AUC (G1 vs controls) and a leave-one-repo-out logistic fit agree:

| feature | logistic weight | direction |
|---|---|---|
| `mean_lines` | **+0.48** | ↑ hazard (strongest) |
| `modules` (dispersion) | **+0.29** | ↑ hazard (robust, low variance) |
| `invisibility` (1−tightness) | **+0.20** | ↑ hazard (robust) |
| `mean_sem` (semantic size) | **−0.18** | **↓ hazard (anti-predictive)** |
| `members` (copies) | +0.13 | ↑ weak |
| `params` | −0.06 | ↓ weak |
| `languages` | +0.04 | ↑ weak |

The original design (docs/hazard-ranking.md, pre-data) led with `mean_sem` as the
**primary multiplier** — but semantic-atom size is *anti-predictive* for divergent-edit
ranking: typical divergences happen in smaller-fingerprint families (the mean is pulled
up by a large tail, so the rank-AUC is < 0.5). Source **line** span (`mean_lines`) is the
real positive size signal. `invisibility` is confirmed positive overall and is the
**top signal in the cross-language (X) stratum** (per-family AUC 0.67, P@10 0.80) — the
Type-4 "invisible sibling" hypothesis holds exactly where predicted.

## Candidate formulas (leave-one-repo-out test AUC)

| formula | AUC |
|---|---|
| **v7 = mean_lines × spread(files,modules,langs) × invisibility × scope × paramdamp** | **0.674** |
| v5 = v7 without param dampening | 0.660 |
| v2 = eff_copies × spread × invisibility × scope (size dropped) | 0.625 |
| **v1 = the original size-led design** | **0.619** |
| value (raw-volume baseline) | 0.600 |
| random | ~0.49 |

`paramdamp = 1/(1+0.5·params)` — note this is the **same** parameter penalty
extractability uses, so hazard and extractability agree on it.

## Decision: the formula to implement

```
hazard = mean_lines
       × spread(files, modules, languages)   // dispersion (existing helper)
       × invisibility                        // 0.3 + 0.7·(1 − tightness)
       × scope_weight                        // prod 1.0 / mixed 0.5 / test 0.25
       × 1/(1 + 0.5·params)                  // params is anti-predictive
```

Cross-repo AUC 0.674 vs 0.619 for the pre-data design (+0.055). All terms reuse
existing `RefactorFamily` fields; **`mean_sem` is dropped** as a magnitude term.

## Honest limits

AUC ≈ 0.67 is a useful *ranking* signal, not a precise predictor — divergent-edit
propensity is inherently noisy from static features. The label is G1 (divergence
occurred), not G2 (divergence caused a bug); the score ranks divergence *propensity*,
the precursor to harmful divergence. 7 repos (X = thrift only); needs ≥12 repos, deeper
X stratum, G2 bug-linking, and a human-audited gold subset before final weights are
locked. Re-run: `tune.py all-events.jsonl`.
