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

## Scoring against it

`eval_by_language.py` — per-language precision@10 + worthy-recall, dev/heldout split, with
**bootstrap 95% CIs** and the lowering confound (mean Raw-node ratio). The CIs are
essential: they tell you whether a per-language difference is real or noise.

```sh
python3 bench/labels/eval_by_language.py
```
