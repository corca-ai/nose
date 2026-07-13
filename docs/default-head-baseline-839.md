# Default-head baseline for 0.20

Issue [#839](https://github.com/corca-ai/nose/issues/839) freezes the product
quality baseline before 0.20 behavior changes. Precision and recall deliberately
use different query universes:

- precision@10 uses the first ten `surface=default` families in the default list;
- the literal bare dashboard is checked as the displayed prefix of that list;
- worthy recall searches the explicit `all` universe, so hidden, shallow,
  generated, divergent, and declaration candidates remain measurable;
- `--precision-surface all` retains the pre-#839 full-universe precision metric
  as an explicit compatibility mode.

The evaluator obtains the complete `all` response once, derives the raw default
precision order from it, and requires a default-list query to return the same IDs
in the same order before metric ranking. It then executes a literal selector-free
bare query and requires the dashboard IDs to be that list's prefix. Any
non-default family in either product response, missing repository, corpus revision
mismatch, ID mismatch, prefix mismatch, or order mismatch fails the run.

## Published release identity

The baseline is the published `v0.19.0` Darwin arm64 release asset, not a local
rebuild or the release-candidate binary used by the earlier release closeout.

| Input | SHA-256 |
|---|---|
| release commit | `0985e6963c58d5a97e523bc532b88aa5e34f2ef9` |
| committed evaluator revision | `ed61a865af67d5b6b490e1c6be17190366d9ea6a` |
| `nose-cli-aarch64-apple-darwin.tar.xz` | `097c7e766e9ab756a32cec715897067d1360e145074715168a653962be409981` |
| published `.sha256` asset | `f860777bc74bfe18b9be76d02cb1b53e4ea0c8db206ecdcfdc4f16a5f8af5274` |
| extracted `nose 0.19.0` binary | `0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3` |
| v6 composite labelset | `6b72927d0e68e05406540016d3fa136029c52a406af0938b5a805d3fa199ac23` |
| corpus manifest | `87b3defc02c87e53f5ce20d10b68afdbc7190a6db5d5bfdb6b655b305bbc7ba8` |
| pinned corpus commit digest | `366c977c096a91d50095253cce77a3ec8468d3147ecbd819353dc01196281083` |

GitHub's asset digest and the downloaded checksum file both verified the archive
hash. The checked [schema-v3 report](../bench/labels/product_quality_evaluation_v0_19_0_default_head_2026_07_13.v3.json)
records the complete command, configuration, component hashes, repository
commits, evaluator-source hashes, release archive/checksum identity,
per-repository surface counts, denominators, and bootstrap intervals.

## Result

All 120 pinned repositories passed both raw default-list ID-and-order parity and
literal bare-dashboard prefix parity. Across the explicit full universe the
published binary reported 53,990 default, 14,989 hidden, 30,795 shallow, 973
generated, 88 divergence, and 131 declaration families.

| Split | Repositories | Default-surface labeled P@10 | Top-10 label coverage | All-surface worthy recall |
|---|---:|---:|---:|---:|
| dev | 66 | `271/437 = 62.0137%` | `437/658 = 66.4134%` | `2716/2849 = 95.3317%` |
| held-out | 54 | `222/375 = 59.2000%` | `375/538 = 69.7026%` | `2005/2091 = 95.8871%` |

Two identical full evaluations from the committed evaluator produced
byte-identical reports. The evaluator uses a fixed bootstrap seed and
deterministic repository ordering; the equality check covered the complete
report, not only the point estimates. CI checks the archived report's sidecar
SHA-256, `c73d148d3307673e4adaedc6a1e81474e7abd2c48c98ea02fcda9ddb3f6143dd`,
and its source/input contracts.

The held-out result was computed mechanically from the frozen labels. No
held-out source was opened or used to choose behavior.

## Compatibility metric

Run with `--precision-surface all` to retain the historical full-universe
precision definition. On the published asset this gives dev `264/444 =
59.4595%` and held-out `213/382 = 55.7592%`; recall is unchanged because it
already uses `all`.

The earlier 0.19 release closeout recorded held-out `213/381 = 55.9055%` while
reusing `target/release-0.19.0/final-eval-cache`. An uncached replay of both its
local release candidate and the published asset gives `213/382`: the difference
was stale cached analysis for one label-matched `asciidoctor` top-10 family, not a
binary behavior difference. Baseline evaluation therefore rejects `--cache-dir`
unless the caller makes an explicit diagnostic-only opt-in; the checked report is
uncached. Cached/uncached product drift is tracked separately from this
measurement baseline.

## Reproduction

After downloading and checksum-verifying the published asset:

```sh
python3 bench/labels/query_schema.py --self-test --nose <official-v0.19.0-nose>
python3 bench/labels/eval_by_language.py \
  --nose <official-v0.19.0-nose> \
  --nose-release-archive <official-v0.19.0-archive> \
  --nose-release-checksum <official-v0.19.0-archive.sha256> \
  --rank extractability \
  --bootstrap 2000 \
  --json-out <baseline-report>
```

Use the same command plus `--precision-surface all` only when comparing with the
old metric universe. Do not pass `--cache-dir` for a baseline.
