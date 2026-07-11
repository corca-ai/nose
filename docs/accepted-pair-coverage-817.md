# Accepted-pair endpoint coverage (#817)

Issue #817 fixes a post-acceptance loss: a direct structural pair could pass
candidate scoring, enter a union-find component, and still disappear because rank
subsumption and query opportunity folding retained families that covered neither
accepted endpoint. The change preserves pair-local evidence without changing
candidate thresholds, accepted-pair semantics, ranking order, or default-surface
classification.

This page records the dev result, the held-out gate frozen before measurement,
and the later one-time confirmation. See the machine-readable [`accepted_pair_coverage_dev_gate_2026_07_11.v1.json` chronology](../bench/labels/accepted_pair_coverage_dev_gate_2026_07_11.v1.json).

## Reproduced defect

The focused real case is the exact `sqlEvalFunc` copy at
`sqlite/ext/misc/eval.c:71-102` and `sqlite/tool/fuzzershell.c:332-363`.
Near-only query output retained its family, while adding the syntax channel hid it:

1. a larger syntax window subsumed the accepted structural family during rank
   deduplication;
2. that syntax window partially overlapped an earlier callback family and folded
   under it;
3. transitive union-find rooting treated the callback as the final primary even
   though it covered neither `sqlEvalFunc` endpoint.

The failing regression fixture was committed before the behavior change. Synthetic
fixtures isolate the same law for a same-file pair, a large region, a parameterized
test shape, and an A-B/B-C bridge.

## Bounded implementation

Detection now retains a compact graph for each accepted structural group: sites are
stored once and direct edges use local integer indices. Same-file site collapse maps
those edges through the same location mapping used by the family. Rank subsumption
moves the obligation to its covering survivor. Query folding then builds a direct
suppression forest rather than treating overlap as a transitive equivalence relation.

An accepted carrier folds only when one visible family covers both endpoints of
every direct accepted edge. Existing roots are checked by edge pair, not by the
union of independently covered sites. Syntax-only windows keep the previous folding
policy and direct suppression navigation.

This representation is bounded:

- no accepted pair is emitted as a separate product row;
- dense opportunity buckets above the existing 200-family cap skip quadratic
  folding;
- accepted sites are stored once per graph rather than copied per edge;
- internal edge graphs are dropped after the query fold forest is built and never
  enter query JSON.

The rejected alternative was global site-wise coverage. It made Guava lose 7,849
accepted edges because two different roots could cover the two endpoints separately;
that does not prove that either root represents the pair. The implementation therefore
uses exact pair incidence throughout.

## Full dev accepted-edge census

The diagnostic `nose detect --query-accepted` uses the product query's structural
channels and emits accepted pairs only for the census. The collector runs all 66 dev
repositories, verifies every pinned commit, records a digest over the full edge set,
and retains only bounded per-repository samples.

| dev accepted-edge state | baseline | head |
| --- | ---: | ---: |
| accepted edges | 1,222,948 | 1,222,948 |
| eligible distinct, non-nested edges | 1,222,904 | 1,222,904 |
| final all-surface covered | 798,973 | 1,222,856 |
| recovered / regressed | — | 423,883 / 0 |
| final default covered | 140,495 | 660,055 |
| visible families added / removed | — | 2,377 / 0 |
| added families without a recovered edge | — | 0 |

The remaining 48 eligible edges are explicit generated/vendor suppressions, not
unexplained folding loss: 35 in Hugo's vendored `parson.c`, seven in Prettier's build
script output, and six in Raylib external dependency paths. The 44 ineligible edges
collapse to the same or nested source site.

Authoritative artifacts:

- Use the [baseline census](../bench/labels/accepted_pair_coverage_2026_07_11.dev.baseline.v2.json) for the frozen loss state.
- Use the [head census and exact comparison](../bench/labels/accepted_pair_coverage_2026_07_11.dev.head.v2.json) for recovery accounting.
- Use the [collector and validator](../bench/labels/accepted_pair_coverage.py) to reproduce either role.

## Dev product quality

The checked v6 evaluator was run on dev only, with the frozen baseline binary passed
through `--comparison-nose`. The report therefore records every recovered and
regressed worthy label ID rather than inferring the delta from aggregate counts.

| metric | baseline | head | delta |
| --- | ---: | ---: | ---: |
| worthy recall | 2,626/2,849 (92.17%) | 2,691/2,849 (94.45%) | +65 |
| labeled P@10 | 259/437 (59.27%) | 264/447 (59.06%) | -0.21 pp |
| top-10 label-match coverage | 437/660 (66.21%) | 447/660 (67.73%) | +10 |

No worthy label regressed. Recall increased in C +26, Go +2, Java +9, Python +1,
Ruby +9, Rust +15, and TypeScript +3; Swift's recall denominator remains zero.

The exact 65 recovered `repo:family_id` rows are:

- `alacritty`: `4c6345613406b888`, `c5b0dc1d87506916`, `c93b1e63f96d2a38`
- `axios`: `70db87b821c19292`, `80e929ac1884dd5b`, `9c60235300651f6f`
- `bat`: `53a5234e49ea003a`
- `clap`: `67e4d3199dcb742a`, `8314f42e205f4857`
- `curl`: `07576398d3db15e6`, `1b0b0a2a78f4053e`, `438cb822fefd5174`, `688ec0f1c0146e0d`, `79d633f8b5dc6977`, `add4f3e341202511`, `af38d8d5c688322b`, `b6645e054f2f60ec`
- `fastlane`: `cfbff7878814e3a7`
- `git`: `895d513507a19cf6`, `d9d84da753e732ca`
- `graphhopper`: `fb83868d7d451012`
- `libsodium`: `37ebbd018de90717`, `442744c3d62eaa4a`, `4edecc65d87fe574`, `79eeba5922837da4`, `863cbbeb598f2419`, `8d6552152465a969`, `d141aa6e57d27742`, `d8e13c5353153ccd`
- `mockito`: `67d372348d1d6446`, `9f967a3cfc10f3b8`, `faafc7cc04839b34`
- `netty`: `265776dc187e262c`, `5bd323713843fdad`, `aaffd3da87f0f758`, `de6611cbcf1671b4`
- `nginx`: `04895d9a2eeb6131`, `7ea165022a7e3302`, `7ee7007d5fc2a0d5`, `8fa530f40ce49c73`, `d153a4f3802a42c5`
- `nushell`: `06abb0a4b9c069e2`, `2637963422239bd3`, `5717ee835ee37e72`, `822da63ee38b1ab2`, `a5418a9b1b048516`, `d6fb645fa2e28fdf`
- `prometheus`: `00732ff2a6289f00`, `03e916354ac5c664`
- `retrofit`: `450a5844cce0c0bd`
- `rich`: `cd216bafb0dfec7c`
- `ripgrep`: `e6d81b48f46ce521`
- `rspec-core`: `122d3349380489f2`, `f1d562131fa8096a`, `f90e040392961f73`
- `rubocop`: `24160f04580c3762`, `490869ad2d30695d`, `9a3d18bd8a3ac7b6`
- `serde_json`: `5c3a440867f18f80`, `bec63f16f3ebd8fd`
- `sinatra`: `30191d2f42a72a70`, `6c0f20ce8939ef2f`
- `sqlite`: `409c1b9791d270e9`
- `tmux`: `3a515df5216b5259`, `a0458bd522fd4095`

The complete records, including language, channel, scope, and per-repository counts,
are in the [dev product-quality artifact](../bench/labels/product_quality_evaluation_issue_817_2026_07_11.dev.v2.json).

## Product output and runtime price

The #809 slice uses axios, curl, netty, nushell, prometheus, rich, and rubocop.
Semantic-only and actual CLI-default output were measured separately. Every output
change is declared exactly; no hidden, shallow, generated, declaration, or divergence
count changed.

| seven-repo output | baseline | head | delta |
| --- | ---: | ---: | ---: |
| semantic all families | 1,263 | 1,271 | +8 (+0.63%) |
| semantic default families | 415 | 423 | +8 (+1.93%) |
| CLI-default all families | 11,849 | 12,312 | +463 (+3.91%) |
| CLI-default default families | 6,200 | 6,663 | +463 (+7.47%) |

The measured default growth is below the frozen 10% dev budget. All 463 added rows
are default-surface accepted carriers; classification policy did not promote them.

The primary run used four alternating measurements after one warmup. A strict stage
signal requested focused reruns. The final focused reports use 20 balanced measurements
after two warmups and a head/head same-binary control, which measures execution-order
noise on the changed binary. Both gates pass:

| query | focused repositories | control-adjusted aggregate |
| --- | --- | ---: |
| semantic | curl, netty, prometheus | -2.11% |
| CLI default | curl, nushell, prometheus, rubocop | -4.54% |

Rendering initially exposed a real cost because every output family reread source and
reran N-way anti-unification. Shared-line weighting already computed the identical
all-copies counts, so query JSON now reuses them; byte hashes remained unchanged while
render cost fell materially. The fused normalize/extract path was also deduplicated,
and accepted tracing was moved outside the hot `DetectOptions` layout.

Checked evidence is adjacent under `bench/labels/accepted_pair_coverage_pricing_*`:
primary, same-binary control, focused, focused control, exact drift manifest, checker
status, and compact summary for both query modes.

## Hard negatives and remaining frontier

The test suite fixes these boundaries:

- A-B plus B-C does not manufacture A-C equivalence or one all-member skeleton;
- nested and overlapping same-file sites remain one refactoring location;
- a true covering family suppresses a redundant carrier;
- one outer member may cover multiple incident sites, but nonincident sites do not
  satisfy an edge;
- dense buckets remain capped and do not materialize all pair rows;
- generated, ignored, min-member, min-value, and surface suppressions still apply;
- every added default family accounts for at least one baseline-uncovered accepted edge.

This change addresses only post-acceptance loss. Candidate-only, extraction, same-unit
fragment, and no-coherent-mechanism cohorts from #816 remain separate. No threshold was
lowered and no new semantic law was admitted.

## Frozen held-out confirmation and next action

Before opening held-out, the dev gate was frozen as: at least 18 recovered worthy
families, zero worthy regressions, P@10 no worse than -1 percentage point, default
growth at most 10%, zero unexpected accepted-edge regressions, zero unaccounted added
families, and both runtime gates passing. Dev passes every condition.

Held-out was then run exactly once from clean commit `074f029b`, with no subsequent
grouping tune. The pre-registered pass rule was at least 15 recovered worthy labels
across at least three languages, no regression, and P@10 within -2 percentage points
of the frozen baseline.

| held-out metric | baseline | head | delta |
| --- | ---: | ---: | ---: |
| worthy recall | 1,949/2,091 (93.21%) | 1,996/2,091 (95.46%) | +47 |
| labeled P@10 | 206/383 (53.79%) | 214/384 (55.73%) | +1.94 pp |
| label-match coverage | 383/540 (70.93%) | 384/540 (71.11%) | +1 |

No worthy label regressed. Recovery spans C +9, Go +2, Java +23, Python +2,
Rust +6, and TypeScript +5: 47 labels across six languages. The confirmation
therefore passes every frozen condition. The [held-out evaluation](../bench/labels/product_quality_evaluation_issue_817_2026_07_11.heldout.v1.json)
contains the exact recovered IDs, and the [confirmation record](../bench/labels/accepted_pair_coverage_heldout_confirmation_2026_07_11.v1.json)
binds it to the pre-held-out dev gate.

The selected next action is now the pass branch: merge this bounded fix, rerun the
missed-worthy frontier, and choose the next tranche only from the remaining misses. That
rerun is the [#820 post-#817 frontier](missed-worthy-frontier-820.md), which selects the
bounded connected-witness follow-up #821.

## Validation

```sh
python3 bench/labels/accepted_pair_coverage.py --self-test
python3 bench/labels/accepted_pair_coverage.py \
  --validate bench/labels/accepted_pair_coverage_2026_07_11.dev.baseline.v2.json
python3 bench/labels/accepted_pair_coverage.py \
  --validate bench/labels/accepted_pair_coverage_2026_07_11.dev.head.v2.json
python3 scripts/check-query-regression.py \
  bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.primary.v3.json \
  --same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.control.v3.json \
  --expected-drift-manifest bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.expected-drift.v1.json \
  --focused-report bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.focused.v3.json \
  --focused-same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.focused-control.v3.json \
  --require-same-binary-control --max-runtime-delta-pct 5 --min-runtime-delta-ms 5 \
  --check-status bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.status.v3.json \
  --check-markdown bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.summary.v3.md
```
