/-
Soundness of the named `factor_distribute` value-graph rule.

The Rust rule rewrites `x*f + y*f` to `(x+y)*f` only when all leaves are proven numeric.
Over `Int`, this is distributivity. Free-monoid counterexamples explain why the Num gate is
required.
-/

namespace NoseFactorDistribute

inductive Expr where
  | lit : Int → Expr
  | var : Nat → Expr
  | add : Expr → Expr → Expr
  | mul : Expr → Expr → Expr

def eval (env : Nat → Int) : Expr → Int
  | .lit n => n
  | .var i => env i
  | .add a b => eval env a + eval env b
  | .mul a b => eval env a * eval env b

theorem distrib_sound (env : Nat → Int) (x y f : Expr) :
    eval env (.mul (.add x y) f) = eval env (.add (.mul x f) (.mul y f)) := by
  simp [eval, Int.add_mul]

end NoseFactorDistribute
