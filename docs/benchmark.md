# Benchmark

How nose's quality is measured, and the headline numbers. The blow-by-blow log of
individual experiments is in [experiments](experiments.md); this page is the methodology.

There are two distinct questions, measured separately:

| question | how | data |
|---|---|---|
| **Product quality** — does the query surface rank *genuine* refactoring candidates first? | precision@10 + worthy-recall, per language, dev/held-out, bootstrap 95% CIs | the v5 refactoring-family labelset |
| **Soundness** — does an equal fingerprint really mean equal behavior? | an interpreter oracle on a battery of inputs (`nose verify`) + Lean proofs | the pinned corpus |

A third asset is the [Type-4 benchmark factory](type4-benchmark.md): an evidence-carrying
synthetic benchmark for exact semantic equivalence classes. It is separate from the product
labelset because Type-4 exactness asks whether two fragments compute the same thing under a
declared semantics, not whether a reported family is worth refactoring.

## Product quality — the refactoring-family labelset

The active gold set is `bench/labels/refactoring_families.v5.json` (105 repos, ~9.5k
families, each judged *worthy / not-worthy* of refactoring by a 3-persona LLM panel with
tie-break/arbiter escalation — see [bench/labels/README.md](../bench/labels/README.md) and
its [RUBRIC.md](../bench/labels/RUBRIC.md)). The corpus has a **dev / held-out** split (`bench/goldens/corpus.json`),
so a change has to generalize, not just fit the dev repos; tune only on dev.

Swift support adds 15 pinned Swift repositories to `bench/goldens/corpus.json` for
lowering/oracle/corpus coverage (120 repos total). The v5 product labelset remains frozen
at 105 repos; Swift's initial LLM-judge add-on golden is
`bench/labels/swift_families.v1.json` and is tied to the executable Type-4 Swift probes.
That add-on has five worthy dev examples from synthetic probes, not product labels from the
15 real repositories; [#812](https://github.com/corca-ai/nose/issues/812) tracks real Swift
and current top-10 label coverage.
The checked-in prune manifest is still the v5 product-corpus artifact; reconstructing the
expanded corpus rewrites it for that checkout until the next product-labelset refresh
commits a new prune snapshot.

```sh
bench/setup_repos.sh                      # clone the pinned corpus into bench/repos
python3 bench/prune_corpus.py --check-manifest  # verify the recorded prune digest
cargo build --release -p nose-cli
python3 bench/labels/query_schema.py --self-test --nose target/release/nose
python3 bench/labels/eval_by_language.py --rank extractability --bootstrap 500 \
  --json-out bench/labels/product_quality_evaluation_2026_07_10.v1.json
```

`--rank value` reproduces the historical volume order; `--rank extractability` keeps the
native `nose query --format json` order used by the current product. Both reports also show
the historical anti-unification re-rank comparison. Query output passes through a strict
schema-v7 adapter: a changed envelope, location key, surface, or scope fails with a
path-specific error instead of silently dropping a result.

**Reproducible snapshot (2026-07-10, nose 0.18.0):** the checked
[machine-readable artifact](../bench/labels/product_quality_evaluation_2026_07_10.v1.json) records
the exact command and configuration, git SHA `61fde21d3a43fe10e6611184a104da695ba2da8c`,
release-binary SHA-256 `a0a210b9527353b65bbf253c1ee483f9a435e69e4b9587b432eefdcbdba52128`,
label/corpus/prune hashes, the 105 pinned repository commits, per-repository counts, and
deterministic 500-resample confidence intervals.

| split | repos | labeled precision@10 | matched top-10 | worthy recall |
|---|---:|---:|---:|---:|
| dev | 58 | 60.53% [55.73–65.87] | 375/580 | 2,626/2,849 = 92.17% [91.30–93.16] |
| held-out | 47 | 54.89% [49.53–60.25] | 317/470 | 1,949/2,091 = 93.21% [92.11–94.36] |

Precision is conditional on a current top-10 family matching the frozen v5 pool. The
artifact records 358 unmatched positions (205 dev, 153 held-out), so they are neither
silently omitted from the coverage audit nor guessed worthy/not-worthy. Expanding that pool
and the synthetic-only Swift add-on is [#812](https://github.com/corca-ai/nose/issues/812),
separate from this evaluator repair. The anti-unification re-rank measures 60.05% dev and
55.33% held-out, providing no generalizing reason to change current ranking policy here.

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
