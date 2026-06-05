/-
Soundness of nose's control-flow canonicalizations, in a minimal statement language with
return semantics (mirrors `interp.rs`: a statement either returns a value or falls
through, and code after a return is dead).

Proven here:
  • guard_clause          — `if c {return a}; return b`  ≡  `if c {return a} else {return b}`
                            (value_graph.rs `process_block` guard-clause path narrowing)
  • dead_code_after_return — a statement after an unconditional return is unreachable
                            (value_graph.rs `process_block` dead-code break)

Self-contained; check:  ~/.elan/bin/lean formal/Control.lean   (exit 0 = proofs hold)
-/

namespace NoseControl

/-- A minimal statement: return a value, an if/else, a sequence, or a no-op. The branch
    condition is modeled by its truth value (what the value graph's path captures). -/
inductive Stmt where
  | ret  : Int → Stmt
  | ife  : Bool → Stmt → Stmt → Stmt
  | seq  : Stmt → Stmt → Stmt
  | skip : Stmt

/-- Execution denotation: `some n` if the statement returns `n`, `none` if it falls
    through. In a sequence, once the first part returns, the rest is dead. -/
def exec : Stmt → Option Int
  | .ret n     => some n
  | .skip      => none
  | .ife c t e => if c then exec t else exec e
  | .seq a b   => match exec a with
                  | some n => some n
                  | none   => exec b

/-- GUARD-CLAUSE ≡ IF-ELSE: writing `if c { return a }; return b` (a guard clause whose
    then-branch exits, with a trailing `return b`) denotes exactly the same as the
    if-else `if c { return a } else { return b }`. So the value graph narrowing the path
    of the trailing statement by `¬c` (making it match the else-arm) is sound. -/
theorem guard_clause (c : Bool) (a b : Int) :
    exec (.seq (.ife c (.ret a) .skip) (.ret b)) = exec (.ife c (.ret a) (.ret b)) := by
  cases c <;> rfl

/-- DEAD CODE AFTER RETURN: anything sequenced after an unconditional `return a` is
    unreachable — the sequence denotes just the return. So the value graph stopping at an
    unconditional terminator (and not emitting later statements as sinks) is sound. -/
theorem dead_code_after_return (a : Int) (s : Stmt) :
    exec (.seq (.ret a) s) = exec (.ret a) := rfl

/-- Cascaded guards reduce the same way (sanity check): two stacked guard clauses match
    the nested if-else, so the path narrowing composes. -/
theorem guard_clause_cascade (c d : Bool) (a b e : Int) :
    exec (.seq (.ife c (.ret a) .skip) (.seq (.ife d (.ret b) .skip) (.ret e)))
      = exec (.ife c (.ret a) (.ife d (.ret b) (.ret e))) := by
  cases c <;> cases d <;> rfl

/-- TERNARY-RETURN DECOMPOSITION ≡ IF-ELSE: `return (a if c else b)` (a single return of a
    ternary value) denotes exactly the same as the two-armed `if c { return a } else
    { return b }`. So the value graph splitting a `Phi(c,a,b)` return into a `c`-guarded
    return of `a` and a `¬c`-guarded return of `b` (`emit_return`) is sound — and, composed
    with `guard_clause`, converges a nested ternary with an `elif` cascade. -/
theorem ternary_return (c : Bool) (a b : Int) :
    exec (.ret (if c then a else b)) = exec (.ife c (.ret a) (.ret b)) := by
  cases c <;> rfl

end NoseControl
