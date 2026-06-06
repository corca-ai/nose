/-
Counterexamples for rewrites that would treat ordered concat like a commutative ring.
-/

namespace NoseFreeMonoidCounterexamples

abbrev Str := List Nat

def concat (left right : Str) : Str := left ++ right

def vrepeat : Str → Nat → Str
  | _, 0 => []
  | xs, n + 1 => xs ++ vrepeat xs n

/-- Ordered concat is not commutative. -/
theorem concat_not_commutative :
    ∃ xs ys : Str, concat xs ys ≠ concat ys xs :=
  ⟨[0], [1], by decide⟩

/-- Distribution/factoring over repetition is not sound for strings/lists:
    `(x+y)*2` gives `xyxy`, while `x*2 + y*2` gives `xxyy`. -/
theorem repeat_distribution_not_sound :
    ∃ xs ys : Str, vrepeat (concat xs ys) 2 ≠ concat (vrepeat xs 2) (vrepeat ys 2) :=
  ⟨[0], [1], by decide⟩

end NoseFreeMonoidCounterexamples
