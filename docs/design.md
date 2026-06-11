# nose — design & direction

This document records the *why* behind nose's direction: what the product fundamentally is,
who actually consumes it, and how those two facts decide where engineering effort goes. It is
the strategic companion to the operational [documentation home](home.md); the
[architecture](architecture.md), [normalization](normalization.md), and
[formal soundness](formal-soundness.md) pages describe *how* the machine works.

It is intentionally short and opinionated. When a roadmap decision is unclear, this is the
document it should be checked against.

---

## 1. The sound core is the moat

nose's defensible differentiator is **not** "finds more clones." It is a guarantee that
almost no other clone detector makes:

> **If nose reports two pieces of code as semantically equivalent, they really are —
> exactly, never a false positive on the equivalence claim.**

Concretely, the contract is *equal fingerprint ⟹ equal behavior*. A **false merge** (two
behaviorally-different fragments sharing a fingerprint) is a **bug**, not an accepted
approximation. This is enforced by, in layers:

- the **exact `semantic` channel**: acceptance is total value-graph multiset equality
  (`crates/nose-detect/src/lib.rs` — `a.value == b.value`, length-gated), over a hash-consed
  value graph that canonicalizes behaviorally-equivalent code to identical structure;
- a **static** safety gate (`strict_exact_safe_tree`, `crates/nose-detect/src/strict_exact.rs`) on
  the accept path — no interpreter in the hot path, fully deterministic;
- **per-rule Lean obligations** for the proof-sensitive canonicalizations
  ([formal soundness](formal-soundness.md));
- an **offline** interpreter oracle (`nose verify`) used as a differential-testing harness —
  *not* a runtime acceptance gate;
- **adversarial per-rule batteries** that defend the guarantee as the rule set grows (this is
  where real trust comes from — a corpus oracle once read clean while latent false merges
  existed, found only by adversarial crafting).

Three properties are part of the moat and are **non-negotiable**:

- **Soundness** — zero false merges. Every recall extension must preserve this.
- **Determinism** — byte-identical output across runs and thread counts.
- **Speed & self-containment** — one fast binary, no required network or services.

Everything below is downstream of protecting and sharpening this core.

---

## 2. The two consumers (nose is not read by a human)

The clone-detection literature assumes a human reads a ranked report. **nose's primary
consumers are not human.** This single fact reshapes the roadmap.

### Consumer 1 — LLM coding agents

Tools like Codex and Claude Code that **call nose** as part of their own loop. (To be precise:
the LLM lives in the *caller*; nose does **not** embed an LLM.)

The deep, judgment-heavy question — *"is this duplication actually worth refactoring, or is it
parallel-by-design / locale tables / intentional repetition?"* — is answered by the **calling
agent's own LLM**. nose re-deriving that judgment internally is redundant.

What nose owes consumer 1:

- **High recall** — surface the candidates; the agent filters cheaply.
- **Good-enough, deterministic ranking** — to triage and save the agent's tokens, not to be
  a perfect worth-it oracle.
- **Rich machine-readable evidence** — the [scan JSON](scan-json.md) output should carry
  *why* two units are equivalent, *what* differs, exact locations, and the behavior contract,
  so the agent can decide and act without re-deriving the analysis.
- **Speed.**

Implication: **perfectly separating parallel-by-design is not specially important here** — the
consumer does that. Investing nose's own bandwidth in that judgment is low-leverage.

### Consumer 2 — automated gates

CI/CD runners and pre-commit hooks that use nose as a **bottom-line that can force a stop**
(fail the build, block the commit).

A gate that cries wolf gets disabled. So the requirement is inverted from a search tool:

- **Very high precision — even at very low recall.** Missing many is fine; firing wrongly is
  not. What it *does* fire on must be undeniable.
- **Determinism is mandatory** — a flaky gate is worse than no gate. An LLM in the path is
  therefore *harmful* here.

This requirement **is the sound core.** The CI-gate product and the moat are the same thing.
The [review](review.md) signal — a clone copy fixed while its siblings were missed in a
diff — is a natural high-precision, actionable gate trigger.

### One core, two operating points

| | **Consumer 1 — agent-feed** | **Consumer 2 — gate** |
|---|---|---|
| recall | higher is better (agent filters) | low is fine |
| precision | good enough (deep judgment outsourced) | **must be extremely high** |
| output | rich JSON + equivalence evidence | pass / block + `--fail-on` |
| ranking | triage aid (saves agent tokens) | mostly irrelevant (gates don't rank) |
| LLM | in the *caller* (external) | none (determinism required) |
| shared | **soundness · determinism · speed** | same |

Both ride the same sound core. They differ only in operating point and output contract.

---

## 3. What this means for priorities

**Raise (high certainty, serves the moat and/or a real consumer):**

- **Institutionalize adversarial per-rule batteries.** zero-false-merge is the premise both
  consumers depend on; this is what makes the guarantee scale as rules are added.
- **scan-json evidence richness.** The real lever for consumer 1 — make equivalence
  *explainable and actionable* in machine-readable form.
- **`review`-as-gate.** The natural high-precision bottom-line for consumer 2; harden it past
  v1 and define a conservative fire policy. *Measured 2026-06-11
  ([experiments §BR](experiments.md)): on replayed merged PRs the default arm fires on 33%
  of changes at ~4% top-1 strict precision — real catches exist but the fire policy work
  is mandatory before any default-on gate; half the noise is one mechanical bucket
  (span-level overlap), the first policy lever.*
- **Determinism** as a sacred invariant — now wanted by *both* consumers.

**Keep / conditional:**

- **Recall-extending sound work** (new sound canonicalization rules; possibly bounded
  pure interprocedural inlining and anchored sub-DAG matching) — valuable for consumer 1,
  since the agent filters. **Hard constraint: it must never break zero-false-merge**, and
  recall-extension is gated on measuring that real missed-worthy pairs exist to recover.
  *Measured 2026-06-10 ([experiments §BJ](experiments.md)): the residual beyond the
  shipped v1 mechanisms is small (sub-DAG ceiling 2.0% optimistic / 0.6% at the shipped
  anchor weight; inlining 0.3%) — further effort routes to unit-extraction coverage and
  the fragment axis, not more matching.*
- **Extractability ranking** — a "good enough" deterministic triage signal; no need to chase
  more.

**Lower (demoted):**

- **An internal LLM-judge re-ranker.** Redundant for consumer 1 (their LLM does it) and
  harmful to consumer 2 (breaks determinism). Demoted, not because precision doesn't matter,
  but because the *internal* model is the wrong place to spend it.

---

## 4. Open question — the e-graph / equality-saturation substrate

**Status: deferred. Not decided. Revisit later.**

A proposal to replace the hand-ordered canonicalizer with an equality-saturation (e-graph)
engine over a single formal IL semantics was evaluated in depth. The current evidence points
**against** doing it now:

- the existing canonicalizer (`mk`) already reaches the rewrite fixpoint in practice —
  measured: 6/7 phase-ordering-stressing equivalences already converge, the 7th closed by
  *one* new rule, not an engine ("the lever is new sound rules, not a better engine");
- the fingerprint is a whole-DAG node-hash multiset (`fingerprint_lits` in
  `crates/nose-normalize/src/value_graph.rs`); cost-based extraction from two non-isomorphic
  e-graphs could pick different representatives and **break currently-converging matches**,
  and threatens the byte-determinism invariant;
- behavioral equivalence is **not a congruence** under the IL's ordered effects (the oracle
  compares an ordered effect trace), so a congruence-closure merge is unsound except on a pure
  sub-IL — at which point it largely reinvents the existing normal form;
- performance: the canonicalizer is single-pass today and is not the dominant scan cost
  (per-unit value-graph extraction is, now that the evidence passes are index-backed —
  [experiments §BQ](experiments.md)); saturation is super-linear and determinism-hostile.

It is recorded here as an **open question, not a closed door.** Conditions that would justify
revisiting:

- new-rule maintenance hits a real wall (rule interaction / phase-ordering becomes the
  dominant source of *missed* sound equivalences, measured — not assumed);
- a determinism-preserving normal-form extraction is shown to keep today's matches; and/or
- the recall frontier shifts such that compositional equivalences the fixed order cannot reach
  become a measured, material loss.

Until then, prefer **adding individually-proven sound rules** over re-platforming the engine.

---

## 4b. The coverage co-evolution loop (implemented)

The adversarial recall/soundness co-evolution is now a running, deterministic loop in
`bench/type4/`, expanding an explicit `(axis × language × {recall, soundness})` coverage
matrix evenly instead of by prevalence:

- **`coverage_taxonomy.py`** — the controlled axis vocabulary, incl. the high-value
  *structural* axes the old queue never tracked (extract-method inline, partial sub-DAG,
  recursion↔iteration extended, statement-window) + explicit out-of-scope rows.
- **`coverage_matrix.py`** — `matrix` (the grid + evenness gauge), `next` (a coverage-aware
  *cell* dispenser: gap term + fairness floor — fixes the old `type4-next` axis-atom +
  static-prevalence bias that produced a diagonal, language-skewed matrix), `soundness` (the
  soundness arm).
- **`coverage_sweep.py`** — runs each generatable axis through nose per language AND through
  the interpreter oracle (`nose verify`). One run advances **both arms**: positive recall +
  generator hard-negatives + oracle under-merged leads + completeness. **Strengthening the
  oracle is part of every sweep**, not a separate pass.
- **`coverage_probe.py`** — checked-in positive + adjacent hard-negative pairs for axes the
  generator can't make; each positive must converge, each hard-negative must stay un-merged
  (the soundness guard). Block sub-units are skipped (a bare loop with no escaping effect is a
  vacuous no-op — its collision is sound, not a clone).

**Soundness co-evolves with recall by construction**: no axis is "done" without a
hard-negative guard, and the oracle runs on every sweep (0 merged hard-negatives across all
swept axes; the real-corpus 0-violation gate is the nightly pinned-corpus
`nose verify --max-violations 0` workflow).

The battery has already paid off — it surfaced a systematic **`exact_safe` language
asymmetry** (recursion / builder loops / java stream-reduce admitted to the exact channel in
some languages but not others — [bench/type4/coverage_leads.md](../bench/type4/coverage_leads.md)),
the concrete next implementation queue for *even* cross-language coverage.

## 5. Decisive measurements (run before betting heavily)

Cheap experiments that turn direction into data:

- **Latent false-merge adversarial sweep** — run §AS-style adversarial batteries against the
  current rule set. Anything found makes "institutionalize batteries" urgent. *(Validates the
  moat both consumers depend on.)*
- **Recall-ceiling probe** — on the gold set, how many *missed worthy* pairs would
  largest-common-pure-sub-DAG matching (and helper inlining) recover? If small, recall
  extension survives only as sound-rule work, not as a headline. *(Gates recall-extension for
  consumer 1.)* **Ran 2026-06-10 — answer: small.** [experiments §BJ](experiments.md):
  worthy-recall at the maximal current surface is 94.3% dev / 96.4% heldout; the
  generalized sub-DAG ceiling is 2.0% (0.6% at the shipped anchor weight), inlining 0.3%,
  and the remaining misses are unit-extraction gaps, statement-window fragments, and
  zero-shared-mass judgment cases. Follow-up experiments closed most Ruby test-DSL block
  misses and part of the Rust `macro_rules!` arm gap; see [experiments §BN](experiments.md)
  and [§BO](experiments.md).
- **Byte-determinism stress** — diff `nose scan --format json` across thread counts on a large
  repo. Any difference is a hard-invariant violation. *(Protects both consumers.)*

---

*See also: [architecture](architecture.md) · [normalization](normalization.md) ·
[formal soundness](formal-soundness.md) · [clone types](clone-types.md) ·
[scan JSON](scan-json.md) · [review](review.md).*
