/-
Soundness of the clamp idiom that the #50 `numeric_clamp` frontier target packet proposes
for detector convergence (owner: Team A / #49).

A clamp `min(max(x, lo), hi)` (nested min/max composition) and `max(min(x, hi), lo)` and a
two-comparison `if x < lo then lo else if hi < x then hi else x` all denote the same value,
PROVIDED `lo ≤ hi`. This file proves the positive equivalences. Boundary counterexamples
live in `Counterexamples.lean`.

Self-contained over `Int` (no Mathlib).
-/

namespace NoseClamp

/-- Canonical min/max, matching the `normalize.value_graph.min_max` obligation. -/
def vmin (a b : Int) : Int := if a < b then a else b
def vmax (a b : Int) : Int := if a < b then b else a

/-- The two-comparison clamp form found in real code (`x < lo ? lo : x > hi ? hi : x`). -/
def clampCmp (x lo hi : Int) : Int := if x < lo then lo else if hi < x then hi else x

/-- `min(max(x, lo), hi)` IS the two-comparison clamp, given `lo ≤ hi`. -/
theorem clamp_minmax (x lo hi : Int) (h : lo ≤ hi) :
    vmin (vmax x lo) hi = clampCmp x lo hi := by
  unfold vmin vmax clampCmp
  by_cases h1 : x < lo <;> by_cases h2 : x < hi <;> by_cases h3 : hi < x <;>
    by_cases h4 : lo < hi <;> simp_all <;> omega

/-- `max(min(x, hi), lo)` IS the same two-comparison clamp, given `lo ≤ hi`. -/
theorem clamp_maxmin (x lo hi : Int) (h : lo ≤ hi) :
    vmax (vmin x hi) lo = clampCmp x lo hi := by
  unfold vmax vmin clampCmp
  by_cases h1 : x < lo <;> by_cases h2 : x < hi <;> by_cases h3 : hi < x <;>
    by_cases h4 : lo < hi <;> simp_all <;> omega

/-- The two min/max compositions therefore agree (order-insensitive clamp), given `lo ≤ hi`. -/
theorem clamp_forms_agree (x lo hi : Int) (h : lo ≤ hi) :
    vmin (vmax x lo) hi = vmax (vmin x hi) lo := by
  rw [clamp_minmax x lo hi h, clamp_maxmin x lo hi h]

end NoseClamp
