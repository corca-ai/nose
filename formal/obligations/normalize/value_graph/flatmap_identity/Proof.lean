/-
Soundness of nose's flat-map identity canonicalization.

The value graph lowers `flatMap(λx. x)` (identity inner — the lambda returns the outer
element unchanged) to the modeled element-stream inner `flatMap(λx. map(λy. y) x)`, so identity
flat-map converges with the nested builder loop and the explicit inner-identity-map form
(value_graph.rs `HoFKind::FlatMap` arm).

This file proves the underlying monad law `flatMap id = join` for `List`, and that the modeled
inner-identity-map form has the same denotation — so the canonicalization is meaning-preserving
(the converged forms compute the same flattened list).

Self-contained; checked by the formal obligation CI gate.
-/

namespace NoseFlatMapIdentity

/-- The list functor's map. -/
def lmap (f : α → β) : List α → List β
  | []      => []
  | x :: xs => f x :: lmap f xs

/-- Flatten a list of lists (the monad `join`). -/
def lflatten : List (List α) → List α
  | []        => []
  | xs :: xss => xs ++ lflatten xss

/-- The list monad's flat-map / concatMap. -/
def lflatMap (f : α → List β) : List α → List β
  | []      => []
  | x :: xs => f x ++ lflatMap f xs

/-- FUNCTOR IDENTITY law: `map id = id`. The identity inner map is a no-op on each sublist. -/
theorem lmap_id (xs : List α) : lmap (fun x => x) xs = xs := by
  induction xs with
  | nil => rfl
  | cons x xs ih => simp [lmap, ih]

/-- MONAD law `flatMap id = join`: identity flat-map is exactly flatten. This is the
    equivalence the canonicalization targets — `xss.flatMap(λx. x)` is `flatten xss`. -/
theorem flatMap_id (xss : List (List α)) :
    lflatMap (fun x => x) xss = lflatten xss := by
  induction xss with
  | nil => rfl
  | cons xs xss ih => simp [lflatMap, lflatten, ih]

/-- The canonicalization itself is denotation-preserving: rewriting the identity inner
    `λx. x` to the modeled element-stream inner `λx. map (λy. y) x` does not change the result
    (both flatten the sublists), because `map id = id` on each sublist. -/
theorem flatMap_inner_mapId_eq (xss : List (List α)) :
    lflatMap (fun x => lmap (fun y => y) x) xss = lflatMap (fun x => x) xss := by
  simp only [lmap_id]

end NoseFlatMapIdentity
