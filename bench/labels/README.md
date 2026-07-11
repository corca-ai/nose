# Refactoring-family labelset

Ground-truth evaluation data for nose's **product** metric: does `nose query`
surface *genuine refactoring candidates*? Each label is on a clone **family** (the
unit nose reports), judged worthy / not-worthy per [`RUBRIC.md`](RUBRIC.md).

This is the most important asset in `bench/` — the metric that keeps ranking/
detection changes honest (it has rejected several plausible-but-wrong ideas; see
`docs/experiments.md` §U/§V/§X/§Z/§AB).

## The active set — `refactoring_families.v6.json`

v6 is a hash-checked composite manifest, not a rewrite of the historical pool:

- frozen `refactoring_families.v5.json`: 9,461 multi-source families used for both
  precision and worthy-recall;
- `refactoring_families.v6.dev.json`: 59 current-top-10 judgments;
- `refactoring_families.v6.heldout.json`: 56 current-top-10 judgments.

Together they contain **9,576 families** — 4,995 worthy / 4,581 not-worthy — over
**120 pinned repositories and 8 imperative languages**. The split remains a hard
generalization boundary: 5,504 labels are dev and 4,072 are held-out; tune only on dev.
The manifest checks the SHA-256 of the frozen base and both split components before
loading them.

### v6 coverage-refresh protocol

The refresh artifact
[`refactoring_label_refresh_candidates_2026_07_11.v1.json`](refactoring_label_refresh_candidates_2026_07_11.v1.json)
records the exact nose binary, command, repository commits, raw query-family hashes,
2,072 source-file hashes, deterministic selection order, and label/rubric inputs.
Selection was frozen before judgment:

- five current unmatched top-10 families per language × split across the existing
  seven-language corpus (70 labels), using the SHA-256 seed
  `nose-issue-812-existing-unmatched-v1` and preferring distinct repositories;
- the first three current product families from every one of the 15 real pinned Swift
  repositories (45 labels).

The three-persona panel judged 115 families (55 worthy / 60 not-worthy); 50 split votes
were resolved by an explicit arbiter. The dev decisions/component were committed before
held-out judgment. Every refresh label carries `metric_eligibility: ["precision_at_10"]`:
because this sample came from the current top-10, allowing it into worthy-recall would
bias that metric. Worthy-recall therefore remains the frozen v5 multi-source pool. No
ranking, detector, or default-surface policy changed during the refresh.

## Swift add-on — `swift_families.v1.json`

`swift_families.v1.json` is the earlier focused add-on golden over the executable Swift
Type-4 probes. It contains
5 high-confidence, 3-persona LLM-judge families with adjacent hard negatives for
collection emptiness, string affixes, collection membership, option presence, and
for-in/indexed-loop reduction. It remains a Swift bring-up golden, not part of the product
metric. v6 supersedes its coverage limitation with 45 worthy/not-worthy labels from all
15 real Swift repositories and both corpus splits.

## The declarative set — `frontend_families.v1.json`

The same product metric for the **declarative track** (CSS + HTML/Vue/Svelte/JSX/TSX
markup, incl. cross-dialect). Same schema, same RUBRIC, same 3-persona panel + 2-1
tiebreak methodology; built by the `frontend-goldset-panel` workflow over a nose
low-threshold pool (generated-filtered per `is_generated_loc`, capped ≤60/repo for
diversity) across 19 frontend repos + the RealWorld trio for cross-dialect.

- **448 families** — 71 worthy / 377 not-worthy. dev 386 / **heldout** 62.
- **Worthy-rate by corpus kind** (the headline precision profile):
  | kind | families | worthy |
  |---|---|---|
  | app (hand-written markup) | 44 | **50%** |
  | cross-dialect (React≡Vue≡Svelte) | 10 | **70%** |
  | CSS framework (dist) | 394 | **10%** |
- **What it measures.** On real app markup, precision is ~50% — on par with the imperative
  languages. On shipped CSS frameworks it is ~10%: the surface there is **69% `generated`**
  (CSS compiled from SCSS — `css/bulma.css` etc., which `is_generated_loc` does NOT catch
  because they are not `.min`/`dist/`) plus `parallel-by-design` utility scales. The
  cross-dialect arm (nose's novel capability) is 70% worthy — genuinely-shared components
  (TagList, article-meta, error-list, banner) across frameworks; the 30% not-worthy are
  small generic shells (`li.nav-item`, a `<button>`) coincidentally same-shaped across
  unrelated components. This says nothing is broken in the markup engine — it quantifies
  that **the CSS-framework default surface needs a compiled-CSS filter** (a measured lever)
  and that real-code/cross-dialect precision is healthy.
- Each family records its 3-persona `votes`. **v1 is dev-grade** (panel-labeled, not yet
  arbitrated); the 45 medium-confidence (2-1 split) families are the audit queue.

### `frontend_families.v2.json` — arbitrated + grown

v2 grows the set and **resolves every 2-1 split with an LLM arbiter** (the authoritative
final judge — replaces the human-arbitration step; `labeler: llm-arbiter`). Built by the
`frontend-goldset-v2-panel` + `frontend-goldset-arbiter` workflows: 62 new candidates from
added app/markup repos (excalidraw, solid/preact/angularjs RealWorld, svelte-sites) and the
combined-RealWorld cross-dialect pool were panel-labeled, then all 55 non-high families (v1's
46 + 9 new splits) went to a high-effort rubric-strict arbiter that re-read the code.

- **510 families** — 107 worthy / 403 not. **505 high-confidence, 5 low** (genuinely
  undecidable, marked so). `labeler`: 455 panel, 55 llm-arbiter.
- Worthy-rate by kind: **app 45%** (85), **cross-dialect 77%** (31), **CSS framework 11%** (394).
- The cross-dialect arm grew 10 → 31 by pooling the React/Vue/Svelte/Solid Conduit
  implementations together (the same app across dialects). The arbiter decisively reclassified
  the v1 medium families — notably tachyons `src/*.css` + compiled + min families as
  `generated` (a build-pipeline artifact the compiled-CSS surface filter still under-catches:
  a future lever).

### `frontend_families.v3.json` — grown for per-dialect coverage

v3 adds real app repos to lift the per-language CI bound (precision CIs are bounded by
#repos × 10, not #labels) and labels them with the same panel + LLM-arbiter pipeline.
Added: Vue (vitesse, vuero, vue-cli, vue-theme), Svelte (sveltesociety, svelte-core) — Vue
2 → 6 repos, Svelte 2 → 4. 264 new candidates panel-labeled; the 39 non-high arbitrated.

- **774 families** — 196 worthy / 578 not. 765 high / 8 low (undecidable) / 1 medium.
  `labeler`: 681 panel, 93 llm-arbiter.
- Worthy-rate by kind: **app 36%** (349), **cross-dialect 77%** (31), **CSS framework 11%** (394).
- Repos per dialect: css 13, html 16, vue 6, svelte 4, react 5 (+ the cross-dialect pool).
- **Measured NO-GO** recorded alongside: mechanically demoting `parallel-by-design` utility
  scales / per-state demos from the surface is unsound — they are structurally identical to
  the *worthy* `parameterize`/`extract-data-table` families (same member-count distribution
  2–17, same selectors); the distinction is a semantic judgment only the panel can make.
- Each family records its 3-persona `votes`, so agreement is auditable. The format is
  defined in [`schema.json`](schema.json).

## How it was built (methodology)

The frozen v5 base was produced by an LLM-panel pipeline (the build scripts are historical;
this records the method):

1. **Pool** — an unbiased candidate set: nose's structural candidates ∪ a `jscpd`-weak
   pass over dev+heldout. The independent `jscpd` arm ensures families nose *misses* are
   present, so worthy-**recall** is measurable, not just precision.
2. **Panel** — 3 personas (pragmatic / dedupe / skeptic) label each family independently
   against `RUBRIC.md`.
3. **Reconcile** — majority vote; 2-1 splits go to a rubric-strict tie-break judge, and the
   still-ambiguous to a final arbiter (`labeler: claude-arbiter`; 126 remain genuinely
   undecidable and are marked as such).

The base evolved v1 (235) → v2 (576, +heldout) → v3 (3,092) → v4 (4,615, 62 repos) →
**v5 (9,461, 105 repos)**; adding repos per language is the lever for per-language
*precision* CIs (bounded by #repos×10, not #labels). v5 (§AU) settled the anti-unification
re-rank as small-sample overfit (+1pp dev / −1pp heldout, Rust-only — **not shipped**).
v6 retains that pool byte-for-byte and adds only the split-safe, precision-eligible overlay
described above.

## Adjacent audit artifacts

`prune_manifest.json` is the reproducibility artifact for `bench/setup_repos.sh`'s
file-level corpus prune. It lists generated/vendored source files removed after clone,
label-referenced files that were protected from removal, and the post-prune corpus
digest used to verify a reconstructed checkout. The checked-in manifest remains scoped to
the active v6 composite and all 120 pinned repositories. Its labelset path and protected
source counts are checked together with the post-prune corpus digest.

`fragment_quality_audit_2026_06_10.json` is not part of the active v6 product metric. It is a
small, three-person audit of Java/Python hidden/divergence exact-fragment families used to
validate surface policy after the semantic corpus pass. See
[`docs/fragment-quality-audit-2026-06-10.md`](../../docs/fragment-quality-audit-2026-06-10.md).

`lawpack_provenance_audit_2026_06_10.json` is also adjacent evidence, not part of
the active metric. It records the full-corpus and targeted real-repo pass for the
first-party `nose.value_graph.laws` LawPack pilot. See
[`docs/lawpack-provenance-audit-2026-06-10.md`](../../docs/lawpack-provenance-audit-2026-06-10.md).

`recall_ceiling_probe.py` + `recall_ceiling_probe_2026_06_10.json` are the design §5
recall-ceiling probe: for every worthy label the maximal current query surface misses, an
over-approximated classification of whether generalized sub-DAG matching or one-step
pure inlining could recover it. The measured verdict and method are recorded in
[`docs/experiments.md`](../../docs/experiments.md) §BJ.

The current #816 refresh is
`recall_ceiling_probe_2026_07_11.v2.json`, backed by the same script plus
`missed_worthy_frontier.py`. It records complete binary/input/repository/query provenance,
exactly reproduces the v6 evaluator's v5-worthy recall counts, and freezes a deterministic
35-family, seven-language dev audit before source judgment. The adjacent dev stage,
decision, held-out confirmation, and #809 pricing artifacts separate raw accepted pairs
from final query coverage. The selected Route A follow-up is #817; method, rejected routes,
and commands are documented in
[`docs/missed-worthy-frontier-816.md`](../../docs/missed-worthy-frontier-816.md).

`query_json_agent_audit_2026_06_10.json` records the #216 agent-usability audit of the
query-JSON contract: 18 sampled families, JSON-only decisions graded against source,
and the ranked evidence-gap list. See
[`docs/query-json-agent-audit-2026-06-10.md`](../../docs/query-json-agent-audit-2026-06-10.md).

`near_default_surface_experiment.py` +
`near_default_surface_2026_06_10.json` price the product decision of adding the
`near` channel to the default query surface. The script compares default,
`syntax,semantic,near`, and two thresholded `near` arms on v5 P@10, worthy-recall,
and default-surface family-count deltas. The decision record is in
[`docs/experiments.md`](../../docs/experiments.md) §BM.

`ruby_test_dsl_recovery_2026_06_10.json` is the #214 recovery artifact for Ruby
test-DSL block extraction. It compares the recall-ceiling probe before/after
allowlisted Ruby test blocks became `Block` units, records the remaining Ruby
misses, and captures the Ruby unit-count extraction delta. The decision record is
in [`docs/experiments.md`](../../docs/experiments.md) §BN.

`rust_macro_rules_recovery_2026_06_10.json` is the #215 recovery artifact for
Rust `macro_rules!` arm extraction. It records the feasibility spike conclusion,
the Rust recall-ceiling probe before/after, remaining Rust no-overlapping-unit
records, default P@10, and Rust corpus surface/raw-ratio deltas. The decision
record is in [`docs/experiments.md`](../../docs/experiments.md) §BO.

`merge_exclusion_census.py` + `oracle_exclusion_census_2026_06_10.json` +
`oracle_under_merge_leads_2026_06_10.json` are the oracle-completeness campaign's
baseline: per-construct inventory of units the interpreter oracle cannot check (and the
fingerprint-merge mass left unverified), plus the merged behavior-equal/fingerprint-split
under-merge leads. Method and numbers in
[`docs/experiments.md`](../../docs/experiments.md) §BL.

## Scoring against it

`eval_by_language.py` — per-language precision@10 + worthy-recall, dev/heldout split, with
**bootstrap 95% CIs** and per-repository denominator/coverage counts. The CIs are essential:
they tell you whether a per-language difference is real or noise.

```sh
cargo build --release -p nose-cli
python3 bench/labels/query_schema.py --self-test --nose target/release/nose
python3 bench/labels/eval_by_language.py --rank extractability --bootstrap 500 \
  --json-out bench/labels/product_quality_evaluation_2026_07_11.v2.json
```

The [2026-07-11 product-quality artifact](product_quality_evaluation_2026_07_11.v2.json)
records the exact binary/source/input hashes, all composite input hashes, pinned corpus
digest, configuration, per-repository counts, and current dev/held-out metrics. Precision
remains conditional on a top-10 family matching an active precision label; coverage is
reported beside it, never silently discarded.

| split | repos | labeled precision@10 | matched top-10 | v5 worthy recall |
|---|---:|---:|---:|---:|
| dev | 66 | 259/437 = 59.27% [54.92–64.30] | 437/660 = 66.21% | 2,626/2,849 = 92.17% [91.30–93.19] |
| held-out | 54 | 206/383 = 53.79% [48.30–58.75] | 383/540 = 70.93% | 1,949/2,091 = 93.21% [92.20–94.36] |

On the original 105 repositories, v6 matches 771/1,050 top-10 positions versus v5's
692/1,050: +79 positions and +7.52 percentage points. Across all 15 Swift repositories it
matches 49/150 positions; the 45 selected labels can match more than one overlapping
reported family. Swift's labeled P@10 is 11/24 dev and 12/25 held-out. Swift has no
multi-source recall labels yet, so its recall denominator is correctly zero.

To reproduce the historical v5 metric, pass the base explicitly:

```sh
python3 bench/labels/eval_by_language.py \
  --labelset bench/labels/refactoring_families.v5.json \
  --rank extractability --bootstrap 500 \
  --json-out bench/labels/product_quality_evaluation_v5_reproduction_2026_07_11.v2.json
```

The [v5 reproduction artifact](product_quality_evaluation_v5_reproduction_2026_07_11.v2.json)
has the same 105 repositories and exactly reproduces every metric field (raw counts, point
estimates, and deterministic bootstrap intervals) in the frozen
[2026-07-10 v1 report](product_quality_evaluation_2026_07_10.v1.json). Schema v2 adds only
coverage and metric-eligibility metadata.

Pass `--mode` to compare a non-default channel mix without editing the script:

```sh
python3 bench/labels/eval_by_language.py --mode syntax,semantic,near
```
