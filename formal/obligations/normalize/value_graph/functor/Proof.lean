/-
Soundness of nose's categorical (functor-law) canonicalizations on map/filter pipelines.

The value graph fuses higher-order pipelines via the laws of the list functor:
  • map fusion     `map g (map f xs) = map (g∘f) xs`     (value_graph.rs `elem`: Elem(Map f c) → f·)
  • filter fusion  `filter q (filter p xs) = filter (λx. p x ∧ q x) xs`

This file proves both laws hold for `List`, so the fusions are denotation-preserving —
the converged pipelines compute the same result.

Self-contained; checked by the formal obligation CI gate.
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

/-- The list filter: keep the elements satisfying the predicate, in order. -/
def lfilter (p : α → Bool) : List α → List α
  | []      => []
  | x :: xs => if p x then x :: lfilter p xs else lfilter p xs

/-- FILTER FUSION: filtering by `p` then by `q` equals filtering by their conjunction.
    Justifies the value graph representing `filter(p, c)` as `Hof(Map, [Elem c, p])` and
    fusing nested filters via `and_preds` (`value_graph.rs` `HoFKind::Filter` arm), so
    `filter(q, filter(p, xs))`, `[x for x in xs if p if q]`, and the filtered builder loop
    all converge to one `Hof(Map, [Elem xs, p∧q])`. Denotation-preserving — sound. -/
theorem filter_fusion (p q : α → Bool) (xs : List α) :
    lfilter q (lfilter p xs) = lfilter (fun x => p x && q x) xs := by
  induction xs with
  | nil => rfl
  | cons x xs ih => cases hp : p x <;> cases hq : q x <;> simp [lfilter, hp, hq, ih]

/-- Count the elements satisfying `p` by summing a 0/1 indicator — the form `sum(1 for x in
    xs if p)` lowers to. -/
def lcount (p : α → Bool) : List α → Nat
  | []      => 0
  | x :: xs => (if p x then 1 else 0) + lcount p xs

/-- COUNT OF FILTER: `len(filter p xs) = Σ (p x ? 1 : 0)`. Justifies the value graph folding
    `len([c for x in xs if p])` to the same count-reduce as `sum(1 for x in xs if p)`
    (`value_graph.rs` `Builtin::Len` over a filtered `Hof(Map)`). Denotation-preserving. -/
theorem filter_length_eq_count (p : α → Bool) (xs : List α) :
    (lfilter p xs).length = lcount p xs := by
  induction xs with
  | nil => rfl
  | cons x xs ih => cases hp : p x <;> simp [lfilter, lcount, hp] <;> omega

end NoseFunctor
