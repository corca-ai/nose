# Refactoring-family labelset

Ground-truth evaluation data for nose's **product** metric: does `nose scan`
surface *genuine refactoring candidates*? Each label is on a clone **family** (the
unit nose reports), judged worthy / not-worthy per [`RUBRIC.md`](RUBRIC.md).

This is the most important asset in `bench/` — the metric that keeps ranking/
detection changes honest (it has rejected several plausible-but-wrong ideas; see
`docs/experiments.md` §U/§V/§X/§Z/§AB).

## The set — `refactoring_families.v5.json`

- **9,461 families** — 4,940 worthy / 4,521 not-worthy.
- **105 repos, 7 languages**, with a dev (5,445) / **held-out** (4,016) split — held-out
  is a generalization gate; tune only on dev.

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

The frozen labelset was produced by an LLM-panel pipeline (the build scripts are historical;
this records the method):

1. **Pool** — an unbiased candidate set: nose's structural candidates ∪ a `jscpd`-weak
   pass over dev+heldout. The independent `jscpd` arm ensures families nose *misses* are
   present, so worthy-**recall** is measurable, not just precision.
2. **Panel** — 3 personas (pragmatic / dedupe / skeptic) label each family independently
   against `RUBRIC.md`.
3. **Reconcile** — majority vote; 2-1 splits go to a rubric-strict tie-break judge, and the
   still-ambiguous to a final arbiter (`labeler: claude-arbiter`; 126 remain genuinely
   undecidable and are marked as such).

The labelset evolved v1 (235) → v2 (576, +heldout) → v3 (3,092) → v4 (4,615, 62 repos) →
**v5 (9,461, 105 repos)**; adding repos per language is the lever for per-language
*precision* CIs (bounded by #repos×10, not #labels). v5 (§AU) settled the anti-unification
re-rank as small-sample overfit (+1pp dev / −1pp heldout, Rust-only — **not shipped**).

## Adjacent audit artifacts

`prune_manifest.json` is the reproducibility artifact for `bench/setup_repos.sh`'s
file-level corpus prune. It lists generated/vendored source files removed after clone,
label-referenced files that were protected from removal, and the post-prune corpus
digest used to verify a reconstructed checkout.

`fragment_quality_audit_2026_06_10.json` is not part of the v5 product metric. It is a
small, three-reviewer audit of Java/Python hidden/review exact-fragment families used to
validate surface policy after the semantic corpus pass. See
[`docs/fragment-quality-audit-2026-06-10.md`](../../docs/fragment-quality-audit-2026-06-10.md).

`lawpack_provenance_audit_2026_06_10.json` is also adjacent evidence, not part of
the v5 metric. It records the full-corpus and targeted real-repo pass for the
first-party `nose.value_graph.laws` LawPack pilot. See
[`docs/lawpack-provenance-audit-2026-06-10.md`](../../docs/lawpack-provenance-audit-2026-06-10.md).

`recall_ceiling_probe.py` + `recall_ceiling_probe_2026_06_10.json` are the design §5
recall-ceiling probe: for every worthy label the maximal current scan surface misses, an
over-approximated classification of whether generalized sub-DAG matching or one-step
pure inlining could recover it. The measured verdict and method are recorded in
[`docs/experiments.md`](../../docs/experiments.md) §BJ.

`scanjson_agent_audit_2026_06_10.json` records the #216 agent-usability audit of the
scan-JSON contract: 18 sampled families, JSON-only decisions graded against source,
and the ranked evidence-gap list. See
[`docs/scanjson-agent-audit-2026-06-10.md`](../../docs/scanjson-agent-audit-2026-06-10.md).

`near_default_surface_experiment.py` +
`near_default_surface_2026_06_10.json` price the product decision of adding the
`near` channel to the default scan surface. The script compares default,
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
**bootstrap 95% CIs** and the lowering confound (mean Raw-node ratio). The CIs are
essential: they tell you whether a per-language difference is real or noise.

```sh
python3 bench/labels/eval_by_language.py
```

Pass `--mode` to compare a non-default channel mix without editing the script:

```sh
python3 bench/labels/eval_by_language.py --mode syntax,semantic,near
```
