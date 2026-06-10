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
