/-
Boundary counterexamples for the clamp obligation.

These prove why the recognizer must reject swapped bounds, wrong nesting, and missing
`lo ≤ hi` evidence.
-/

namespace NoseClampCounterexamples

def vmin (a b : Int) : Int := if a < b then a else b
def vmax (a b : Int) : Int := if a < b then b else a
def clampCmp (x lo hi : Int) : Int := if x < lo then lo else if hi < x then hi else x

/-- Swapped bound order `min(max(x, hi), lo)` is not the clamp. -/
theorem swapped_bounds_not_clamp :
    ∃ x lo hi : Int, lo ≤ hi ∧ vmin (vmax x hi) lo ≠ clampCmp x lo hi :=
  ⟨5, 0, 1, by decide, by decide⟩

/-- Wrong nesting `max(min(x, lo), hi)` is not the clamp. -/
theorem wrong_nesting_not_clamp :
    ∃ x lo hi : Int, lo ≤ hi ∧ vmax (vmin x lo) hi ≠ clampCmp x lo hi :=
  ⟨0, 0, 5, by decide, by decide⟩

/-- The `lo ≤ hi` precondition is required. -/
theorem precondition_required :
    ∃ x lo hi : Int, hi < lo ∧ vmin (vmax x lo) hi ≠ vmax (vmin x hi) lo :=
  ⟨0, 1, 0, by decide, by decide⟩

end NoseClampCounterexamples
