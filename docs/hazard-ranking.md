# Hazard ranking (divergent-edit risk)

> Status: **shipped as the default sort** (`--sort hazard`, the default; `--sort
> extractability` for the former one). The formula is calibrated against mined
> divergent-edit history ([eval/hazard/RESULTS.md](../eval/hazard/RESULTS.md),
> [experiments §BG](experiments.md)); the git-history Phase 2 and a human-audited gold
> subset remain. The parked cognitive-complexity sub-idea lives in
> [issue #23](https://github.com/corca-ai/nose/issues/23).

nose's default ranking, [extractability](usage.md#ranking), answers *"how cleanly
does this duplication fold into one helper?"* — a **fixability** axis. This page
designs a second, orthogonal **severity** axis: *"how likely is this clone to be
edited inconsistently and cause a bug?"* — the **divergent-edit hazard**.

## Why a separate axis

The reason duplication is a smell is the maintenance hazard: a developer fixes one
copy, misses the siblings, and ships a bug. Extractability does not measure this —
and is in fact *partly anti-correlated* with it. Its `tightness = shared_weight /
rep` term rewards copies that share a lot of literal text. But the most dangerous
clones are **semantically identical yet syntactically divergent** (Type-4) — the
sibling is invisible to grep and to the developer's eye — and those share little
text, so extractability sinks them. The hazard axis must do the opposite: its
invisibility term is literally `(1 − tightness)`. nose's Type-4 reach is exactly
the high-hazard zone other tools cannot see, so a hazard rank is where that reach
pays off.

## Evidence base

Two literature sweeps (peer-reviewed, ICSE/FSE/WCRE/ICSME/MSR/EMSE/SQJ/IST/TSE,
2005–2025; claims adversarially verified). The backbone:

- **Inconsistent clone edits are common, not rare.** ~52% of clone groups contain
  inconsistencies (Juergens et al., *Do Code Clones Matter?*, ICSE 2009);
  independently, only 45–55% of changed clone groups change consistently (Krinke,
  WCRE 2007).
- **Divergence does not self-heal.** Widening the observation window 1→10 weeks
  leaves the consistent-change rate flat — missed sibling edits are rarely
  propagated later (Krinke, WCRE 2007). Divergence is a durable state, worth
  flagging.
- **Hazard concentrates in *unintentional* drift.** ~28% of inconsistent edits are
  unintentional, and ~50% of *those* become faults (107 developer-confirmed faults;
  Juergens, ICSE 2009). The hazard is not uniform — it lives in the accidental
  subset.
- **But the absolute release-level rate is low (1–3%).** Revision-level studies
  over-count short-lived experimental clones that never ship (Bettenburg et al.,
  WCRE 2009). **Consequence for us:** validate at release / surviving-edit
  granularity, or the hazard score looks inflated.

The directional signals, validated:

- **Diverging/inconsistent change > consistent change** (odds ratio > 1, Fisher's
  exact, p < 0.05). The headline pattern is **DIVp** — an unpropagated one-sided
  edit. Faults also concentrate among clones in **different directories**, so
  cross-directory dispersion is a validated hazard feature (Barbour, An, Khomh,
  Zou, Wang, *Fault-proneness of clone evolutionary patterns*, Software Quality
  Journal 2018).
- **Near-miss > identical.** Type-2/Type-3 clones are more bug-prone and propagate
  bugs at higher intensity than identical Type-1; Type-3 is highest (significantly
  above Type-1, MWW p = 0.026). The strict Type-3 > Type-2 > Type-1 chain is **not**
  fully validated — treat it as *near-miss > identical*, not a total order (Mondal,
  Roy, Schneider, ICSME 2015 / IST 2020).
- **Size and churn dominate; evolution history is a weak add-on.** Genealogy/
  evolution features add only ~4.3% incremental deviance over product+process
  metrics — size (CLOC), churn (added+changed LOC), and historical fault density
  carry most of the signal (Barbour et al., SQJ 2018). And naive clone-count metrics
  improved fault prediction *only for large modules*, not small ones (Choi, Yoshida,
  Higo, Inoue, APSEC 2011 — a negative result). **Consequence:** make size a primary
  multiplier and size-gate the score; do not over-weight the clever signals.

Recent ML work confirms the target is real and learnable, but is framed as
*consistency/propagation* prediction, not a fault-ranked hazard: **CloneRipples**
(EMSE 2024) predicts pairwise whether an edit must propagate to a specific sibling
(Fused Clone PDG + R-GCN; P 83.1 / R 81.2 / F1 82.1 on 51 Java projects);
**CHANN** (JCST 2023) and Yan et al. (*Information Sciences* 2023) predict clone
"consistent-defects" from learned code+context+evolution features (~80–82% F,
~90% within-project recall, but <50% cross-project). None outputs a calibrated
hazard *score* for **semantic/Type-4** clones evaluated by precision@k/NDCG against
bug ground truth — that is the gap, and the heavyweight GNN approach is a poor fit
for nose's fast, static, single-binary product anyway. We use the **handcrafted
feature menu** instead.

## Signal menu

Each signal below has a validated direction and a home in nose's existing
`RefactorFamily` fields ([architecture](architecture.md)). "Phase 1" = computable
from a static snapshot today; "Phase 2" = needs git history.

| Signal | Direction (↑ hazard) | Evidence | nose field | Phase |
|---|---|---|---|---|
| **Behavior size** | larger duplicated behavior | Barbour (size dominates); Choi (only large modules) | `mean_sem`, `mean_lines` | 1 |
| **Invisibility** | identical behavior, *little shared text* | Mondal/Roy (near-miss > identical); Saha (Type-3 inconsistent ≈5:1) | `1 − tightness` (`shared_weight`/rep) | 1 |
| **Latent divergence** | few, small, already-differing spots | Krinke (critical changes: differing params/predicates/exceptions) | `params` (peak at small >0, saturate when many) | 1 |
| **Cross-directory dispersion** | copies spread across directories | Barbour (faults concentrate across directories) | `modules`, `spread()` | 1 |
| **Copy count** | more siblings to miss | Juergens/Bettenburg (coordinated-update obligation) | `members`, `effective_copies()` | 1 |
| **Blast radius** | prod over test | existing test-awareness policy | `scope` | 1 |
| **Realized divergence (DIVp)** | siblings *already* edited apart | Barbour (DIVp = headline); Mondal/Roy (SPCP) | git: per-copy last-edit commit | 2 |
| **Unintentional proxy (RESYNC)** | divergence later re-synced ⇒ was accidental | Mondal/Roy (re-synchronizing change) | git history | 2 |

Deliberately **excluded**: a cognitive-complexity per-copy edit-surface metric (no
evidence for clone hazard — parked in [#23](https://github.com/corca-ai/nose/issues/23));
the strict clone-type total order (refuted); heavyweight ML embeddings (product
mismatch).

## Score design

The formula below is **calibrated against mined ground truth**, not guessed: a
leave-one-repo-out evaluation over a 12-repo / 8-language corpus (462k family-interval
events, 4,639 divergent edits G1, 181 bug-linked G2 at ~1.1% prevalence — see
[eval/hazard/RESULTS.md](../eval/hazard/RESULTS.md) and [experiments](experiments.md)).
The data **overturned the pre-data design**: leading with semantic size (`mean_sem`)
was *anti-predictive*; source-line span, dispersion, and invisibility are the real
signals.

**Phase 1 — static hazard, from fields nose already computes** — `hazard()` beside
`extractability()` in `crates/nose-detect/src/report.rs`, plus `SortKey::Hazard` (now
the default). No new detection logic. Measured formula (leave-one-repo-out AUC:
G1 0.644, gold-G2 0.704; vs 0.609 / 0.668 for the size-led design it replaces):

```
hazard = mean_lines                        // magnitude — source-line span (+0.43)
       × spread(files, modules, languages) // dispersion — cross-directory (+0.28), existing helper
       × invisibility                      // 0.3 + 0.7·(1 − tightness) (+0.14); dominant in cross-language
       × scope_weight                      // prod 1.0 / mixed 0.5 / test 0.25
```

Note what the data changed from the first draft: **`mean_sem` is dropped** (its learned
weight is −0.27 — larger semantic fingerprints diverge *less*); `mean_lines` replaces it
as the magnitude term; **`params` is not used** (its weight is sign-unstable across
corpus sizes — noise). `invisibility` survives and is the **top signal in the
cross-language stratum** (AUC 0.67, P@10 0.80) — the Type-4 hypothesis held exactly
where predicted.

This is **the default sort**. The weights are calibrated on divergence *propensity*
(G1, leave-one-repo-out AUC 0.644) and validated against bug-linked divergence (G2,
function-level attribution, ~1.1% prevalence matching the literature, AUC 0.704) over
the 12-repo corpus — it beats the former size-led/extractability default on both
labels. `--sort extractability` remains for the fixability axis. Still open before the
weights are considered final: a human-audited gold subset and the git-history Phase 2.

**Phase 2 — git-history realized divergence (the high-precision payload).** Because
nose matches Type-4, it can link siblings across revisions where textual tools
(NiCad/CCFinder/iClones) lose them. Two steps:

- **2a (cheap):** at scan time, compare each copy's `git blame` last-edit commit /
  time. Copies last touched in *different* commits = maintenance attention has
  already diverged — a DIVp proxy with no full genealogy.
- **2b (full):** track families across history; detect Kim-style *Inconsistent
  Change* (one sibling changed, others not) and SPCP co-changes. This yields a
  concrete, checkable finding — *"this family was edited apart N times"* — not a
  fuzzy score. Higher precision than any static signal.

Intentional vs. accidental cannot be classified perfectly (Juergens needed
developer interviews). Use **divergence magnitude** as a proxy (small, subtle diff
= flag; large, structural diff = suppress) and the **RESYNC** signal (a divergence
later converged was probably unintended), and let users dismiss false positives via
[structured ignores](structured-ignores.md).

## Implementation plan

**Phase 1 — static hazard score. ✅ Done (shipped as the default).**

1. ✅ `hazard()` on `RefactorFamily` in `crates/nose-detect/src/report.rs`,
   implementing the calibrated formula above. Reuses `spread()`; `invisibility`
   (`0.3 + 0.7·(1 − tightness)`, `tightness = shared_weight / mean_lines`) and
   `scope_weight` (prod 1.0 / mixed 0.5 / test 0.25) inline. Magnitude is `mean_lines`
   — **not** `mean_sem` (the data showed semantic size is anti-predictive; this also
   softly demotes the tiny dense functions the `min-tokens` gate cannot, see
   [normalization](normalization.md)).
2. ✅ `SortKey::Hazard` in `crates/nose-cli/src/main.rs`, wired into `score()`,
   `sort_name()`, the `--sort` value list, and `capabilities` — and made **the
   default**.
3. ✅ Tier-0 contract unit tests in `report.rs` (divergent > tight under hazard and the
   reverse under extractability; cross-language ranks high; test scope demoted).
4. ✅ Docs: [usage](usage.md#ranking) ranking table, scan-json, capabilities, README.

No change to detection, normalization, or the value graph.

**Phase 2 — git-history realized divergence + calibration.**

6. **2a (cheap):** read each copy's `git blame` last-edit commit/time; flag families
   whose copies were last touched in different commits (a DIVp proxy) without full
   genealogy.
7. **2b (full):** the Tier-1 pipeline below — track families across revisions, label
   realized divergence, and **calibrate** the Phase-1 constants (`scope_weight`, the
   size-vs-invisibility balance, whether to add the `params` term). Until this runs,
   Phase-1 weights are provisional.

## Evaluating ranking quality

"Good" means: families ranked high are the ones actually likely to be edited
inconsistently and cause a bug. Hazard is partly **latent** — a clone can be
dangerous before it has ever been mis-edited — so no single instrument suffices. Use
four tiers, anchored by an objective outcome. **Never let an LLM judge be the primary
arbiter:** it scores *opinion*, not *outcome*, and will agree with our own score
circularly if it reasons the same way.

**Tier 0 — sanity (unit + synthetic), in CI.** Pin the defining contract as
`report.rs` unit tests: for two same-size families, a semantically-identical-but-
syntactically-divergent one (low tightness) ranks **above** a tight near-identical
one under `hazard()`, and **below** it under `extractability()`; a cross-language
family ranks high; a test-scope family is demoted. Extend the
[type4-benchmark](type4-benchmark.md) factory with families carrying an injected
subtle divergence and check they surface. Cheap, catches gross errors — but synthetic
≠ real distribution.

**Tier 1 — git-mined realized incidents (the primary, objective instrument).** The
full specification — graded label schema, repo-selection rubric, quantitative
sufficiency criteria, and dataset quality controls — lives in
[hazard-benchmark](hazard-benchmark.md). In outline, adapt the established clone-
genealogy / SZZ pipeline (Mondal/Roy SPCP-Miner; Kim et al. CloneGenealogyExtractor,
MSR 2005; gCad, ICSM 2011), with **nose as the Type-4-aware linker**, as a
*forward-prediction* split (no leakage):

1. Detect families with nose at repo state *T* (the Type-4 step textual tools cannot
   do).
2. Track families across revisions in *(T, T+Δ]*; diff consecutive revisions and map
   changed lines to families.
3. Label **divergent edits** — a sibling changed while others did not (Kim
   *Inconsistent Change*) — and **bug-propagation** via SPCP co-change (Mondal/Roy,
   IST 2020).
4. Identify **bug-fix commits** by the Mockus & Votta message heuristic (~87%; ICSM
   2000) + SZZ for fault attribution.
5. Score families at *T*, evaluate against the *(T, T+Δ]* labels by **precision@k /
   PR-AUC** (not accuracy — release-level positives are 1–3%, heavily imbalanced), as
   an **ablation**: does each signal lift P@k over a size-only baseline? Evaluate at
   **release / surviving-edit granularity**, not raw revision diffs (Bettenburg).

This is the only non-circular, outcome-based ground truth and the basis of any
quality claim. Limits: label noise (SZZ/message heuristic ~87%), sparse positives,
and it sees only **realized** hazard — a dangerous clone that has not diverged *yet*
looks negative.

**Tier 2 — LLM judge (covers the latent hazard Tier 1 cannot label).** For
forward/latent risk and findability (axis A), under strict rules:
- **Anchor first:** validate that the judge's ranking agrees with Tier-1 labels on
  the overlap (Kendall τ / pairwise accuracy) *before* trusting it on latent cases.
- **Blind & outcome-framed:** give it only the raw copies and ask it to predict the
  *outcome* ("if a developer fixes a bug in copy A, how likely are they to miss copy
  B?") — never show it our features or which family scored higher.
- **Pairwise, position-swapped:** prefer "which of these two is more hazardous?" over
  1–5 pointwise scores; run both orderings to cancel position bias.
- **Guard the size shortcut:** test on size-controlled pairs — an LLM tends to
  conflate "large/complex" with "hazardous".
- Bonus use: **bootstrap labels** (LLM proposes, human spot-checks) to grow the
  Tier-1/Tier-3 datasets faster — still validated against Tier 1.

**Tier 3 — developer P@10 on real projects (cheap field signal).** The analog of the
dev-P@10 check that validated extractability ([field-evaluation](field-evaluation.md)):
show the top ~10–20 hazard families from real third-party repos to a developer and
ask "is this a divergence risk you'd want flagged?" Real distribution, no LLM bias,
low cost.

**Threats to validity** (carry into any claim): SZZ and message-keyword heuristics
have known false positives; mapping fragments across edits produces false genealogies;
the validated feature menu comes largely from one group (Mondal/Roy/Schneider), one
toolchain (NiCad), and ~7–9 mostly-Java systems — none targeting Type-4 — so applying
it to semantic clones is an extrapolation we must measure, not assume. Measured
results land in [experiments](experiments.md).

## See also

- [hazard-benchmark](hazard-benchmark.md) — the evaluation criteria and dataset this
  score is calibrated against.
- [hazard-release-checklist](hazard-release-checklist.md) — what to do for this score on
  every new nose release.
- [usage](usage.md#ranking) — the user-facing ranking keys (`hazard` default,
  `extractability`, `value`, `sites`).
- [field-evaluation](field-evaluation.md) — why extractability replaced raw value
  as the default (the fixability axis this page complements).
- [architecture](architecture.md) — the lower → normalize → detect → rank pipeline
  and where ranking sits.
- [clone-types](clone-types.md) — the Type-1/2/3/4 taxonomy the near-miss > identical
  evidence refers to.
