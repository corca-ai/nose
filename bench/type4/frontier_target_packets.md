# Type-4 frontier target packets

Implementation-ready selections from the corpus-balanced frontier evidence platform.
Each packet LINKS human-verified `real_frontier.v1.json` evidence (it never restates a
status) and adds team routing. See [frontier-platform](../../docs/frontier-platform.md).

- build ref: `None` · union signature `90ea2da63d26f788…`
- corpus: 120 repos · commit digest `2bf0b8c147be66b7…`
- owner routes: proof-fact-prerequisite, team-a-detector, team-c-product
- packets: 1

## `numeric-clamp-2026-06-06` — axis `numeric_clamp`

- **owner route**: `proof-fact-prerequisite` (no team yet) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `none`
- **breadth**: repo 22% · primary-language 88% (7/8) · dev 14 · held-out 12 · both-splits
- **semantic claim**: min(max(x,lo),hi), max(min(x,hi),lo), and (x<lo ? lo : (x>hi ? hi : x)) all denote the same clamp for a totally-ordered numeric domain with lo <= hi. The boltons `clamp` and the fzf `Constrain` are the two canonical min/max compositions, in different languages, and should converge.
- **proof invariant**: Recognize clamp as min(max(x,lo),hi) = max(min(x,hi),lo) = two-comparison form ONLY with proven scalar min/max facts and a lo <= hi precondition; reject swapped bound order min(max(x,hi),lo), wrong nesting max(min(x,lo),hi), the lo>hi precondition violation, and float NaN (where min/max builtins vs comparison chains can diverge by language). Machine-checked in formal/obligations/normalize/value_graph/clamp/Proof.lean.
- **hard negatives**:
  - swapped bound order min(max(x, hi), lo) -- clamp Counterexamples.lean swapped_bounds_not_clamp
  - wrong nesting max(min(x, lo), hi) -- clamp Counterexamples.lean wrong_nesting_not_clamp
  - lo > hi precondition violation: the two compositions diverge -- clamp Counterexamples.lean precondition_required
  - float NaN inputs where min/max builtins and comparison chains can return different values depending on language NaN semantics
- **evidence**: `numeric-clamp-minmax-ternary-real-miss` (`real_frontier.v1.json`)
- **representative locations**:
  - `boltons` (heldout, Python) `boltons/mathutils.py:40-69`
  - `fzf` (heldout, Go) `src/util/util.go:63-65`
- **current detector result**: miss=True · `nose 0.5.0` @ `58c4c9b0c513` — Historic controlled scan: abs control merged (1 family: absTern, absBuilt); clamp ternary/library bridge forms did not. Current focused equivalence/adversarial cases now cover the proof-backed controlled bridge forms.
- **why now**: A genuine machine-checked semantic under-merge (formal/obligations/normalize/value_graph/clamp/Proof.lean) that is broad and generalizing — present in 7 of the 8 corpus primary-language buckets, with hits in both the dev and held-out splits. The proof-backed min/max plus controlled two-comparison/library bridge slices are implemented; the remaining value is identifying the next real-corpus bound-order proof without weakening the hard-negative boundary.
- **blocked by**: real-corpus bound-order / guarded-range proof fact that `lo <= hi` (formal/obligations/normalize/value_graph/clamp/Counterexamples.lean proves the precondition is required; the current proof-backed slice handles literal bounds and exiting inverse guards, but parameter naming such as fzf `Constrain(val, minimum, maximum)` is not a proof), float-NaN domain exclusion (min/max builtins vs comparison chains can diverge on NaN, by language)
- **notes**: The proof-backed integer Clamp canon now covers min/max composition plus controlled two-comparison and library method bridge surfaces when literal or exiting-guard evidence proves lo<=hi. The remaining packet is still routed proof-fact-prerequisite: parameter naming such as fzf `Constrain(val, minimum, maximum)` is not a proof.
