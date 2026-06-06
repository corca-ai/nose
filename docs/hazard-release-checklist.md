# Releasing a new nose version — hazard-ranking checklist

> **One page, everything in one place:** what to do for the [hazard ranking](hazard-ranking.md)
> every time you cut a new nose version. The general release process (version bump,
> [`CHANGELOG`](../CHANGELOG.md), [`CONTRIBUTING`](../CONTRIBUTING.md)) is separate; this
> page is the hazard-specific obligation that is easy to forget.

**Why this exists.** `hazard()`'s weights are *calibrated against mined data whose
features (`mean_lines`, `modules`, `mean_sem`, `params`, …) are produced by nose*. A
change to detection can silently invalidate those weights. The **labels** (G0/G1/G2) come
from git history and are version-independent — see
[hazard-benchmark › Versioning and refresh](hazard-benchmark.md#versioning-and-refresh)
for the full coupling model. This checklist makes the re-calibration step impossible to
miss.

> Status: `hazard()` is implemented as opt-in `--sort hazard` and calibrated as a
> divergence-propensity signal ([eval/hazard/RESULTS.md](../eval/hazard/RESULTS.md)).
> It is **not** the default and is **not** a validated harm ranker; see
> [hazard-ranking](hazard-ranking.md).

## TL;DR — does this release change detection *output*?

| What changed in the release | Re-mine dataset? | Re-tune `hazard()`? | Action |
|---|---|---|---|
| **Detection output** — family identity / member sets / fingerprints, or any feature value (`mean_sem`, `shared_weight`, `params`, `mean_lines`, `modules`, `scope`) | **Yes** | **Yes** | Run the [refresh](#the-refresh-procedure) |
| **New language or detection channel** | **Yes** | **Yes** | Refresh **and** add corpus repos in that language (see [corpus policy](hazard-benchmark.md#corpus-policy)) |
| **Ranking only** — `extractability`, `hazard()` itself, sort keys, output format | No | No | Nothing — the dataset is built from detection output + git, never from ranking |
| **Performance / refactor with identical output** | No | No | Nothing (confirm output is byte-identical first) |

**How to tell if detection output changed**, if unsure: scan a fixed fixture corpus with
the old and new binary and diff the JSON `families` (`nose scan <fixtures> --mode
semantic,near --format json --top 0`). No diff → ranking-only, skip. Any diff → refresh.
When in doubt, refresh — it costs minutes (cached clones).

## The refresh procedure

Tooling lives in [`eval/hazard/`](../eval/hazard/); methodology in
[hazard-benchmark](hazard-benchmark.md).

```sh
# 1. Build the new detector
cargo build --release

# 2. Point the tooling at it (clones are cached in $WORK from prior runs)
export NOSE="$PWD/target/release/nose"
export WORK=/tmp/hazard-mine

# 3. Re-mine: re-runs nose across the cached snapshots -> fresh features + labels,
#    each event stamped with the new `nose_ver`. ~minutes.
bash eval/hazard/run_corpus.sh

# 4. Re-tune: leave-one-repo-out logistic weights + candidate-formula AUC
python3 eval/hazard/tune.py "$WORK/all-events.jsonl"
```

## Reading the result — does the formula still hold?

Compare `tune.py` output against the previous `nose_ver` recorded in
[RESULTS.md](../eval/hazard/RESULTS.md):

- **Weights stable** (same signs, similar relative magnitudes; best candidate-formula AUC
  unchanged) → the formula still holds. Just bump the `nose_ver` line in `RESULTS.md`.
- **Weights drift** (a sign flips, or a different candidate formula now wins) → **re-calibrate**:
  1. Pick the new best candidate formula (or add one) in `tune.py`.
  2. Update the formula constants in `crates/nose-detect/src/report.rs` (`hazard()`).
  3. Update [hazard-ranking › Score design](hazard-ranking.md#score-design), `RESULTS.md`,
     and add an [experiments](experiments.md) entry (next `§` letter).

## Acceptance — the hazard part of the release is done when

- [ ] If detection changed: dataset regenerated, every event carries the new `nose_ver`.
- [ ] `tune.py` run; weights compared to the previous version; formula updated iff drifted.
- [ ] `report.rs` `hazard()` matches the formula in [Score design](hazard-ranking.md#score-design).
- [ ] [RESULTS.md](../eval/hazard/RESULTS.md) reflects the current `nose_ver` and numbers.
- [ ] An [experiments](experiments.md) entry exists if the formula changed.
- [ ] Tier-0 contract unit tests pass (see [evaluation tiers](hazard-ranking.md#evaluating-ranking-quality)).

## Where everything lives

| Thing | Location |
|---|---|
| The score (`hazard()`, `SortKey::Hazard`) | `crates/nose-detect/src/report.rs`, `crates/nose-cli/src/main.rs` |
| The formula + evidence | [hazard-ranking › Score design](hazard-ranking.md#score-design) |
| Evaluation criteria, dataset, versioning model | [hazard-benchmark](hazard-benchmark.md) |
| Mining + tuning tooling | [`eval/hazard/`](../eval/hazard/) (`mine.py`, `run_corpus.sh`, `analyze.py`, `tune.py`) |
| Current measured numbers | [`eval/hazard/RESULTS.md`](../eval/hazard/RESULTS.md) |
| The measured log | [experiments › §BG](experiments.md) |
| Cached corpus clones + raw events | `/tmp/hazard-mine/` (not committed) |

## Future automation

- **CI guard:** run `tune.py` on each release and flag if any learned weight direction
  flips or the headline AUC moves beyond a threshold — turns "remember to refresh" into a
  failing check.
- **Change-fact cache:** persist the nose-independent per-`(file, symbol, interval)` git
  change facts so a refresh only re-runs the nose scan + join, not the diffs.

## See also

- [hazard-ranking](hazard-ranking.md) — the score, its evidence base, and implementation plan.
- [hazard-benchmark](hazard-benchmark.md) — the evaluation criteria, dataset, and the full
  versioning/coupling model this checklist operationalizes.
- [experiments](experiments.md) — the measured log of calibration runs.
