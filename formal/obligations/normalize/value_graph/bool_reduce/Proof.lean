/-
Soundness of nose's `any`/`all` reduction canonicalization.

The value graph collapses `any(p(x) for x in xs)` / `xs.some(p)` / `xs.iter().any(p)` to one
`Reduce(REDUCE_ANY, [p])` node, and likewise `all`/`every`/`.all` to `Reduce(REDUCE_ALL, [p])`.
For that canonical fold to be well-defined regardless of how the source iterated or grouped,
the underlying operator must be a commutative monoid: OR with identity `false` for `any`,
AND with identity `true` for `all`. This file proves exactly those laws over `Bool`, plus
that folding a predicate-mapped list with them computes existential / universal truth — so
the cross-language forms denote the same thing.

Self-contained over `Bool`/`List`; checked by the formal obligation CI gate.
-/

namespace NoseBoolReduce

/-- OR is a COMMUTATIVE MONOID with identity `false` (justifies the commutative, seedless
    `REDUCE_ANY` fold). -/
theorem or_comm  (a b : Bool) : (a || b) = (b || a) := by cases a <;> cases b <;> rfl
theorem or_assoc (a b c : Bool) : ((a || b) || c) = (a || (b || c)) := by
  cases a <;> cases b <;> cases c <;> rfl
theorem or_id    (a : Bool) : (a || false) = a := by cases a <;> rfl
theorem or_idem  (a : Bool) : (a || a) = a := by cases a <;> rfl

/-- AND is a COMMUTATIVE MONOID with identity `true` (justifies `REDUCE_ALL`). -/
theorem and_comm  (a b : Bool) : (a && b) = (b && a) := by cases a <;> cases b <;> rfl
theorem and_assoc (a b c : Bool) : ((a && b) && c) = (a && (b && c)) := by
  cases a <;> cases b <;> cases c <;> rfl
theorem and_id    (a : Bool) : (a && true) = a := by cases a <;> rfl
theorem and_idem  (a : Bool) : (a && a) = a := by cases a <;> rfl

/-- `any`/`all` as folds with the monoid identity as seed (what the value graph builds). -/
def vany (xs : List Bool) : Bool := xs.foldr (· || ·) false
def vall (xs : List Bool) : Bool := xs.foldr (· && ·) true

/-- ANY denotes existence: `any xs = true ⟺ some element is true`. -/
theorem vany_iff (xs : List Bool) : vany xs = true ↔ ∃ x ∈ xs, x = true := by
  induction xs with
  | nil => simp [vany]
  | cons a t ih => simp [vany, List.foldr] at *; cases a <;> simp_all

/-- ALL denotes universality: `all xs = true ⟺ every element is true`. -/
theorem vall_iff (xs : List Bool) : vall xs = true ↔ ∀ x ∈ xs, x = true := by
  induction xs with
  | nil => simp [vall]
  | cons a t ih => simp [vall, List.foldr] at *; cases a <;> simp_all

/-- A predicate-mapped fold equals the fold over the mapped list — so `any(p(x) for x in xs)`
    (map then any) and `xs.some(p)` (any of the per-element predicate) denote the SAME Bool,
    which is what lets the cross-language forms converge to one `Reduce` node. -/
theorem any_map (p : Int → Bool) (xs : List Int) :
    vany (xs.map p) = (xs.foldr (fun x acc => p x || acc) false) := by
  induction xs with
  | nil => rfl
  | cons a t ih => simp [vany, List.foldr, List.map] at *; rw [ih]

end NoseBoolReduce
