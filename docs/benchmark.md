# Benchmark

How nose's quality is measured, and the headline numbers. The blow-by-blow log of
individual experiments is in [experiments](experiments.md); this page is the methodology.
Back to [home](home.md).

There are two distinct questions, measured separately:

| question | how | data |
|---|---|---|
| **Product quality** — does the review-oriented scan surface rank *genuine* refactoring candidates first? | precision@10 + worthy-recall, per language, dev/held-out, bootstrap 95% CIs | the v5 refactoring-family labelset |
| **Soundness** — does an equal fingerprint really mean equal behavior? | an interpreter oracle on a battery of inputs (`nose verify`) + Lean proofs | the pinned corpus |

## Product quality — the refactoring-family labelset

The active gold set is `bench/labels/refactoring_families.v5.json` (105 repos, ~9.5k
families, each judged *worthy / not-worthy* of refactoring by a 3-persona LLM panel with
tie-break/arbiter escalation — see [`bench/labels/README`](../bench/labels/README.md) and
its `RUBRIC.md`). The corpus has a **dev / held-out** split (`bench/goldens/corpus.json`),
so a change has to generalize, not just fit the dev repos; tune only on dev.

```sh
bench/setup_repos.sh                      # clone the pinned corpus into bench/repos
python3 bench/labels/eval_by_language.py  # P@10 + worthy-recall, per language, dev/held-out, 95% CIs
```

**Headline (current):** precision@10 ≈ **60% dev / 54% held-out**, worthy-**recall ≈ 99%**.
The per-language CIs are wide (bounded by #repos×10), which is the point — they tell you
whether a per-language difference is real or noise. The standing finding (experiments §AV)
is that the residual precision loss is *judgment-deep* (genuinely-ambiguous, parallel-by-
design families), not a detector signal gap.

## Soundness — the behavioral oracle

nose's value-graph fingerprint is **sound by intent**: equal fingerprint ⟹ equal behavior
(experiments §AJ). `nose verify` enforces it — a tree-walking interpreter runs every unit on
an input battery and flags any fingerprint-equal pair whose behavior differs. It interprets
the *pre-canonicalization* IL (so a behavior-changing canon can't mask itself), and a
**canon-preservation** check requires each unit's core-IL behavior to equal its full-IL
behavior. The core canonicalizations are additionally machine-checked in Lean (`formal/`).
Both currently report **zero** violations.

```sh
nose verify bench/repos   # SOUND / canon PRESERVED, + a completeness ratio
```

## Throughput

The detector is parallel at every stage and deterministic across runs, threads, and
machines. On the pinned corpus it sustains **~19,500 files/sec** (warm, full pipeline);
the frontend (tree-sitter parse + lower, ~65%) dominates and scales ~11.6× on 18 cores.
`NOSE_TIME=1 nose scan <path> --top 0` prints the per-stage breakdown. Add
`--mode syntax,semantic,near` when measuring the full review surface. See
experiments §T for the throughput work.

## The research commands

The everyday surface is `nose scan` ([usage](usage.md)). The exact default is
`syntax,semantic`; benchmark runs that evaluate review-oriented Type-3 candidates should
enable `near` explicitly. The benchmark also uses a hidden research surface:

- `nose detect <paths> --out preds.json` — raw clone pairs/groups (the signal before the
  refactoring-family grouping).
- `nose verify <paths>` — the soundness oracle (above).
- `nose features <paths>` — per-unit fingerprints as JSON (convergence analysis).
- `nose eval` / `nose ceiling` — score predictions against a gold set / split recall across
  the extraction and candidate-generation stages.

These exercise the same engine described in [architecture](architecture.md); the qualitative
counterpart — running nose on real third-party code — is [field-evaluation](field-evaluation.md).
