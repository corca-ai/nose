# Markdown near-duplicate golden set

Frozen, committed evaluation artifact for same-language Markdown near-duplicate detection
(epic #435, step #436). Built with **no human in the loop**: an LLM judge panel labels the
duplication *relation*, self-calibrated against construction-truth anchors.

## Files

- `corpus/` — 30 real Contributor Covenant `CODE_OF_CONDUCT.md` files (flattened names), a
  frozen real-Markdown corpus. Stable so golden line-spans never rot.
- `golden.v1.json` — labeled pairs `{a, b, label, source, agreement}`. `label=true` ⇒ the two
  spans are a near-duplicate relation (`dup` or `near`); `false` ⇒ `not`.
- `golden.v1.meta.json` — panel-quality numbers (see below).
- `scripts/` — the deterministic build procedure (`sample_golden.py`, `aggregate_golden.py`).

## How it was built (no human labeling)

1. `nose markdown bench/markdown/corpus --dump-pairs` → scored candidate pairs (with text).
2. `sample_golden.py` → deterministic stratified sample (~135 pairs across score bands) +
   construction-identical anchors (normalized-identical pairs → certain positives).
3. **3 heterogeneous LLM judges** (opus / sonnet / haiku — distinct models to decorrelate bias)
   label each pair's *relation* (`dup` / `near` / `not`) per a fixed rubric. The rubric labels
   the relation only — **never** whether a repetition is intentional or worth removing
   (judgement-deep; out of scope per #435). Boilerplate copies are true duplicates.
4. `aggregate_golden.py` → majority vote (binary positive = `dup|near`), Fleiss κ, anchor
   self-calibration; anchors override to certain-positive.

## Panel quality (golden.v1.meta.json)

- **Fleiss κ = 0.702** (≥ 0.70 target — substantial inter-judge agreement).
- **Anchor self-calibration = 1.0** — every construction-identical pair was labeled positive by
  the panel majority (the no-human trust signal: the panel agrees with certain truth).
- 135 pairs (84 positive / 51 negative); 106 unanimous, 29 split (2-1).

## Detector measurement (against this golden)

`nose markdown bench/markdown/corpus --eval bench/markdown/golden.v1.json`:

- **PR-AUC 0.995**, ROC-AUC 0.992 (PR-AUC is primary — robust under near-dup class imbalance).
- **Recall@P95 0.96**, Recall@P99 0.93.
- **Candidate-recall 1.0** — Stage-1 surfaces every golden positive (no candidate-gen misses).
- Byte-identical across runs (determinism gate).

Caveat (honest scope): this golden's corpus is boilerplate-heavy (CoC family), so its negatives
are hard topical/boilerplate-overlap pairs — a realistic precision test, but not the full
multi-domain corpus distribution. Broader-corpus golden is a measured follow-up.
