# Soundness Lab baseline

The Soundness Lab is the offline release gate for widening nose's exact semantic
claim without weakening its defining invariant:

> equal exact fingerprint implies equal behavior.

The 0.19.0 baseline is frozen under
[`bench/soundness/0.19.0`](../bench/soundness/0.19.0). Later oracle support,
adversarial batteries, and proof work may satisfy more of this cohort, but may
not add easy fixtures to change its numerator or denominator.

## Two release records, kept separate

The preserved clean release-candidate binary and raw report reproduce the
historical release gate:

| Metric | Frozen value |
| --- | ---: |
| total / interpretable / excluded | `7,835 / 1,122 / 6,713` |
| canon checked / violations | `117 / 0` |
| fingerprint groups / false merges | `46 / 0` |
| completeness | `63/180 = 35.00%` |
| missing oracle support | `6,054` |
| semantic-boundary attributed | `652` |

The subsequently published GitHub v0.19.0 macOS arm64 asset has a different
binary identity. Replaying it on the same `crates` tree yields
`7,869 / 1,128 / 6,741`, with the same 117 canon checks, zero hard violations,
and 63/180 completeness. This discrepancy is not normalized away: the
historical report remains the frozen denominator, while the published asset is
the required deployment replay. The
[`manifest.v1.json` evidence manifest](../bench/soundness/0.19.0/manifest.v1.json) records both hashes and both metric records.

## Stable cohort identity

The reporting-only [`reporting.patch`](../bench/soundness/0.19.0/reporting.patch)
adds no admission or normalization behavior. It exposes the data already used
by `nose verify --json`: exact-claim eligibility, domain signature, value
fingerprint, behavior class, construct tags, and per-unit canon exposure.

Each frozen unit is identified by SHA-256 over a version tag and NUL-separated:

```text
repository pin
relative path and exact source span
SHA-256 of the exact source lines
SHA-256 of the canonical core-IL value fingerprint
claim id
```

The 1-thread and 4-thread raw artifacts are byte-identical. The checked
[`cohort.v1.json`](../bench/soundness/0.19.0/cohort.v1.json) contains 1,122
interpretable units, 193 claimable units, and 117 individually identified canon
exposures.

## Non-gameable score

The published 0.19 semantic query contributes 215 exact pairs. A fixed cap of
eight pairs per product family leaves 97 baseline pairs in nine cells keyed by
claim, obligation, language, and construct family. Cells have fixed Tier A/B/C
risk weights and a capped log-scaled prevalence weight.

Only a pair whose two sides are interpretable, exact-claim eligible,
non-symbolic, in the same declared domain, and behavior-equal enters the
verified numerator. Every other pair remains visible as exact-unsafe mass; it
can neither inflate the numerator nor disappear from the denominator.

The frozen result is:

| Metric | 0.19.0 |
| --- | ---: |
| verified / capped baseline pairs | `17 / 97` |
| exact-unsafe capped pairs | `80` |
| pair-micro coverage | `17.53%` |
| risk/prevalence-weighted macro coverage | `22.00%` |
| 0.20 target | `41.50%` |

The target is `C0 + max(10 percentage points, 25% of the remaining gap)`. The
scorecard also reports each language independently; aggregate gain cannot make
an uncovered Tier-A cell disappear. A future exact claim must add its required
cells; it may not reuse the frozen denominator to avoid new proof work.

## Reproduce and validate

Use the published binary for the deployment replay and all pinned repositories:

```sh
mkdir -p target/soundness-lab/v0.19.0

RAYON_NUM_THREADS=1 <published-v0.19.0-nose> verify crates \
  --max-violations 0 \
  --recall-loss-report target/soundness-lab/crates.json

RAYON_NUM_THREADS=1 ./scripts/corpus-verify-nightly.sh \
  --nose <published-v0.19.0-nose> \
  --repos-root bench/repos \
  --logs-dir target/soundness-lab/v0.19.0

python3 scripts/check-soundness-scorecard.py --self-test
python3 scripts/check-soundness-scorecard.py \
  --baseline bench/soundness/0.19.0 \
  --reproduce target/soundness-lab/v0.19.0
```

Missing repositories, changed pins, hard false merges, canon violations,
artifact drift, or a report matching neither recorded release identity fail the
check. Timing is omitted from the canonical 120-repository result; statuses,
hard counts, and advisory counts are retained. The measured 1-thread and
4-thread corpus results have the same canonical hash, zero hard failures, and
4,756 advisory disagreements.

To audit the historical per-unit instrument from scratch, create a detached
v0.19.0 worktree, apply `reporting.patch`, build `nose-cli`, and run `verify
--json crates` at `RAYON_NUM_THREADS=1` and `4`. Then use the checker's
`--freeze --units ... --query ... --source-root ...` mode in a scratch baseline
directory; never overwrite the checked official baseline during an experiment.
