# Soundness Lab baseline

The Soundness Lab is the offline release gate for widening nose's exact semantic
claim without weakening its defining invariant:

> equal exact fingerprint implies equal behavior.

The 0.19.0 baseline is frozen under
[`bench/soundness/0.19.0`](../bench/soundness/0.19.0). Later oracle support,
adversarial batteries, and proof work may satisfy more of this cohort, but may
not add easy fixtures to change its numerator or denominator.

## 0.20 domain-aware falsification boundary

The 0.20 offline gate supplements the unchanged global battery with a per-group,
declared-domain-aware search. It compares exact parameter-domain vectors rather than trusting
their compact report hash, excludes symbolic behavior from the hard lane, prioritizes known
law boundaries, and emits deterministic seed/case/shrunk-input receipts. This keeps broad input
generation away from the core-vs-canonical global battery, where type-incoherent rows would be
misleading. A missing domain is treated as runtime `Any` only for dynamically typed languages;
missing static evidence and domains without a faithful interpreter representation (including
set, byte-array, iterator, map, record, result, and future-like values) fail closed to advisory.
So do width-erased static integers/floats and element/payload-erased arrays, collections,
iterables, and options. Dynamic scalar domains, TypeScript `number`, boolean, and string retain
concrete pools; parameterized domains reopen only when their exact constraints are preserved.

The independent [source-runtime calibration](../bench/soundness/0.20.0/source-runtime-calibration.v1.json)
is checked with:

```sh
python3 scripts/check-domain-calibration.py
python3 scripts/check-domain-calibration.py --self-test
```

Both commands run in the required GitHub `build · test · lint · dup-gate` job as well as the
local fast gate.

It executes Python and Node directly for string order, float association, JS-vs-Python integer
width, mutation coordinates, signed zero, and NaN. The script self-test mutation-checks the
independent comparison boundary. The Rust `domain_runtime_calibration` integration tests then
lower real source fixtures through the production frontend, normalize and interpret them, and
compare both internal channels with the checked runtime artifact. A second integration test
injects the same wrong float fact into both actual-channel receipts and requires rejection, so
frontend/interpreter agreement cannot overrule the source runtimes. The calibration code and
artifact remain offline and are not linked into the shipped `nose` binary.

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

## Exclusion attribution and claimable mass

The v2 [exclusion ledger](../bench/soundness/0.19.0/exclusion-ledger.v2.json)
turns all 6,713 frozen exclusions into source-bound evidence. Every row records
the exact release source span and hashes, product exact-claim eligibility,
obligation, first unsupported capability, and a leaf-first blocker stack. There
are no generic or unattributed rows. The original classification is unchanged:
6,054 missing-support units, 652 attributed semantic boundaries, five path-cap
exclusions, one cost exclusion, and one empty fingerprint. All 652 semantic
boundaries remain explicitly closed and non-claimable.

The checked [raw exclusion
census](../bench/soundness/0.19.0/exclusion-census.v2.json) binds every ledger
attribution and blocker stack back to the reporting run. Baseline freezing now
requires both the 1-thread and 4-thread census/report peers and rejects either
pair unless it is byte-identical.

Census locations include line and byte spans. Line ranges alone are not unique:
minified JavaScript in the pinned corpus contains hundreds of distinct function
units on one line. Exact-safety lookup uses the same byte-span key, preventing a
safe and unsafe function on that line from overwriting each other's eligibility.

Most baseline failures reach one of two IL gaps: missing variable identity
(`4,596`) or an unsupported expression node (`1,553`). Raw frequency is not the
implementation order, however. The [claimable-mass
census](../bench/soundness/0.19.0/claimable-mass-census.v2.json) recomputes
fingerprint families within each of the 120 pinned repositories and admits only
units that survive the product's default extraction gates (including
declaration-only, data-like-function, and large-test-file rejection) and then
pass its exact-safety and fingerprint-size gate. The
separate [interpreter
priority](../bench/soundness/0.19.0/interpreter-priority.v2.json) then caps each
family at eight pairs and multi-attributes its remaining
unverified mass by language, obligation, leaf construct, and first capability.
Every such row is Tier A because it represents an already product-claimable
merge family with an oracle-excluded member.

The full pinned corpus contains 639,516 function units: 197,369 interpretable
and 442,147 fail-closed. Eligibility reduces the actionable surface to 659
claimable families carrying 2,522 unverified pairs, or 1,256 pairs after the
per-family cap. The first two investment cells are:

| language / leaf construct / capability | families | raw / capped pair mass |
| --- | ---: | ---: |
| Java / `kind:Var` / `il.variable-identity-missing` | 105 | `696 / 220` |
| Python / `kind:Assign` / `protocol.field-write-proof` | 68 | `319 / 147` |

This makes the next decision concrete: close Java variable identity first for
the broadest capped reach, then Python field-write proof for the next-largest
capped reach. Each implementation must still pass the frozen soundness gate;
the ranking is value evidence, not permission to widen admission.

The checked policy tests inject both a 100-unit exact-unsafe fingerprint cluster
and a 100-unit product-ineligible large-test cluster, and require the priority
artifact to remain byte-equivalent. Thus generated, lossy, or non-product mass
may remain visible in exclusion totals but cannot steer interpreter investment.

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

To reproduce the v2 attribution artifacts, build the current reporting
instrument, but point it at a separate clean release worktree. The commands
below create every referenced path, run both thread counts, and make the freeze
command verify both byte-identical peers. The clean worktree is input only; the
instrument binary always comes from the current checkout.

```sh
repo_root="$(pwd)"
scratch="$repo_root/target/soundness-lab"
clean_root="$scratch/clean-v0.19.0"
outputs="$scratch/exclusions"
mkdir -p "$outputs"

git worktree add --detach "$clean_root" v0.19.0
cargo build --release -p nose-cli

(
  cd "$clean_root"
  RAYON_NUM_THREADS=1 "$repo_root/target/release/nose" verify crates \
    --max-violations 0 \
    --exclusion-census "$outputs/crates-t1.json" \
    --recall-loss-report "$outputs/recall-t1.json"
  RAYON_NUM_THREADS=4 "$repo_root/target/release/nose" verify crates \
    --max-violations 0 \
    --exclusion-census "$outputs/crates-t4.json" \
    --recall-loss-report "$outputs/recall-t4.json"
)

cmp "$outputs/crates-t1.json" "$outputs/crates-t4.json"
cmp "$outputs/recall-t1.json" "$outputs/recall-t4.json"
test "$(python3 -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())' "$outputs/recall-t1.json")" = \
  149abb80c7ffb790d3fc0fbc2ad910add776d768ecf2961ee4321239f935d9c9

python3 scripts/collect-soundness-census.py \
  --nose target/release/nose \
  --output "$scratch/corpus-exclusions"
python3 scripts/soundness_exclusions.py --freeze-baseline \
  --census "$outputs/crates-t1.json" \
  --census-peer "$outputs/crates-t4.json" \
  --report "$outputs/recall-t1.json" \
  --report-peer "$outputs/recall-t4.json" \
  --source-root "$clean_root"
python3 scripts/soundness_exclusions.py --freeze-corpus \
  --raw-dir "$scratch/corpus-exclusions/raw" \
  --evidence "$scratch/corpus-exclusions/evidence.json"
python3 scripts/soundness_exclusions.py --self-test
python3 scripts/soundness_exclusions.py \
  --raw-dir "$scratch/corpus-exclusions/raw" \
  --evidence "$scratch/corpus-exclusions/evidence.json"
```

The collector refuses changed repository pins, verifies the prune digest before
and after the run, fixes interpreter threads to one inside each repository, and
binds every raw census hash to the exact binary and corpus identities. The
baseline freeze refuses missing or byte-different 1/4-thread peers. The checker
independently rebuilds pair mass and priority from product-admitted unit or
family rows; stored aggregate counters are never accepted on trust. The
checked per-repository commitments catch derived-artifact drift without the
1.3 GB raw corpus. A source-completeness claim additionally requires the
explicit `--raw-dir` replay shown above; the validator says so when only
commitments were available.
