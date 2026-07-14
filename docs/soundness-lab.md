# Soundness Lab baseline

The Soundness Lab is the offline release gate for widening nose's exact semantic
claim without weakening its defining invariant:

> equal exact fingerprint implies equal behavior.

The 0.19.0 baseline is frozen under
[`bench/soundness/0.19.0`](../bench/soundness/0.19.0). Later oracle support,
adversarial batteries, and proof work may satisfy more of this cohort, but may
not add easy fixtures to change its numerator or denominator.

## Two binary identities, one release-tree report

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
binary identity, but replaying it on the exact v0.19.0 `f57b078` `crates` tree
produces the byte-identical report above. A previously observed
`7,869 / 1,128 / 6,741` result belongs to a later, post-release `crates` tree
and is not baseline evidence. The
[`manifest.v1.json` evidence manifest](../bench/soundness/0.19.0/manifest.v1.json) records both binary hashes, the shared tree, and the shared report hash.

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

The instrumented binary is built in a patched worktree but analyzes a separate,
clean v0.19.0 source tree. The checker compares every recorded source span with
that Git commit, so reporting-code line shifts cannot be mislabeled as release
source. The 1-thread and 4-thread raw artifacts are byte-identical. The checked
[`cohort.v1.json`](../bench/soundness/0.19.0/cohort.v1.json) contains 1,122
interpretable units, 193 claimable units, and 117 individually identified canon
exposures.

## Non-gameable score

The published 0.19 semantic query contributes 215 exact pairs. A fixed cap of
eight pairs per product family leaves 97 baseline pairs in eight cells keyed by
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
| risk/prevalence-weighted macro coverage | `23.33%` |
| 0.20 target | `42.50%` |

The target is `C0 + max(10 percentage points, 25% of the remaining gap)`. The
scorecard also reports each language independently; aggregate gain cannot make
an uncovered Tier-A cell disappear. A future exact claim must add its required
cells; it may not reuse the frozen denominator to avoid new proof work.

## Reproduce and validate

Use the published binary, the exact release source tree, and all pinned
repositories. The expected binary SHA-256 is intentionally supplied to the
corpus runner rather than inferred from its filename:

```sh
mkdir -p target/soundness-lab/v0.19.0
git worktree add --detach target/soundness-lab/source-v0.19.0 v0.19.0

repo_root="$(pwd)"
published_nose="/absolute/path/to/published-v0.19.0-nose"
(
  cd target/soundness-lab/source-v0.19.0
  RAYON_NUM_THREADS=1 "$published_nose" verify crates \
    --max-violations 0 \
    --recall-loss-report "$repo_root/target/soundness-lab/crates.json"
)

RAYON_NUM_THREADS=1 ./scripts/corpus-verify-nightly.sh \
  --nose "$published_nose" \
  --expected-nose-sha256 0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3 \
  --repos-root bench/repos \
  --logs-dir target/soundness-lab/v0.19.0

python3 scripts/check-soundness-scorecard.py --self-test
python3 scripts/check-soundness-scorecard.py \
  --baseline bench/soundness/0.19.0 \
  --reproduce target/soundness-lab/v0.19.0
```

The runner checks each repository HEAD, validates the checked pruned-corpus
digest, and writes `evidence.json` binding those identities and `summary.tsv`
to the actual binary hash. Missing repositories, changed pins or source bytes,
the wrong binary, hard false merges, canon violations, artifact drift, or a
non-release-tree report fail the check. Timing is omitted from the canonical
120-repository result; statuses, hard counts, and advisory counts are retained.
The measured 1-thread and 4-thread corpus results have the same canonical hash,
zero hard failures, and 4,756 advisory disagreements.

To audit the historical per-unit instrument from scratch, create one detached
v0.19.0 worktree for building, apply `reporting.patch`, and build `nose-cli`.
Run that binary against a second, clean v0.19.0 worktree with `verify --json
crates` at `RAYON_NUM_THREADS=1` and `4`. Then use the checker's `--freeze
--units ... --query ... --source-root <clean-worktree>` mode in a scratch
baseline directory; never overwrite the checked official baseline during an
experiment.
