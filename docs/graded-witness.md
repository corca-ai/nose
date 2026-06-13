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

A pair too large or too deep to align soundly yields **no** witness rather than a
guessed one.

## Scope and limits

- **Same-language near families only.** Cross-language copies share no value-DAG
  structure by construction; the witness is absent for them, as it is for sub-function
  fragments and pathological (generated/minified) files.
- **The unit body, not its definition site.** The witness compares the two units'
  *value graphs* — the behavior the fingerprint models. Differences at the definition
  site that are *outside* that body are not seen: decorators/annotations and their
  arguments (`@click.command()` vs `@click.command(context_settings=…)`,
  `@option(show_default=True)` vs `show_default="…"`), and the parameter signature.
  So `equal_modulo_holes` is "equal within the modeled body"; a decorated pair whose
  decorators differ can read as equal-modulo-the-body-holes while their configuration
  diverges. Treat the grade accordingly, and prefer a quick check of the definition
  site for decorated/annotated units. (Surfacing definition-site modifiers as holes is
  tracked follow-up work.)
- The witness is **evidence, not a proof.** Unlike the exact channel it carries no Lean
  obligation yet; the `equal_modulo_holes` grade is a checked claim over the modeled
  fraction, defended by the referent check and the [verify oracle](design.md), not a
  machine-checked theorem. Treat `referent_mismatches`/`caveat_names` as the honest
  boundary of the claim.
- It is **best-effort enrichment**, computed at the presentation layer (which has
  source access), exactly like the line-level [`varying_spots`](scan-json.md) — the two
  describe the same divergence at the value-graph and source-line granularities
  respectively.

*See also: [design](design.md) · [architecture](architecture.md) ·
[normalization](normalization.md) · [clone-types](clone-types.md) ·
[scan JSON](scan-json.md) · [review](review.md).*
