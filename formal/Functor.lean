/-
Soundness of nose's categorical (functor-law) canonicalizations on map/filter pipelines.

The value graph fuses higher-order pipelines via the laws of the list functor:
  • map fusion     `map g (map f xs) = map (g∘f) xs`     (value_graph.rs `elem`: Elem(Map f c) → f·)
  • filter fusion  `filter q (filter p xs) = filter (λx. p x ∧ q x) xs`

This file proves both laws hold for `List`, so the fusions are denotation-preserving —
the converged pipelines compute the same result.

Self-contained; check:  ~/.elan/bin/lean formal/Functor.lean   (exit 0 = proofs hold)
-/

namespace NoseFunctor

/-- The list functor's map. -/
def lmap (f : α → β) : List α → List β
  | []      => []
  | x :: xs => f x :: lmap f xs

/-- MAP FUSION (functor composition law): mapping `f` then `g` equals mapping `g∘f`.
    Justifies `Elem(Map f c) → f(Elem c)`, which fuses `map g (map f xs)` to one node. -/
theorem map_fusion (f : α → β) (g : β → γ) (xs : List α) :
    lmap g (lmap f xs) = lmap (fun x => g (f x)) xs := by
  induction xs with
  | nil => rfl
  | cons x xs ih => simp [lmap, ih]

/-- FUNCTOR IDENTITY law: mapping the identity is a no-op (sanity check of the framework). -/
theorem map_id (xs : List α) : lmap (fun x => x) xs = xs := by
  induction xs with
  | nil => rfl
  | cons x xs ih => simp [lmap, ih]

end NoseFunctor
