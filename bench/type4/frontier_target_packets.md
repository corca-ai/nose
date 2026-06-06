# Type-4 frontier target packets

Implementation-ready selections from the corpus-balanced frontier evidence platform.
Each packet LINKS human-verified `real_frontier.v1.json` evidence (it never restates a
status) and adds team routing. See [frontier-platform](../../docs/frontier-platform.md).

- build ref: `c941c4094b608335f8b378224a17cea63ad84640` · union signature `35fa3e15355ae069…`
- corpus: 105 repos · commit digest `278b5b6b7c2e0a9a…`
- owner routes: proof-fact-prerequisite, team-a-detector, team-c-product
- packets: 1

## `numeric-clamp-2026-06-06` — axis `numeric_clamp`

- **owner route**: `team-a-detector` (#49) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `none`
- **breadth**: repo 25% · primary-language 100% (7/7) · dev 14 · held-out 12 · both-splits
- **semantic claim**: min(max(x,lo),hi), max(min(x,hi),lo), and (x<lo ? lo : (x>hi ? hi : x)) all denote the same clamp for a totally-ordered numeric domain with lo <= hi. The boltons `clamp` and the fzf `Constrain` are the two canonical min/max compositions, in different languages, and should converge.
- **proof invariant**: Recognize clamp as min(max(x,lo),hi) = max(min(x,hi),lo) = two-comparison form ONLY with proven scalar min/max facts and a lo <= hi precondition; reject swapped bound order min(max(x,hi),lo), wrong nesting max(min(x,lo),hi), the lo>hi precondition violation, and float NaN (where min/max builtins vs comparison chains can diverge by language). Machine-checked in formal/Clamp.lean.
- **hard negatives**:
  - swapped bound order min(max(x, hi), lo) -- Clamp.lean swapped_bounds_not_clamp
  - wrong nesting max(min(x, lo), hi) -- Clamp.lean wrong_nesting_not_clamp
  - lo > hi precondition violation: the two compositions diverge -- Clamp.lean precondition_required
  - float NaN inputs where min/max builtins and comparison chains can return different values depending on language NaN semantics
- **evidence**: `numeric-clamp-minmax-ternary-real-miss` (`real_frontier.v1.json`)
- **representative locations**:
  - `boltons` (heldout, Python) `boltons/mathutils.py:40-69`
  - `fzf` (heldout, Go) `src/util/util.go:63-65`
- **current detector result**: miss=True · `nose 0.5.0` @ `c941c4094b60` — abs control merges (1 family: absTern, absBuilt); the clamp forms (clampTern, clampBuilt) are NOT merged.
- **why now**: A genuine machine-checked semantic under-merge (formal/Clamp.lean) that is broad and generalizing — present in all 7 corpus primary languages on both the dev and held-out splits — and composes already-proven scalar min/max facts, so the proof invariant is narrow. Two real canonical forms (boltons `clamp` = min(max), fzf `Constrain` = max(min)) do not converge today.
- **blocked by**: nothing
- **notes**: Value-graph clamp canonicalization. NOT proof-fact-prerequisite: the required scalar min/max facts already exist (numeric_minmax_abs is covered). Team A owns the recognizer/soundness gates; this packet's contract ends at the proof invariant and target evidence.

