# Markdown near-duplicate golden sets

Frozen, committed evaluation artifacts for same-language Markdown near-duplicate detection
(epic #435, step #436; broadened in #443). Built with **no human in the loop**: an LLM judge
panel labels the duplication *relation*, self-calibrated against construction-truth anchors.

There are **two golden sets** (precision) plus a **synthetic recall benchmark**:

| set | corpus | genre | what it stresses |
|---|---|---|---|
| `golden.v1` | `corpus/` (30 Contributor Covenant CoC files) | single-genre boilerplate | hard topical/boilerplate-overlap precision |
| `golden.docs.v1` | `corpus-docs/` (165 files, 5 genres) | **multi-domain** | realistic precision across CLI/API/guide/framework/README docs |
| `synth` (recall) | embedded `synth_base.md` | distinct prose | recall vs edit ratio (see `crates/nose-markdown/src/synth.rs`) |

`corpus-docs/` spans: CLI reference (curl options), function/API reference (hugo functions),
guides + news (jekyll), framework docs (prettier, trpc), and cross-repo README boilerplate —
so its negatives include genuinely hard *templated-but-different* pairs (e.g. two different CLI
options that share a skeleton), not just one boilerplate family.

## Files

- `corpus/`, `corpus-docs/` — frozen real-Markdown corpora (flattened names; stable so golden
  line-spans never rot).
- `golden.v1.json`, `golden.docs.v1.json` — labeled pairs `{a, b, label, source, agreement}`.
  `label=true` ⇒ the two spans are a near-duplicate relation (`dup` or `near`); `false` ⇒ `not`.
- `golden.v1.meta.json`, `golden.docs.v1.meta.json` — panel-quality numbers.
- `scripts/` — the deterministic build procedure: `setup_docs_corpus.sh` (curate `corpus-docs/`
  from `bench/repos`), `sample_golden.py` (stratified sample + anchors), `aggregate_golden.py`
  (majority vote + Fleiss κ + anchor calibration). All take paths, so any corpus can be golden'd.

## How a golden is built (no human labeling)

1. `nose markdown <corpus> --dump-pairs` → scored candidate pairs (with text).
2. `sample_golden.py <pairs> <sample> <anchors>` → deterministic stratified sample across score
   bands + construction-identical anchors (normalized-identical pairs → certain positives).
3. **3 heterogeneous LLM judges** (opus / sonnet / haiku — distinct models to decorrelate bias)
   label each pair's *relation* (`dup` / `near` / `not`) per a fixed rubric. The rubric labels the
   relation only — **never** whether a repetition is intentional or worth removing (judgement-deep;
   out of scope per #435). Boilerplate/templated *copies* are duplicates; shared *scaffolding* with
   different content is `not`.
4. `aggregate_golden.py …` → majority vote (binary positive = `dup|near`), Fleiss κ, anchor
   self-calibration; anchors override to certain-positive.

## Panel quality

| | golden.v1 (CoC) | golden.docs.v1 (multi-domain) |
|---|---|---|
| pairs (pos / neg) | 135 (84 / 51) | 129 (38 / 91) |
| Fleiss κ | 0.702 | 0.710 |
| anchor self-calibration | 1.0 | 1.0 |
| unanimous / split | 106 / 29 | 105 / 24 |

Both clear the κ ≥ 0.70 bar and label every construction-identical anchor positive (the no-human
trust signal). The multi-domain set is genuinely harder to label (more `not`, more nuanced `near`).

## Detector measurement

`nose markdown <corpus> --eval <golden>`:

| | golden.v1 (CoC) | golden.docs.v1 (multi-domain) |
|---|---|---|
| PR-AUC (primary) | 0.995 | **0.944** |
| ROC-AUC | 0.992 | 0.970 |
| Recall@P95 | 0.96 | **0.737** |
| Recall@P99 | 0.93 | 0.632 |
| candidate-recall | 1.0 | 1.0 |

**The multi-domain golden is the honest baseline.** The CoC-only golden *over-stated* precision
(0.995) because all its negatives are one boilerplate family; on real multi-domain docs the
templated-but-different pairs are harder, and precision is meaningfully lower (PR-AUC 0.944,
R@P95 0.74). Candidate-recall stays 1.0 (Stage-1 misses nothing). `golden.docs.v1` is wired into
a precision regression gate (`eval::docs_golden_precision_floor`), and `synth` gates recall.
Deterministic; byte-identical across runs.
