/-
Soundness of the clamp idiom that the #50 `numeric_clamp` frontier target packet proposes
for detector convergence (owner: Team A / #49).

A clamp `min(max(x, lo), hi)` (nested min/max composition) and `max(min(x, hi), lo)` and a
two-comparison `if x < lo then lo else if hi < x then hi else x` all denote the same value,
PROVIDED `lo ≤ hi`. This file proves that — and proves the boundaries the recognizer must
reject: swapped bound order is a different function, and the `lo ≤ hi` precondition is
required (the two min/max compositions diverge without it).

Self-contained over `Int` (no Mathlib); check:  ~/.elan/bin/lean formal/Clamp.lean
-/

namespace NoseClamp

/-- Canonical min/max, matching `formal/MinMax.lean` (the ternary they denote). -/
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

/-- HARD NEGATIVE: swapped bound order `min(max(x, hi), lo)` is NOT the clamp.
    x=5, lo=0, hi=1: min(max(5,1),0) = min(5,0) = 0, but clamp(5,0,1) = 1. -/
theorem swapped_bounds_not_clamp :
    ∃ x lo hi : Int, lo ≤ hi ∧ vmin (vmax x hi) lo ≠ clampCmp x lo hi :=
  ⟨5, 0, 1, by decide, by decide⟩

/-- HARD NEGATIVE: wrong nesting `max(min(x, lo), hi)` is NOT the clamp.
    x=0, lo=0, hi=5: max(min(0,0),5) = max(0,5) = 5, but clamp(0,0,5) = 0. -/
theorem wrong_nesting_not_clamp :
    ∃ x lo hi : Int, lo ≤ hi ∧ vmax (vmin x lo) hi ≠ clampCmp x lo hi :=
  ⟨0, 0, 5, by decide, by decide⟩

/-- The `lo ≤ hi` precondition is REQUIRED: without it the two canonical compositions
    diverge, so the recognizer must not merge them when bounds can be inverted. -/
theorem precondition_required :
    ∃ x lo hi : Int, hi < lo ∧ vmin (vmax x lo) hi ≠ vmax (vmin x hi) lo :=
  ⟨0, 1, 0, by decide, by decide⟩

end NoseClamp
