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

/-
FLOAT BOUNDARY. The rewrite (right-fold recursion → left-fold loop) is sound only over an
ASSOCIATIVE monoid. IEEE float `+`/`*` is NOT associative — rounding makes the grouping
observable — so a float-valued HEAD must be excluded by the Rust gate (`head_possibly_float` in
`recursion/structural_fold.rs`), NOT admitted just because its coarse `ValueDomain` is `Number`
(which does not separate integer from float). Lean's `Float` is opaque (no `decide`), so we witness
the SAME failure mode with a faithful low-precision model: `radd` rounds the sum to the nearest ten
(half up) but keeps an EXACT identity (`x ⊕ 0 = 0 ⊕ x = x`, mirroring `a +. 0.0 = a`). Even with the
exact identity the monoid law assumes, the rounded sum is non-associative — `(100 ⊕ 4) ⊕ 4 = 100`
loses both small terms, while `100 ⊕ (4 ⊕ 4) = 110` keeps them — so foldl and foldr disagree, which
is exactly why the rewrite must stay integer-only.
-/
def round10 (x : Int) : Int := ((x + 5) / 10) * 10

def radd (a b : Int) : Int :=
  if a = 0 then b else if b = 0 then a else round10 (a + b)

theorem float_like_rounding_breaks_fold :
    (([100, 4, 4] : List Int).foldl (fun total head => radd total head) 0) ≠
      (([100, 4, 4] : List Int).foldr (fun head total => radd head total) 0) := by
  decide

end NoseRecursionStructuralFoldCounterexamples
