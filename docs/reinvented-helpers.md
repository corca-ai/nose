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
  fingerprint — matching a fold while iterating differently is not containment.

Two exclusions keep the surface honest:

- **Callers are never findings.** [Generalized pure inlining](normalization.md) splices
  a callee's value graph into its caller's fingerprint, so every well-behaved caller
  would otherwise "contain" its helper. A unit's provable same-file call targets
  (`CallTarget::DirectFunction`) are recorded, and a match on a called helper's return
  hash — directly or via a behaviorally-equal twin — is skipped: calling is the fix,
  not the smell.
- **Idiom-sized helpers are never matched.** The helper must clear both a value-graph
  floor (≥ 8 nodes) and a source floor (≥ 20 tokens). Value-graph weight alone cannot
  tell a compressed accumulator loop (a whole loop canonicalizes to a ~4-node `Reduce`
  — semantically rich) from a one-line delegation idiom (`self._print(expr.args[0])` —
  trivial to re-type); the source floor is the honest "is calling it actually better"
  proxy. Calibrated on sympy: the delegation-noise band sits at ≤ 12 tokens, real
  helpers at ≥ 25 (108 raw matches → 2 true findings).

## Surface

- **Human report**: a one-line count after the family report; `--show reinvented`
  lists every finding. The default surface stays a count because the class is new and
  its field precision is freshly measured, per
  [design §2c](design.md) — promotion follows the labelset discipline.
- **Scan JSON**: an additive `reinvented_helpers` array (omitted when empty) — see
  [scan-json](scan-json.md#reinvented-helpers).
- The container being a test file or vendored code is *judgment-deep* non-action
  ([design §2b](design.md)): the consumer decides; nose carries the locations.

## Measured (2026-06-12, the 105-repo corpus)

16 findings across 8 repos ([experiments §CF](experiments.md)): 16/16 value-exact on
hand-labeling, ~13/16 directly actionable; the remainder are test/vendored containers.
One finding surfaced a real upstream bug — h2database's `getGarbageCollectionCount()`
copy-pasted from the time variant and still calls `getCollectionTime()`, which is
*why* it exactly contains the time helper's computation. Tuning knob:
`NOSE_REINVENTED_MIN_WEIGHT` (research surface) adjusts the anchor collection floor.

*See also: [normalization](normalization.md) · [clone-types](clone-types.md) ·
[scan-json](scan-json.md) · [design](design.md) · [experiments](experiments.md).*
