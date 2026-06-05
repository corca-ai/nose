# Hazard benchmark (evaluation criteria + dataset)

> Status: **specification.** This page defines *how we measure whether
> [hazard ranking](hazard-ranking.md) is good* and how the labeled dataset is
> built. The dataset and pipeline are not yet built; this is the criteria to build
> them against. Measured results land in [experiments](experiments.md).

A hazard score is only as trustworthy as the yardstick it is tuned against. The
weights in `hazard()` are **not** fixed by the literature — they must be calibrated
and validated against ground truth. This benchmark *is* that ground truth: a large,
graded, multi-language dataset of clone families labeled by whether they were later
edited inconsistently and caused a bug. Building it well is the highest-leverage
task in this effort, so it comes first.

## What we measure

The **divergent-edit hazard** of a clone family: the likelihood that a developer
edits one copy, misses the siblings, and ships an inconsistent-edit bug. This is
partly **realized** (it already happened, and is mineable from history) and partly
**latent** (the clone is dangerous but has not yet been mis-edited). Version control
gives us objective labels for the *realized* part only; the latent part is covered
separately and weakly by Tiers 2–3 in [hazard-ranking](hazard-ranking.md#evaluating-ranking-quality).
This page builds the realized, objective core.

## Unit of evaluation

A **family observed at revision _T_**, labeled by what happens to it in a forward
window **_(T, T+Δ]_**. Scoring at _T_ and labeling from the future is a strict
forward-prediction split — no leakage. _Δ_ is a fixed wall-clock or release span
(see [Granularity](#granularity-and-the-release-rule)). The same family contributes
multiple examples at different _T_ if its history is long enough.

## Label schema (graded)

Three ordered tiers, from a per-family genealogy in _(T, T+Δ]_:

| label | definition | role |
|---|---|---|
| **G2 — harmful divergence (gold)** | a sibling was changed while others were not (a *divergent edit*), **and** the lagging sibling was later brought into line by a **bug-fix** commit (SPCP / late-propagation pattern) | strongest positive; the event hazard ranking exists to predict |
| **G1 — realized divergence (weak)** | a divergent edit occurred (one sibling changed, others did not) with no confirmed bug link, and the divergence **persisted** (not re-synced as a clean refactor) | dense weak positive |
| **G0 — control** | the family changed **consistently** (all siblings together) or did not change | negative |

Graded, not binary, because release-level harmful events are sparse (1–3%, Bettenburg
WCRE 2009): G2 alone is too thin to calibrate on, G1 adds density, and keeping them
*ordered* lets us validate that the score ranks G2 > G1 > G0, not just positive vs
negative. **Intentional adaptation is excluded from G1/G2** via the proxies in
[Intentionality](#separating-accidental-from-intentional).

## Tracking families and detecting divergence

The technical key — and why nose can build a dataset textual tools cannot. Within a
revision, a semantic family is a set of fragments sharing one **value fingerprint**
_F_ ([normalization](normalization.md)). Across revisions we track by that
fingerprint identity plus location continuity:

- **Consistent state:** at _T+1_ every member still maps to _F_ (possibly relocated).
- **Consistent change:** every member moves _F → F′_ together (propagated edit) → G0.
- **Divergent edit:** a *subset* moves _F → F′_ while siblings stay at _F_ → the
  family **splits**. This is the Kim *Inconsistent Change* predicate, computed
  natively from fingerprints rather than from textual line-overlap (Lozano &
  Wermelinger), which cannot follow Type-4 siblings.
- **Re-link the laggard:** after a split, the diverged sibling is re-associated via
  nose's **near** channel (fuzzy structural/value match), so we keep tracking *which*
  copy lagged and whether it later catches up.

A fingerprint split in a previously-converged family **is** the realized divergent
edit. This makes Type-4 genealogy tractable and is the dataset's foundation.

## Bug linking

For a G1 divergence to become **G2**, link the catch-up edit to a fault:

1. **Bug-fix commit identification** — the Mockus & Votta commit-message heuristic
   (~87% precision per Barbour; ICSM 2000), augmented with issue-tracker links where
   available.
2. **Fault attribution** — SZZ to connect the fix to the divergence that introduced
   the latent inconsistency.
3. **SPCP confirmation** — the catch-up is a *similarity-preserving co-change* that
   re-converges the family (Mondal & Roy, IST 2020).

All three are heuristics with known noise; their agreement is the gold-label
confidence (see [Quality controls](#dataset-quality-controls)).

## Separating accidental from intentional

Hazard concentrates in *unintentional* drift (Juergens ICSE 2009), but perfect
classification needs developer interviews. Use automatable proxies to keep G1/G2
clean of deliberate adaptation:

- **RESYNC:** a divergence later re-converged is evidence it was unintended (Mondal &
  Roy). G2 inherently has this; G1 keeps only persisting divergences.
- **Magnitude:** small, localized diffs (one parameter, one predicate, one exception —
  Krinke's "critical change" profile, WCRE 2007) are accident-like; large structural
  rewrites are adaptation-like and demoted.
- **Commit-message signal:** an explicit adaptation/refactor message on the diverging
  commit demotes it.

These are imperfect; the human-audited gold subset measures how often they misfire.

## Corpus policy

The dataset must be **large** and **balanced across two strata**, because the
literature is entirely single-language Java/NiCad and cross-language hazard is exactly
where nose is unique:

- **Stratum S (single-language):** families whose copies are one language.
- **Stratum X (cross-language):** families spanning ≥2 languages (nose's
  cross-language convergence). Rarer and concentrated in polyglot repos (client/server
  mirrors, ports, multi-language SDKs) — **over-sample such repos** to reach parity
  with S. Balance is achieved by *repo selection*, never by discarding data; S and X
  are always reported separately, never pooled in a way that hides per-stratum
  performance.

### Repo selection criteria

Pick repos by these *structural* rules, then **measure** clone/hazard prevalence —
never select a repo *because* it has known clone bugs (that biases the positive rate).

**Include** a repo only if it meets all of:

| criterion | threshold | why |
|---|---|---|
| Primary language | ∈ nose's supported [languages](languages.md); corpus spans **≥ 5** languages collectively | external validity beyond Java |
| History depth | **≥ 3 years** and **≥ 2,000 commits** | genealogies and faults need time to accumulate |
| Commit hygiene | **≥ 30%** of commits reference an issue tracker or match a bug-fix message pattern | Mockus & Votta + SZZ linking is only as good as the messages |
| Active maintenance | **≥ 3** authors, commits across the whole window (not a one-shot dump or mirror) | real divergent-edit opportunity |
| Clone yield | nose finds **≥ 100** multi-site families at HEAD | enough raw examples to track |
| License | permits redistributing derived metadata (line refs, labels) | the frozen dataset must be shareable |

**Exclude:** any repo used to tune nose's detector (leakage); generated/vendored-
dominated repos; squash-only or message-poor histories (break bug-fix linking);
shallow/young repos below the thresholds. Monorepos are scoped to a subdirectory
rather than excluded.

For **stratum X**, maintain a targeted sub-list of polyglot repos with mirrored logic
across languages, held to the same rubric.

> **Measured caveat (v1):** the X-stratum floor turned out to be **structurally
> unachievable**, not just hard. nose detects almost no *true* cross-language clones in
> real code — 37 of 15,199 families corpus-wide (2 ever-G2), and apache/arrow, a heavily
> polyglot repo, yields **0 cross-language families of 928**. The same logic in C++ vs
> Python rarely converges to one value-fingerprint. So the S/X balance goal is reported
> as a **measured limit**: tag a repo X only after confirming it yields cross-language
> families (`languages > 1`) — and expect very few. Define the stratum **per family**
> (`languages > 1`), never per repo (a polyglot repo is ~98% same-language clones). See
> [eval/hazard/RESULTS.md](../eval/hazard/RESULTS.md).

### How much is enough — quantitative sufficiency

Worked back from the sparsest label (G2): release-level harmful divergence is ~1–3%
of inconsistent changes (Bettenburg WCRE 2009), so a useful G2 count implies a large
G1 pool and many repos. The dataset is **sufficient** when *all* of these hold:

| quantity | floor | target | rationale |
|---|---|---|---|
| **G2** harmful-divergence events | **≥ 80** | **≥ 150** | enough positives for a stable PR-AUC and a precision@k CI within ±~10 pts |
| **G2 per stratum** (S and X) | **≥ 40 each** | **≥ 60 each** | per-stratum precision must be reportable, not just pooled |
| **G1** realized-divergence events | **≥ 1,000** | **≥ 2,000** | the dense weak-positive layer; ablation power |
| **Tracked family-genealogies** | **≥ 5,000** | **≥ 10,000** | the ranking denominator pool |
| **Repos** | **≥ 12** | **≥ 20** | enough to split by repo (below) and average out per-repo idiosyncrasy |
| **Human-audited gold subset** | **≥ 100** | **≥ 150** | estimate label precision to ±~8% and inter-annotator κ |

Given G2 ≈ 1–3% of G1, hitting **G2 ≥ 150** implies **G1 on the order of 5k–15k** —
so the repo count and history depth above are the real drivers; treat the G1/genealogy
floors as the lever, G2 as the binding outcome. X-stratum G2 is the *tightest*
constraint (cross-language clones are rarer): keep adding polyglot repos until X
reaches its per-stratum floor.

**Evaluation must be split by repo**, not by random example, to prevent same-repo
leakage and to measure external validity: calibrate weights on one set of repos,
report final precision@k / PR-AUC on **held-out repos** (e.g. 5-fold cross-repo, or a
70/30 repo split, with both S and X present in every fold).

### Stopping rule

Stop growing the corpus when, simultaneously: (1) G2 ≥ floor in **both** strata;
(2) the held-out-repo precision@k 95% CI is within **±10 pts**; and (3) cross-repo
variance of the headline metric is reported. Until all three hold, add repos
(prioritizing whichever stratum is short). These are the operational definition of
"a sufficient evaluation set."

## Metrics and protocol

### Granularity and the release rule

Label and evaluate at **release / surviving-edit granularity**, not raw revision
diffs: revision-level mining over-counts short-lived experimental clones that never
ship and inflates the hazard rate from ~1–3% to ~50% (Bettenburg WCRE 2009 vs
Juergens/Krinke). _Δ_ spans releases or a fixed multi-week window.

### Primary metrics

- **precision@k** — of the top-_k_ families ranked by `hazard()` at _T_, the fraction
  that are G2 (and, reported separately, G1∪G2) in _(T, T+Δ]_. The headline number.
- **PR-AUC**, not ROC-AUC — positives are sparse and imbalanced.
- **Ordinal AUC / Mann-Whitney** — do G2 families rank significantly above G1, and G1
  above G0? Validates the *graded* ranking, and treats a highly-ranked non-event as
  *ambiguous* (possibly latent) rather than a hard false positive.

### Baselines and ablation

Every claim is **relative**:

- vs **random**, **size-only** (`mean_sem`), and **extractability** — does hazard beat
  the trivial and the existing ranker?
- **signal ablation** — each term (invisibility, dispersion, copies, scope, later git
  DIVp) added/removed; report marginal precision@k lift. Expect size to dominate and
  evolution signals to add modestly (Barbour SQJ 2018: ~4.3%; Choi APSEC 2011: clone
  metrics help only on large modules) — so a signal that does not lift P@k is dropped.

## Dataset quality controls

The dataset is itself validated, not assumed:

- **Human-audited gold subset** — a random sample of G2 (and borderline G1) labels
  manually checked; report estimated **label precision** and inter-annotator
  agreement. Target precision comparable to the ~87% bug-fix heuristic or better.
- **Two-method agreement** — cross-check independent label routes (message heuristic
  vs SZZ; fingerprint-split vs near-channel re-link) and report concordance.
- **Built blind to `hazard()`** — labels are mined purely from VCS outcomes, never
  from the score's features, so the benchmark is an independent yardstick (no
  circularity).
- **Versioned and frozen** — v1 is a fixed artifact with documented statistics, strata
  balance, and known-noise estimates; the mining pipeline is reproducible (the
  [type4-benchmark](type4-benchmark.md) "factory" philosophy, applied to history).

## Versioning and refresh (coupling to nose)

The dataset has two layers with **different coupling to nose's version**:

- **Labels (G0/G1/G2) come from git history, not nose** — "which siblings changed
  inconsistently, and did it cause a bug" is a fact about the repo, independent of the
  detector. This is the durable, expensive-to-establish asset; it never needs
  re-deriving.
- **Features (`mean_lines`, `modules`, `mean_sem`, `params`, …) and the family set come
  from nose** — so the tuned `hazard()` weights are valid only for the detector version
  that produced them. Each event is stamped with `nose_ver` for provenance.

Consequence — **not every release forces re-tuning:**

| nose change | regenerate dataset? | re-tune? |
|---|---|---|
| detection (family definition, member sets, fingerprints, feature computation) | yes | yes |
| ranking only (`extractability`, `hazard()` itself) | **no** | no |
| performance / refactor with identical output | no | no |

The dataset is built from detection output + git, **never from ranking**, so changing
`hazard()` does not invalidate it. Only a change to detection output does.

Refresh is a fast, automated re-run (cached clones; the optimized miner re-scans and
re-labels in minutes), not a rebuild — the same harness *detects* whether a release
needs re-tuning by comparing weights across `nose_ver`. The **step-by-step release
procedure, decision table, and acceptance criteria** live in one place:
[hazard-release-checklist](hazard-release-checklist.md).

## Threats to validity

Carry these into every claim: SZZ and message-keyword heuristics have false
positives; fingerprint-split can miss a divergence whose fingerprint collides, or
mis-link via the near channel; the intentionality proxies misfire on subtle
adaptation; release-level mining still right-censors recent clones (too new to have
diverged). The human-audited subset bounds the first three; the forward window and
multiple _T_ samples bound the last.

## Build phases

- **A. This spec** — criteria, label schema, metrics, corpus policy. *(here)*
- **B. Mining pipeline** — nose as cross-revision linker: per-revision detect →
  fingerprint genealogy → split detection → bug-fix linking.
- **C. Corpus run** — execute on the balanced S/X corpus → large graded dataset.
- **D. Gold audit** — human-check a sample; estimate label precision; LLM-assist for
  latent cases, anchored per [Tier 2](hazard-ranking.md#evaluating-ranking-quality).
- **E. Freeze v1** — versioned artifact + statistics + threats.

## See also

- [hazard-ranking](hazard-ranking.md) — the score this benchmark evaluates, and the
  four evaluation tiers (this page is the objective Tier 1 core).
- [normalization](normalization.md) — the value fingerprint that makes Type-4 family
  tracking across revisions possible.
- [type4-benchmark](type4-benchmark.md) — the sibling synthetic benchmark; same
  reproducible-factory philosophy.
- [experiments](experiments.md) — where measured calibration/ablation results land.
