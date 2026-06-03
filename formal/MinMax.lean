/-
Soundness of nose's 2-way min/max idiom canonicalization.

The value graph rewrites a 2-way select `x if x<y else y` (a `Phi`) and a `min(x,y)` call
to one canonical `Min(x,y)` node (likewise `Max`), and recognizes a min/max accumulator
loop as a selection reduction. This file proves the laws those rely on: `Min`/`Max` are
exactly the ternary, and they are commutative and associative (a commutative monoid up to
the ±∞ identity) — so the canonical node is order-insensitive and the fold is well-defined.

Self-contained over `Int`; check:  ~/.elan/bin/lean formal/MinMax.lean
-/

namespace NoseMinMax

/-- The canonical Min node IS the ternary `x if x<y else y` (definitional soundness:
    the value graph interprets `MIN_CODE` as exactly this). -/
def vmin (a b : Int) : Int := if a < b then a else b
def vmax (a b : Int) : Int := if a < b then b else a

/-- `x if x<y else y` denotes `vmin x y` — the recognition is exact. -/
theorem min_is_ternary (x y : Int) : (if x < y then x else y) = vmin x y := rfl
theorem max_is_ternary (x y : Int) : (if x < y then y else x) = vmax x y := rfl

/-- Min/Max are COMMUTATIVE (so the commutative `MIN_CODE`/`MAX_CODE` canon is sound). -/
theorem vmin_comm (a b : Int) : vmin a b = vmin b a := by
  unfold vmin; split <;> split <;> omega
theorem vmax_comm (a b : Int) : vmax a b = vmax b a := by
  unfold vmax; split <;> split <;> omega

/-- Min/Max are ASSOCIATIVE (so a min/max accumulator fold is well-defined regardless of
    grouping — the selection-reduction recognition). -/
theorem vmin_assoc (a b c : Int) : vmin (vmin a b) c = vmin a (vmin b c) := by
  unfold vmin
  by_cases h1 : a < b <;> by_cases h2 : b < c <;> by_cases h3 : a < c <;>
    simp_all <;> omega
theorem vmax_assoc (a b c : Int) : vmax (vmax a b) c = vmax a (vmax b c) := by
  unfold vmax
  by_cases h1 : a < b <;> by_cases h2 : b < c <;> by_cases h3 : a < c <;>
    simp_all <;> omega

/-- Idempotent: `min(x,x) = x` (a corner of the lattice laws). -/
theorem vmin_idem (a : Int) : vmin a a = a := by unfold vmin; split <;> rfl

end NoseMinMax
