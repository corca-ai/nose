# Benchmark

How nose's quality is measured, and the headline numbers. The blow-by-blow log of
individual experiments is in [experiments](experiments.md); this page is the methodology.

There are two distinct questions, measured separately:

| question | how | data |
|---|---|---|
| **Product quality** — does the query surface rank *genuine* refactoring candidates first? | precision@10 + worthy-recall, per language, dev/held-out, bootstrap 95% CIs | the checked v6 composite refactoring-family labelset |
| **Soundness** — does an equal fingerprint really mean equal behavior? | an interpreter oracle on a battery of inputs (`nose verify`) + Lean proofs | the pinned corpus |

A third asset is the [Type-4 benchmark factory](type4-benchmark.md): an evidence-carrying
synthetic benchmark for exact semantic equivalence classes. It is separate from the product
labelset because Type-4 exactness asks whether two fragments compute the same thing under a
declared semantics, not whether a reported family is worth refactoring.

## Product quality — the refactoring-family labelset

The active gold set is the hash-checked
`bench/labels/refactoring_families.v6.json` composite: the byte-frozen v5 multi-source
pool (9,461 families) plus separate dev and held-out current-top-10 components (115
families). In total it has **9,576 worthy/not-worthy judgments over 120 pinned
repositories and eight languages**, including 45 labels from all 15 real Swift
repositories. Each judgment follows a three-persona LLM panel with explicit arbiter
resolution — see [bench/labels/README.md](../bench/labels/README.md) and the labeling
contract in [RUBRIC.md](../bench/labels/RUBRIC.md).

The corpus has a **dev / held-out** split (`bench/goldens/corpus.json`), so a change
has to generalize, not just fit the dev repositories; tune only on dev. The dev
component was committed before held-out judgment. The checked prune manifest protects
the complete v6 labelset and records the unchanged 120-repository post-prune digest.

The refresh labels are explicitly **precision-only**: they were selected from current
top-10 output, so adding them to worthy-recall would bias the recall denominator toward
what nose already sees. Worthy-recall continues to use only the source-independent v5
pool. The candidate artifact freezes commands, binary/source hashes, repository commits,
raw family hashes, deterministic selection, votes, and arbitration. The earlier
`swift_families.v1.json` remains a synthetic Type-4 bring-up golden outside this metric.

```sh
bench/setup_repos.sh                      # clone the pinned corpus into bench/repos
python3 bench/prune_corpus.py --check-manifest  # verify the recorded prune digest
python3 bench/labels/query_schema.py --self-test --nose <official-v0.19.0-nose>
python3 bench/labels/default_head_query_schema.py \
  --self-test --nose <official-v0.19.0-nose>
python3 bench/labels/eval_by_language.py --nose <official-v0.19.0-nose> \
  --nose-release-archive <official-v0.19.0-archive> \
  --nose-release-checksum <official-v0.19.0-archive.sha256> \
  --rank extractability --bootstrap 2000 \
  --json-out bench/labels/product_quality_evaluation_v0_19_0_default_head_2026_07_13.v3.json
```

`--rank value` reproduces the historical volume order; `--rank extractability` keeps the
native `nose query --format json` order used by the current product. Both reports also show
the historical anti-unification re-rank comparison. Query output passes through a strict
schema-v7 adapter: a changed envelope, location key, surface, or scope fails with a
path-specific error instead of silently dropping a result.

**Current product baseline (2026-07-13, published nose 0.19.0):** precision@10
uses the default surface users see, while worthy recall searches the explicit
full `all` universe. On every repository the evaluator proves that the raw
default list has the same IDs and order as the default families derived from
`all`, and that the literal bare dashboard is the list's displayed prefix.

| split | repos | default-surface labeled precision@10 | matched top-10 | all-surface worthy recall |
|---|---:|---:|---:|---:|
| dev | 66 | 271/437 = 62.01% [57.44–66.59] | 437/658 = 66.41% | 2,716/2,849 = 95.33% [94.56–96.10] |
| held-out | 54 | 222/375 = 59.20% [54.13–64.00] | 375/538 = 69.70% | 2,005/2,091 = 95.89% [95.03–96.70] |

The [#839 baseline](default-head-baseline-839.md) records the published release
asset and checksum, all input hashes, 120/120 parity, per-surface counts, and
byte-identical repeated reports. Pass `--precision-surface all` explicitly when a
comparison needs the pre-#839 full-universe precision definition.

**Historical snapshot (2026-07-11, nose 0.18.0):** the checked
[machine-readable artifact](../bench/labels/product_quality_evaluation_2026_07_11.v2.json) records
the exact command and configuration, git SHA
`52457d541af605a65281f2c8642e153d2fe80950`, release-binary SHA-256
`a0a210b9527353b65bbf253c1ee483f9a435e69e4b9587b432eefdcbdba52128`,
all component/corpus/prune hashes, the 120 pinned repository commits,
per-repository counts, and deterministic 500-resample confidence intervals.

| split | repos | labeled precision@10 | matched top-10 | worthy recall |
|---|---:|---:|---:|---:|
| dev | 66 | 259/437 = 59.27% [54.92–64.30] | 437/660 = 66.21% | 2,626/2,849 = 92.17% [91.30–93.19] |
| held-out | 54 | 206/383 = 53.79% [48.30–58.75] | 383/540 = 70.93% | 1,949/2,091 = 93.21% [92.20–94.36] |

Precision is conditional on a current top-10 family matching an active precision label;
coverage is therefore reported as a first-class companion metric. On the original 105
repositories, v6 matches 771/1,050 positions (73.43%) versus v5's 692/1,050 (65.90%):
**+79 positions / +7.52 percentage points**. Across all 120 repositories it matches
820/1,200 (68.33%). Real Swift coverage is 49/150, with labeled P@10 of 11/24 dev
(45.83%) and 12/25 held-out (48.00%); Swift worthy-recall is absent because no
source-independent Swift recall pool exists yet.

The v5 labelset remains byte-frozen, and its historical results reproduce exactly.
Running the evaluator with
`--labelset bench/labels/refactoring_families.v5.json --precision-surface all`
produced a checked
[v5 reproduction report](../bench/labels/product_quality_evaluation_v5_reproduction_2026_07_11.v2.json) whose
raw counts, point estimates, and bootstrap intervals leave the frozen
[2026-07-10 v1 report](../bench/labels/product_quality_evaluation_2026_07_10.v1.json) exactly unchanged.
The v2 report adds coverage/eligibility metadata only.

### Current missed-worthy frontier

The [#816 dev-first frontier audit](missed-worthy-frontier-816.md) found that 51/223
dev misses already had accepted raw pairs. [#817](accepted-pair-coverage-817.md)
preserved those endpoints through family folding and recovered 65 dev plus 47 held-out
worthy families with zero regressions.

The current [post-#817 refresh](missed-worthy-frontier-820.md) exactly reproduces
2,691/2,849 dev and 1,996/2,091 held-out recall, leaving 158 and 95 misses. Its regenerated
raw-stage census confirms zero remaining direct accepted rows. A source-bound dev audit
selects the complete candidate-only + sub-DAG>=20 cohort: six of nine rows have coherent
connected witnesses and three are hard negatives. Same-unit windows are sampled at 3/4
coherent but require bounded fragment output; six sampled extraction rows are real but
span unrelated construct-specific laws. The bounded next issue is
[#821](https://github.com/corca-ai/nose/issues/821), which must construct an actual connected
witness without admitting optimistic intersection mass.

No detector, ranking, or default-surface policy changed in this refresh. The historical
anti-unification re-rank remains comparison-only and still supplies no generalizing reason
to replace the native extractability order.

## Soundness — the behavioral oracle

nose's value-graph fingerprint is **sound by intent**: equal fingerprint ⟹ equal behavior
(experiments §AJ). `nose verify` enforces it — a tree-walking interpreter runs every unit on
an input battery and flags any fingerprint-equal pair whose behavior differs. It interprets
the *pre-canonicalization* IL (so a behavior-changing canon can't mask itself), and a
**canon-preservation** check requires each unit's core-IL behavior to equal its full-IL
behavior *up to abort* — two runs that both error are equivalent regardless of the effects
recorded before the trap, so an impossible input can't manufacture a phantom violation
(experiments §CN). The core canonicalizations are additionally machine-checked in Lean (`formal/`).
Both currently report **zero** violations on the characterized gates. `verify` is bounded:
units whose estimated work (`IL nodes × battery rows`) exceeds the oracle budget fail closed as
`battery-bail` and appear in the exclusion census instead of monopolizing the run.

```sh
nose verify bench/repos --max-violations 0   # SOUND / canon PRESERVED, + a completeness ratio
```

The repository's nightly `corpus verify` workflow runs the same oracle one
pinned corpus repo at a time and uploads per-repo logs if the zero-false-merge or
canon-preservation gate trips. Symbolic-trace disagreements remain advisory and
are counted in the summary without failing the run.

## Declarative languages (CSS / HTML)

[CSS and HTML markup](languages.md) are declarative — equivalence is *same computed style* /
*same rendered DOM*, not imperative behavior — so they are measured by an analogous but
separate instrument: a labeled benchmark of POSITIVE groups (computed-equivalent snippets
that must share a fingerprint) and HARD-NEGATIVE pairs (computed-distinct snippets that must
not), one per equivalence-class axis.

```sh
cargo test -p nose-cli --test css_html_quality -- --nocapture   # recall + soundness, per axis
```

Headline (current): **recall 13/13 positive groups converged (100%)** across the modeled
axes — CSS color (hex/short/name/`rgb()`), extended named colors, `hsl()`/`url()` spelling,
zero-units, number canon, box-shorthand collapse, declaration-order and selector
independence, media-query condition and value canonicalization; HTML DOM normalization
(attribute order/boolean/`class`-set/whitespace/case), inline-`style=` canonicalization, and
Vue/Svelte directive shorthand — and **soundness 14/14 hard negatives kept distinct (100%)**
(distinct colors/values, repeated-property and shorthand/longhand cascade order,
box-not-all-equal, value-order, at-rule condition, `hsl` distinctness, `@media` vs `@supports`
and distinct media conditions; HTML text/attr/child-order/`<pre>`-whitespace differences).
CSS, HTML, and imperative fingerprints are domain-disjoint, so the language-blind exact
channel can never merge across them.

Coverage is first-class: the [Raw-node ratio](languages.md#coverage-and-adding-a-language)
on real-world `.css`/`.html` is sub-percent (CSS ~0.002%, HTML ~0.4% on hand-written
markup). Soundness is **by construction** — the fingerprint *is* the canonical computed
style / rendered DOM, so equal fingerprint ⟺ equal denotation — backed by the adversarial
per-rule batteries above plus the `css_value` unit tests (the project's primary trust
mechanism, [design §1](design.md)); the obligation is registered `empirical-only`
(`formal/obligations/normalize/css/computed_style/`), not yet Lean-proven. `nose verify`
excludes declarative units (a declarative domain has no imperative behavior to interpret)
and its imperative gate is unaffected. Modeled scope and honest limits (SCSS/Less, cross-file
`var()`, shorthand↔longhand expansion, Svelte block grammar) are in
[clone-types](clone-types.md) and [languages](languages.md).

## Throughput

The detector is parallel at every stage and designed for deterministic output; tests cover
repeat runs and thread-count variation on the local platform. The archived §T run measured
about **19,500 files/sec** warm on its pinned corpus/hardware, with frontend parse+lower
dominating and scaling about 11.6x on 18 cores. `NOSE_TIME=1 nose query <path>`
prints the per-stage breakdown for your machine (the timing covers the whole analysis run
regardless of how many families are displayed). The default mode already runs
the full `syntax,semantic,near` surface. See
experiments §T for the throughput work.

Current corpus speed budget: a 2026-06-19 release-build pass over the checked-out
`bench/repos` corpus (`target/release/nose query bench/repos/<repo>`, one repo at a time)
completed **150/150 repos successfully**, **0 repos at or above 4s**, **82.063s total**, max
**3.989s** (`sympy`). The saved run is `target/corpus-query-speed-release-0.13.3/summary.tsv`; see [experiments §CZ](experiments.md#cz-corpus-query-speed-budget-pass-2026-06-19).

## The research commands

The everyday surface is `nose query` (interactive exploration, carrying the
`--fail-on`/`--baseline` CI gate). The default mix is
`syntax,semantic,near` (experiments §BM priced the flip); benchmark runs that need the
pre-flip exact-only surface pin `--mode syntax,semantic` explicitly. The benchmark also
uses a hidden research surface:

- `nose detect <paths> --out preds.json` — raw clone pairs/groups (the signal before the
  refactoring-family grouping).
- `nose verify <paths>` — the soundness oracle (above).
- `nose features <paths>` — per-unit fingerprints as JSON (convergence analysis).
- `nose eval` / `nose ceiling` — score predictions against a gold set / split recall across
  the extraction and candidate-generation stages.

`nose behavioral-gate` is a hidden experimental Type-4 benchmark command for measuring a
behavioral-equivalence acceptance gate against a generated manifest; it is not a stable
integration surface.

These exercise the same engine described in [architecture](architecture.md); the qualitative
counterpart — running nose on real third-party code — is [field-evaluation](field-evaluation.md).
