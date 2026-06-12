# Reinvented helpers — the containment channel

An experimental, exact-grade finding class: a function that **reimplements an existing
pure helper inline instead of calling it**. It is the dual of the clone channels — not
"these two units are alike" but "this unit *contains*, as an interior sub-computation,
exactly the whole body of that helper". The actionable fix is the inverse of
extract-method: replace the matched lines with a call to the helper that already exists.

## The claim, precisely

For a finding `container ⟵ helper`:

- the **helper** is a function/method whose value-graph build produced exactly one
  `Return` sink and nothing irreversible (loop iteration `Cond` guards allowed; no
  effects, throws, or breaks), passing the strict exact gate;
- the **container** passes the strict exact gate and carries an interior sub-DAG
  [anchor](normalization.md) whose hash equals the helper's whole return-value hash —
  the same hash-consed canonical-structure guarantee the exact `semantic` channel
  rides, so the matched sub-computation and the helper body are *the same
  computation*, never merely similar;
- every loop-guard (`Cond`) hash of the helper is also present in the container's
  fingerprint — matching a fold while iterating differently is not containment;
- the helper did **not** rely on a pointer-length contract (a free-param loop bound
  `while i < n` assumed to be `len(array)`). Such a bound is dropped from BOTH the
  guard set and the value hash, so two folds with different bounds (`i < n` vs `i < n-1`)
  would share a return hash though they compute different values — and the guard-
  inclusion check above is then vacuous. Contract-bound helpers are excluded fail-closed;
  genuine length iteration (`for x in xs`, `while i < len(xs)`) records no contract and
  stays eligible (coevo series 6, S3-3).

Two exclusions keep the surface honest:

- **Callers are never findings.** [Generalized pure inlining](normalization.md) splices
  a callee's value graph into its caller's fingerprint, so every well-behaved caller
  would otherwise "contain" its helper. Two guards exclude callers: a unit's provable
  same-file call targets (`CallTarget::DirectFunction`) are recorded and a match on a
  called helper's return hash — directly or via a behaviorally-equal twin — is skipped;
  and a matched anchor carrying a REAL source span OUTSIDE the container's own line
  range is rejected, since that span belongs to a different (inlined) function the unit
  merely calls — the case a one-level call-target record misses on a two-hop chain
  (coevo series 6, S3-2). Calling is the fix, not the smell.
- **Idiom-sized helpers are never matched.** The helper must clear both a value-graph
  floor (≥ 8 nodes) and a source floor (≥ 20 tokens). Value-graph weight alone cannot
  tell a compressed accumulator loop (a whole loop canonicalizes to a ~4-node `Reduce`
  — semantically rich) from a one-line delegation idiom (`self._print(expr.args[0])` —
  trivial to re-type); the source floor is the honest "is calling it actually better"
  proxy. Calibrated on sympy: the delegation-noise band sits at ≤ 12 tokens, real
  helpers at ≥ 25 (108 raw matches → 2 true findings).

## Surface

- **Human report**: the default report LISTS the non-test findings (top by weight) —
  promoted from a one-line count after a [field audit](reinvented-helper-audit-2026-06-13.md)
  measured them at 94% genuine value-duplications / 71% directly actionable (design §2c).
  Findings whose CONTAINER is a test file (`container_in_test`) are a decidable
  judgment-deep class (§2b) — a test asserting the helper's value as a literal would be
  circular to "fix" — so they are excluded from the default and shown only by
  `--show reinvented`, which lists every finding.
- **Scan JSON**: an additive `reinvented_helpers` array (omitted when empty) — see
  [scan-json](scan-json.md#reinvented-helpers).
- The container being a test file or vendored code is *judgment-deep* non-action
  ([design §2b](design.md)): the consumer decides; nose carries the locations.

## The suggested fix is advisory, not mechanical

The finding says *this computation already exists as a helper* — it does **not** promise
that mechanically replacing the reported lines compiles or preserves behavior. Two
boundaries the consumer must check (coevo series 6, S3-1/S3-4):

- **Approximate site.** When the matched computation is a synthesized loop fold (a
  `Reduce` with no precise source span), the site falls back to the WHOLE container range
  and `site_approximate` is `true` in the JSON. The helper's computation is then a
  *sub-part* of those lines (the container does more — e.g. `total * extra + 9` around
  the fold), so the fix is "call the helper for the matched part", not "delete these
  lines". The flagship `mean = sum(xs) / len(xs)` is exactly this shape: `sum` is the
  reinvented helper; `/ len` stays.
- **Types are erased.** The value model abstracts away nominal types
  ([clone-types](clone-types.md)), so a container over one struct type can value-exactly
  contain a helper taking a *different* struct type with same-named fields. In a
  statically-typed language (Go/Java/Rust/C) the suggested call may not type-check — the
  consumer must confirm the helper is callable with the container's operands.

## Measured (2026-06-12, the 105-repo corpus)

16 findings across 8 repos ([experiments §CF](experiments.md)): 16/16 value-exact on
hand-labeling, ~13/16 directly actionable; the remainder are test/vendored containers.
One finding surfaced a real upstream bug — h2database's `getGarbageCollectionCount()`
copy-pasted from the time variant and still calls `getCollectionTime()`, which is
*why* it exactly contains the time helper's computation. Tuning knob:
`NOSE_REINVENTED_MIN_WEIGHT` (research surface) adjusts the anchor collection floor.

*See also: [normalization](normalization.md) · [clone-types](clone-types.md) ·
[scan-json](scan-json.md) · [design](design.md) · [experiments](experiments.md).*
