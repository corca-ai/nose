import formal.lib.Recursion

namespace NoseRecursionStructuralFold

open NoseFormal.Recursion

theorem add_monoid_preconditions (x y z : Int) :
    (x + y) + z = x + (y + z) /\ 0 + x = x := by
  constructor
  · exact Int.add_assoc x y z
  · simp

theorem mul_monoid_preconditions (x y z : Int) :
    (x * y) * z = x * (y * z) /\ 1 * x = x := by
  constructor
  · exact Int.mul_assoc x y z
  · simp

theorem add_structural_fold_sound (heads : List Int) :
    heads.foldl (fun total head => total + head) 0 =
      heads.foldr (fun head total => head + total) 0 := by
  exact int_add_left_fold_eq_right_fold heads

theorem mul_structural_fold_sound (heads : List Int) :
    heads.foldl (fun total head => total * head) 1 =
      heads.foldr (fun head total => head * total) 1 := by
  exact int_mul_left_fold_eq_right_fold heads

end NoseRecursionStructuralFold
