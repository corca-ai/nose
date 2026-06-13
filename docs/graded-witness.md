# Graded equivalence witness

The exact `semantic` channel proves two units compute the same thing — *equal
fingerprint ⟹ equal behavior* ([design §1](design.md)). The `near` channel only
scores similarity. The **graded witness** bridges them: for a near family it computes
the *least general generalization* (anti-unification) of its two representative copies'
value DAGs and reports them as **equal except at *k* holes**, each hole carrying the
specific value that differs. It turns a bare `0.86` into a machine-checkable,
actionable statement — the lever for [consumer 1](design.md) (the calling agent reads
*what* differs instead of re-deriving it) and a graded surface for the
[review](review.md) gate.

This is the productized outcome of the #315 investigation; the
[architecture](architecture.md) pipeline (step 5, scoring) emits it, and it surfaces in
[scan JSON](scan-json.md) under `witness.graded`.

## What it computes

Given the two copies' canonical [value graphs](normalization.md), the witness aligns
them node by node:

- nodes that match (equal structural hash) are shared — the body of the helper you
  would extract;
- each spot where they differ becomes a **hole** — a parameter of that helper.

Each hole is classified by what kind of value differs:

- **clean parameters** — `literal`, `input`, `field`, `call`, `lambda`, `operator`,
  `expr`: a value leaf an extracted helper could take as an argument;
- **structural divergence** — `arity`, `shape`, `unmodeled`, `extra-sink`: not a clean
  parameter (different number of operands, different shape, an unmodeled construct, or
  a behavior sink present on only one side).

`equal_modulo_holes` is the strongest grade: every hole is a small value leaf, the
behavior sinks aligned, and no consumed name mismatched. The exact channel is the
degenerate `k = 0` case; the experimental [`abstraction`](usage.md) witness is the
`k = 1` literal case.

## Patterns, not noise

Some divergences are better described than itemized as holes:

- `effects-reordered` — the two copies perform the same effect multiset in a different
  order (matched by an order-preserving LCS, so a pure reorder does not manufacture
  positional holes);
- `sink-superset-a` / `sink-superset-b` — one copy does strictly more (an extra
  return/effect) while the shared part is hole-free equal;
- `fragment-containment` — the smaller copy is a sliver of the larger (a containment,
  not a clone claim);
- `low-substance` — the units are too small for the claim to mean much.

## Soundness — the referent check

Two copies can be node-for-node identical yet behave differently because a *name* they
both use denotes different things — `equals` on two unrelated classes, a same-named
locale table in two files, a per-package context key. The witness resolves every
consumed name to a content-based identity (a local definition by the content hash of
its body, an import by its `(module, exported)` coordinate, a self-call by a stable
marker) and compares:

- **`referent_mismatches`** — a name both copies consume that resolves to *disjoint*
  definitions. This **demotes** the witness: the copies are not equal-modulo-holes.
  Fail-closed — the soundness-relevant direction.
- **`caveat_names`** — a name unresolved on at least one side. The claim is *scoped*
  past it (a reviewer should confirm it denotes the same thing) rather than silently
  trusted; `modeled_caveat` flags the broader case where a copy passed lossy lowering,
  so "equal" means equal in the modeled fraction.
- **Definition-site modifiers.** A decorator/annotation/attribute (`@click.command(…)`,
  `@Test`, `#[inline]`) modifies behavior at the definition site, but its *arguments* are
  dropped at lowering — `@click.argument("x")` and `@click.argument("x", metavar="m")`
  produce the same value graph. Because the graph cannot see this, the witness compares
  the two copies' decorator/attribute **source lines** directly: any difference becomes a
  `decorator` hole (with the differing text), fires the `decorator-differs` pattern, and
  demotes `equal_modulo_holes`. (Language-aware: a leading `@` is a decorator in
  Python/Java/JS/TS and an *instance variable* in Ruby, where it is correctly ignored;
  Rust uses `#[…]`.)

A pair too large or too deep to align soundly yields **no** witness rather than a
guessed one.

## Ranking

The witness's hole count is, in principle, a more semantically-grounded "number of
parameters the helper would need" than the source-line [`varying_spots`](scan-json.md)
count the default [extractability](usage.md) ranking uses. Re-ranking by it was measured
on the gold set (`bench/labels/eval_by_language.py`, anti-unification re-rank vs the
extractability baseline): the effect is **within noise overall** (dev +2pp, held-out
−1pp P@10, CIs overlapping; it helps Java/Ruby/Rust and hurts Python/TypeScript). Per
the measure-before-betting discipline, the **default ranking is left unchanged** — the
witness is carried as machine-readable evidence so a consumer's own re-ranker can use it,
not folded into nose's deterministic order on a neutral signal.

## Scope and limits

- **Same-language near families only.** Cross-language copies share no value-DAG
  structure by construction; the witness is absent for them, as it is for sub-function
  fragments and pathological (generated/minified) files.
- **The unit body, plus decorators by source — not the full signature.** The witness
  compares the two units' *value graphs* (the modeled body), augmented by the
  source-level decorator/attribute check above. What it still does not model: the
  parameter **signature** — a differing *unused* parameter is invisible (a used one
  surfaces as a value-graph hole). Treat `equal_modulo_holes` as "equal within the
  modeled body and matching decorators".
- The witness is **evidence, not a proof.** Unlike the exact channel it carries no Lean
  theorem; its soundness is recorded as the `empirical-only` obligation
  `detect.graded_witness` (see [formal soundness](formal-soundness.md)), defended by the
  referent check, the decorator comparison, and the witness soundness battery rather than
  machine-checked. Treat `referent_mismatches`/`caveat_names` as the honest boundary of
  the claim.
- It is **best-effort enrichment**, computed at the presentation layer (which has
  source access), exactly like the line-level [`varying_spots`](scan-json.md) — the two
  describe the same divergence at the value-graph and source-line granularities
  respectively.

*See also: [design](design.md) · [architecture](architecture.md) ·
[normalization](normalization.md) · [clone-types](clone-types.md) ·
[scan JSON](scan-json.md) · [review](review.md).*
