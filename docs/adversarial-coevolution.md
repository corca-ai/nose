# Adversarial co-evolution — the cross-axis campaign loop

The operating procedure for an **adversarial co-evolution campaign**: a white-box
attacker derives patterns the current implementation *structurally cannot* handle, an
assessor prices which of them matter, and a defender responds with the largest **sound**
generalization — never a case patch, never past what can be proven or decided. One
campaign is a bounded unit of work an agent can execute end-to-end; "run adversarial
co-evolution" means executing the protocol below once.

This generalizes the method that already guards the moat. Soundness has used white-box
adversarial crafting since [experiments §AS](experiments.md) (a corpus oracle read clean
while latent false merges existed — only crafted attacks found them), and the
[design §4b coverage loop](design.md) co-evolves recall and soundness by construction.
This page extends the same paradigm to **every claim nose makes**: recall (missed sound
equivalences), the [decidability-boundary filters](design.md) (§2b), ranking/grouping/
hint claims, and the oracle's own completeness.

## The three parties

A two-party attacker/defender game degenerates; the log proves it twice. Num-gated
doubling was a perfect *structural* attack that priced at **zero** real occurrences
([experiments §BW](experiments.md): 0 of 163 behavior-equal split pairs), and the
behavior-keyed mining arm found a real frontier worth **one** pair per 105 repos
([§BS](experiments.md)). An attacker can generate unboundedly many "missable" patterns;
without pricing, the defender is dragged into rule bloat — every rule is maintenance, a
proof obligation, and false-merge risk surface. Hence three parties:

| party | job | output |
|---|---|---|
| **Attacker** (white-box) | read the implementation and its "stays closed" lists; derive patterns the structure cannot represent; strongest move: *structure-guided corpus mining* — use the implementation to aim, real code to load | target packets |
| **Assessor** | price each packet: prevalence on the pinned corpus, worthiness under the [§BG-gold judge discipline](experiments.md) (judge + two adversarial refuters; human arbitrates splits) | `priced` / `rejected: no-prevalence` / `rejected: not-worthy`, with artifacts |
| **Defender** | for priced packets only: the **largest sound generalization** — generalize exactly up to what can be proven/decided, no further ([§BA](experiments.md): untyped `-(-x) → x` caught 17 false merges; the sound response was type-gated rewriting) | a PR through the full ship gates |

An attack **counts only if it prices**. The attacker's fitness function is priced misses,
not misses.

### Attacker modes (added by series 2)

The attacker is a **fresh subagent**, not the session that authored the surface: spawn
it with the surface's source, the claim sentence, and the packet format — and nothing
else. Authoring context is attack-bias (series 1's Ruby modifier-if hole survived the
author's own C1 sweep and fell only to the boundary re-attack); a clean context removes
exactly that bias.

- **Blind mode** (the default): the attacker must NOT be shown the existing tests,
  batteries, or the author's reasoning — anchoring on tested cases reintroduces the
  staleness the isolation exists to remove. Inputs: source of the surface, the claim
  with its doc pointer, the packet format.
- **Informed mode**: a separate attacker gets the test suite *as the target* and
  attacks its coverage — untested language rows, boundary values, asymmetries between
  the yes and no fixture sets. Blind and informed find different things; keep their
  packet pools separate until assessment so neither anchors the other.
- **Persona rotation**: vary the attacker's stance per campaign — grammar lawyer,
  performance attacker, soundness skeptic, language specialist, docs-vs-code auditor —
  and record in the ledger which persona found what, so the rotation itself can be
  tuned on evidence.
- **The limit, unchanged**: subagents share the base model's priors. Isolation removes
  *authoring-context* bias, not *model* blind spots — the mechanical generator
  ([design §4b](design.md)) and the distribution adversary ([design §2c](design.md)
  fresh-repo audits, field feedback) remain the decorrelation arms that no amount of
  subagent spawning replaces.

## Attack surfaces

Rotate campaigns across these; each entry names what to read and what claim to attack.

1. **Canonicalizer rules** — `crates/nose-normalize/` and
   [normalization](normalization.md). Claim: behaviorally-equivalent forms converge.
   Attack: equivalences no rule chain reaches (note: packets showing *compositional*
   equivalences the fixed rule order cannot reach are the measured trigger for the
   [e-graph revisit conditions, design §4](design.md) — tag them `compositional`).
2. **Exact-channel gates** — `strict_exact.rs`, exact-channel eligibility in
   [semantic-kernel](semantic-kernel.md). Claim: fail-closed gates only exclude, never
   wrongly admit. Attack both directions: admissible-but-excluded (recall) and the gate's
   fail-open edges (soundness).
3. **Fragment contracts** — [fragment-contracts](fragment-contracts.md) and the explicit
   "stays closed" lists there and in [usage](usage.md). Attack: (a) verify closed things
   are closed, (b) find *unlisted* closed things — the unknown unknowns.
4. **Oracle completeness** — `nose verify` bail conditions ([benchmark](benchmark.md),
   [§BL census / §BU](experiments.md)). Claim: bails are fail-closed and bounded. Attack:
   interpretable shapes the bail taxonomy misclassifies.
5. **Decidability filters (§2b)** — `declaration_run_ids` and successors in
   `crates/nose-cli/src/main.rs`, [experiments §BY](experiments.md). Claim: "provably no
   extraction exists". Attack: spans the classifier accepts that a maintainer *would* act
   on, and fail-open edges that leak.
6. **Ranking / grouping / hints** — extractability, opportunity grouping
   (`OpportunityGroups`), the existing-helper and high-parameter hints
   ([usage → Ranking](usage.md)). Claims are per-feature and decidable; attack their
   stated conditions (e.g. a family that groups but is two genuine opportunities).
7. **The clone-type claims** — [clone-types](clone-types.md) honest limits. Attack: a
   limit statement that is no longer true (stale fences are silent recall loss).
8. **Performance & determinism claims** (added by series 1, [§BZ](experiments.md)) —
   the moat's speed/determinism legs ([design §1](design.md)). Attack: inputs whose
   shape concentrates cost (few huge files serialize per-file parallelism — the §BH
   class; §BZ measured 3.1 s on two 4.8 k-line files vs 0.63 s on a 1,364-file repo),
   super-linear presentation-layer passes (per-member file re-reads, per-file pair
   blowups), and byte-determinism under repeated runs and `RAYON_NUM_THREADS`
   variation. Pricing for this surface IS the measurement (`NOSE_TIME=1` per-stage
   breakdown); fixture-generation note: vary token *shape* in synthetic filler, or
   Type-2 identifier abstraction bridges your blocks into one run.

## Target packet format

Reuse the [frontier platform](frontier-platform.md) evidence shape
(`real_frontier.v1.json`) and the task-card style of
[type4-adversarial-coverage](type4-adversarial-coverage.md):

- `case_id`, `surface` (one of the list above), `claim` (the exact sentence/invariant
  attacked, with code/doc pointer);
- `construction` — minimal pair/fixture, plus the equivalence/behavior evidence
  (constructed ground truth where the axis is formal; oracle run where applicable);
- `hard_negative_siblings` — the adjacent forms that must NOT change behavior under any
  defense (the soundness guard travels with the attack);
- `realism` — corpus instances if mined (file/line), else `synthetic`;
- `tags` — e.g. `compositional`, `judgment-axis`.

## Campaign protocol

1. **Scope.** Open a tracking issue; pick 3–5 surfaces (rotate from the last campaign's
   closeout). Record the commit attacked.
2. **Attack.** Per surface, an agent reads the code + docs and emits target packets.
   Synthetic constructions are acceptable for formal axes (ground truth comes free);
   judgment-axis packets should prefer mined real instances.
3. **Price.** Prevalence: count real-corpus occurrences of the packet's shape
   (`bench/repos`, grep/feature/oracle arms as fits — the [§BW](experiments.md)
   precedent). Worthiness (judgment-axis only): judge + two refuters, human arbitration
   on splits. Reject without prejudice anything unpriced; record the rejection — an
   evidence-backed rejection is a result ([§BW](experiments.md) re-rejected doubling and
   that *was* the deliverable).

   **The claim-violation asymmetry** (series 1, [§BZ](experiments.md)): pricing gates
   *recall-direction* attacks ("nose should also detect X"). An attack that breaks a
   **"provably…" claim itself** — a §2b filter classifying real code, a hint giving
   unsafe advice, a false merge — is soundness-class and is fixed at ANY prevalence,
   exactly like a false merge. Every C1/C2/C5 hit in series 1 was this kind.
4. **Defend.** Priced packets only. The defense is the largest sound generalization, and
   it ships through ALL of: adversarial battery including the packet's hard negatives;
   Lean obligation if proof-sensitive ([formal-soundness](formal-soundness.md));
   corpus label join showing zero worthy-label loss (the
   [eval/declaration_runs](../eval/declaration_runs/RESULTS.md) precedent); the
   zero-false-merge and determinism gates ([CONTRIBUTING](../CONTRIBUTING.md)).

   **Defense-deferral is a first-class verdict**: a priced packet whose sound defense
   exceeds the campaign's scope (detector-core work, new proof obligations) closes as
   `deferred: #issue` with the packet and measurement attached — series 1 produced
   two (#269 few-huge-files serialization, #270 law-provenance gating). An attack
   **refuted by an existing sound gate is a green result with teeth**: record what
   refuted it (series 1's clamp escalation was refuted five-for-five and the refutation
   trail explained the LawPack zero-provenance field mystery).
5. **Boundary re-attack.** One round: attack the new generalization's edges (its type
   gates, fail-open conditions, thresholds) before the campaign closes — the
   doubling → type-gating cycle ([§AY/§BA](experiments.md)) is the model.
6. **Record.** An experiments.md section per campaign with the packet ledger and
   verdicts; artifacts checked into `bench/` or `eval/`; update any "stays closed" list
   the campaign changed; close the tracking issue with the §250-style table.

## Anti-degeneration rules

- **Pricing is not optional.** The two-party loop without an assessor is rule bloat
  (doubling, §BS — both above).
- **The defender's ceiling is provability.** Past it lies §BA's 17 false merges. When a
  packet's defense would require judgment, it is not a defense target — it routes to the
  consumer's evidence surface ([agent-recipe](agent-recipe.md)) or the rubric.
- **Keep the adversaries apart.** This loop is the *structure* adversary. The
  *distribution* adversary — fresh-repo head-of-ranking audits and inbound field
  feedback (issues #263/#264 → PRs #265/#266) — is a separate instrument under
  [design §2c](design.md); don't merge them, they find different things.
- **No metric gaming.** Packets must never be tuned against the held-out split; the
  labelset rules in [bench/labels/README](../bench/labels/RUBRIC.md) apply unchanged.
- A campaign that finds nothing priced is a **green result**, not a failure — say so in
  the closeout.

## Cadence & cost (measured, series 1)

On demand ("run adversarial co-evolution" = one full protocol pass) or per release
alongside the [hazard release checklist](hazard-release-checklist.md). A campaign is
bounded: 3–5 surfaces, one boundary re-attack round, one experiments.md section.
A *series* of campaigns may share one tracking issue (series 1 ran five under #268)
and one combined ledger section when run in a single session.

Measured execution speed ([§BZ](experiments.md), series 1 on an M-class laptop):

| unit of work | wall time |
|---|---|
| one campaign (attack → price → defend → re-attack) | ~6–20 min of agent time |
| five-campaign series incl. recording | ~70 min |
| corpus re-price sweep (105 repos, declaration filter) | ~3 min |
| full nose-cli e2e suite | ~23 s |
| pathological perf fixture scan (the C3 packet) | ≤ 3.2 s |

Budget rule of thumb: a release-cadence series costs about an hour of agent time and
two corpus sweeps; the dominant human cost is arbitrating judgment-axis packets,
which series 1 needed zero of.

---

*See also: [design & direction](design.md) · [type4-adversarial-coverage](type4-adversarial-coverage.md) ·
[frontier-platform](frontier-platform.md) · [formal-soundness](formal-soundness.md) ·
[experiments](experiments.md) · [benchmark](benchmark.md).*
