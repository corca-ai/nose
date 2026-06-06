/-
Boundary counterexamples for structural recursion to accumulator-fold rewrites.

The accepted rewrite depends on an associative monoid and the correct identity.
Subtraction is not such a monoid, and multiplication with the wrong initial value
does not preserve the recursive right-fold meaning.
-/

namespace NoseRecursionStructuralFoldCounterexamples

theorem subtraction_is_not_a_structural_fold_monoid :
    (([1, 2, 3] : List Int).foldl (fun total head => total - head) 0) ≠
      (([1, 2, 3] : List Int).foldr (fun head total => head - total) 0) := by
  decide

theorem wrong_mul_identity_changes_fold :
    (([2, 3] : List Int).foldl (fun total head => total * head) 0) ≠
      (([2, 3] : List Int).foldr (fun head total => head * total) 1) := by
  decide

end NoseRecursionStructuralFoldCounterexamples
