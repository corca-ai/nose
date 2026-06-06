/-
Soundness boundaries for the free-monoid string/list model.

String/list builders are modeled as ordered concatenation of opaque pieces. Concatenation
is associative and has an empty identity, but it is not commutative and it is not a ring:
distribution/factoring across repetition is unsound. These facts are the proof boundary
behind the value graph's concat ordering and Num gate on `factor_distribute`.
-/

namespace NoseFreeMonoid

abbrev Str := List Nat

def concat (left right : Str) : Str := left ++ right

def vrepeat : Str → Nat → Str
  | _, 0 => []
  | xs, n + 1 => xs ++ vrepeat xs n

theorem concat_assoc (a b c : Str) :
    concat (concat a b) c = concat a (concat b c) := by
  simp [concat, List.append_assoc]

theorem concat_left_id (xs : Str) : concat [] xs = xs := rfl

theorem concat_right_id (xs : Str) : concat xs [] = xs := by
  simp [concat]

/-- Repeating twice is ordered self-concatenation. -/
theorem repeat_two_eq_concat_self (xs : Str) : vrepeat xs 2 = concat xs xs := by
  simp [vrepeat, concat]

end NoseFreeMonoid
