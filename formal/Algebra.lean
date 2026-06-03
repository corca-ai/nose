/-
Soundness of nose's commutative/associative operand canonicalization.

The value graph canonicalizes an associative-commutative operator chain (e.g. `+`) by
FLATTENING it to its leaf operands and SORTING them by a structural hash
(`value_graph.rs`: `flatten_into` + `operands.sort_by_key`). For this to be sound the
fingerprint must be invariant to that rewrite, i.e. the rewrite must preserve behavior.

This file proves exactly that for `+` over `Int`: flattening an add-tree to a leaf list
preserves its denotation, and ANY permutation of that leaf list (the sort is one) has the
same denotation. Hence sorting flattened `+`-operands is denotation-preserving — sound.

Self-contained: no Mathlib. Compile:  ~/.elan/bin/lean formal/Algebra.lean
-/

namespace NoseAlgebra

/-- A tiny pure IL fragment: integer literals, variables, an associative-commutative
    `add`, a `neg`, and `sub` (the model for the operators the value graph normalizes). -/
inductive Expr where
  | lit : Int → Expr
  | var : Nat → Expr
  | add : Expr → Expr → Expr
  | neg : Expr → Expr
  | sub : Expr → Expr → Expr

/-- Denotation of an expression under an environment. This mirrors `interp.rs`. -/
def eval (env : Nat → Int) : Expr → Int
  | .lit n   => n
  | .var i   => env i
  | .add a b => eval env a + eval env b
  | .neg a   => - eval env a
  | .sub a b => eval env a - eval env b

/-- SUBTRACTION CANONICALIZATION IS SOUND: the value graph rewrites `a - b` to
    `a + (-b)` (value_graph.rs, before the AC `+` normalization). Over `Int` this is
    denotation-preserving — so the rewrite cannot change behavior. -/
theorem sub_eq_add_neg (env : Nat → Int) (a b : Expr) :
    eval env (.sub a b) = eval env (.add a (.neg b)) := by
  simp [eval, Int.sub_eq_add_neg]

/-- NEGATION DISTRIBUTES over addition: `-(a + b) ≡ (-a) + (-b)` over `Int` (the value
    graph's `Neg(Add(x,y)) → Add(Neg x, Neg y)` canonicalization). -/
theorem neg_add_distrib (env : Nat → Int) (a b : Expr) :
    eval env (.neg (.add a b)) = eval env (.add (.neg a) (.neg b)) := by
  simp [eval, Int.neg_add]

/-- Flatten an add-tree to its list of leaf summands — the value graph's `flatten_into`. -/
def leaves : Expr → List Expr
  | .add a b => leaves a ++ leaves b
  | e        => [e]

/-- Sum the denotations of a list of leaves. -/
def sumLeaves (env : Nat → Int) : List Expr → Int
  | []      => 0
  | e :: es => eval env e + sumLeaves env es

/-- `sumLeaves` distributes over append (uses associativity + `0` identity of `Int.+`). -/
theorem sumLeaves_append (env : Nat → Int) (xs ys : List Expr) :
    sumLeaves env (xs ++ ys) = sumLeaves env xs + sumLeaves env ys := by
  induction xs with
  | nil => simp [sumLeaves]
  | cons x xs ih => simp [sumLeaves, ih, Int.add_assoc]

/-- FLATTENING IS SOUND: an add-tree denotes the sum of its flattened leaves. -/
theorem eval_eq_sumLeaves (env : Nat → Int) (e : Expr) :
    eval env e = sumLeaves env (leaves e) := by
  induction e with
  | lit n => simp [eval, leaves, sumLeaves]
  | var i => simp [eval, leaves, sumLeaves]
  | add a b iha ihb =>
    simp only [eval, leaves, sumLeaves_append, iha, ihb]
  | neg a _ => simp [leaves, sumLeaves]
  | sub a b _ _ => simp [leaves, sumLeaves]

/-- List permutation (the relation the structural-hash sort induces on the leaf list). -/
inductive Perm : List Expr → List Expr → Prop where
  | nil : Perm [] []
  | cons (x) {xs ys} : Perm xs ys → Perm (x :: xs) (x :: ys)
  | swap (x y xs) : Perm (y :: x :: xs) (x :: y :: xs)
  | trans {xs ys zs} : Perm xs ys → Perm ys zs → Perm xs zs

/-- SORTING IS SOUND: any permutation of the leaf list has the same sum (uses the
    commutativity + associativity of `Int.+`). The value graph's sort is such a
    permutation, so it cannot change the denotation. -/
theorem sumLeaves_perm (env : Nat → Int) {xs ys : List Expr} (h : Perm xs ys) :
    sumLeaves env xs = sumLeaves env ys := by
  induction h with
  | nil => rfl
  | cons x _ ih => simp [sumLeaves, ih]
  | swap x y xs =>
    simp only [sumLeaves]
    rw [← Int.add_assoc, ← Int.add_assoc, Int.add_comm (eval env y) (eval env x)]
  | trans _ _ ih1 ih2 => exact ih1.trans ih2

/-- MAIN: the value graph's flatten-then-sort canonicalization of an AC `+` chain is
    denotation-preserving. If `e₂`'s flattened leaves are a permutation of `e₁`'s
    (which sorting guarantees — both sort to the same order), then `e₁` and `e₂` have
    equal denotation on every environment. Fingerprint-equal ⇒ behavior-equal, for this
    rule. -/
theorem canon_sound (env : Nat → Int) (e₁ e₂ : Expr)
    (h : Perm (leaves e₁) (leaves e₂)) :
    eval env e₁ = eval env e₂ := by
  rw [eval_eq_sumLeaves env e₁, eval_eq_sumLeaves env e₂]
  exact sumLeaves_perm env h

end NoseAlgebra
