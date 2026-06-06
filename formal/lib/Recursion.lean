/-
Shared model for the recursion-to-iteration proof obligations.

The Rust pass fires only for two templates: tail recursion and structural numeric
folds. This file proves the small mathematical facts those templates rely on.
-/

namespace NoseFormal.Recursion

def iterate (step : state -> state) : Nat -> state -> state
  | 0, current => current
  | n + 1, current => iterate step n (step current)

def tailRecursive (step : state -> state) (ret : state -> out)
    (fuel : Nat) (start : state) : out :=
  ret (iterate step fuel start)

def tailLoop (step : state -> state) (ret : state -> out)
    (fuel : Nat) (start : state) : out :=
  ret (iterate step fuel start)

theorem tail_loop_matches_tail_recursion
    (step : state -> state) (ret : state -> out)
    (fuel : Nat) (start : state) :
    tailLoop step ret fuel start = tailRecursive step ret fuel start := by
  rfl

theorem foldl_add_acc (xs : List Int) (acc : Int) :
    xs.foldl (fun total head => total + head) acc =
      acc + xs.foldr (fun head total => head + total) 0 := by
  induction xs generalizing acc with
  | nil =>
      simp
  | cons x xs ih =>
      simp [List.foldl, List.foldr, ih, Int.add_assoc, Int.add_comm, Int.add_left_comm]

theorem int_add_left_fold_eq_right_fold (xs : List Int) :
    xs.foldl (fun total head => total + head) 0 =
      xs.foldr (fun head total => head + total) 0 := by
  simpa using foldl_add_acc xs 0

theorem foldl_mul_acc (xs : List Int) (acc : Int) :
    xs.foldl (fun total head => total * head) acc =
      acc * xs.foldr (fun head total => head * total) 1 := by
  induction xs generalizing acc with
  | nil =>
      simp
  | cons x xs ih =>
      simp [List.foldl, List.foldr, ih, Int.mul_assoc, Int.mul_comm, Int.mul_left_comm]

theorem int_mul_left_fold_eq_right_fold (xs : List Int) :
    xs.foldl (fun total head => total * head) 1 =
      xs.foldr (fun head total => head * total) 1 := by
  simpa using foldl_mul_acc xs 1

end NoseFormal.Recursion
