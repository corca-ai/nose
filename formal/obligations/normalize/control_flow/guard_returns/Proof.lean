/-
Soundness of nose's control-flow canonicalizations, in a minimal statement language with
return semantics (mirrors `interp.rs`: a statement either returns a value or falls
through, and code after a return is dead).

Proven here:
  • guard_clause          — `if c {return a}; return b`  ≡  `if c {return a} else {return b}`
                            (value_graph.rs `process_block` guard-clause path narrowing)
  • dead_code_after_return — a statement after an unconditional return is unreachable
                            (value_graph.rs `process_block` dead-code break)
  • conjoined_guard_merge — `if a { if b { X } }`  ≡  `if a && b { X }`  (no else)
                            (cfg_norm.rs nested-guard merge)
  • continue_guard_unwrap — as a loop body, `if c { continue }; S`  ≡  `if !c { S }`
                            (cfg_norm.rs continue-guard unwrap)

Self-contained; checked by the formal obligation CI gate.
-/

namespace NoseControl

/-- A minimal statement: return a value, `continue` (skip the rest of this loop iteration), an
    if/else, a sequence, or a no-op. The branch condition is modeled by its truth value (what the
    value graph's path captures). -/
inductive Stmt where
  | ret  : Int → Stmt
  | cont : Stmt
  | ife  : Bool → Stmt → Stmt → Stmt
  | seq  : Stmt → Stmt → Stmt
  | skip : Stmt

/-- Value denotation: `some n` if the statement returns `n`, `none` if it falls through. A
    `continue` yields no value here (like a fall-through in the value channel); its loop-control
    effect is modeled separately by `run`/`proceeds` below. In a sequence, once the first part
    returns, the rest is dead. -/
def exec : Stmt → Option Int
  | .ret n     => some n
  | .skip      => none
  | .cont      => none
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

/-- CONJOINED-GUARD MERGE: an `if` nested directly inside an `if` with NO else on either level —
    `if a { if b { X } }` — denotes exactly the conjoined guard `if a && b { X }`. When `a` is
    false the outer else (`skip`) falls through, matching `a && b = false`; when `a` is true the
    inner `if b { X }` matches `b`-guarded `X`. So cfg_norm.rs collapsing the nested guard into a
    single `&&` condition is sound (it preserves short-circuit `a ∧ b`). -/
theorem conjoined_guard_merge (a b : Bool) (x : Stmt) :
    exec (.ife a (.ife b x .skip) .skip) = exec (.ife (a && b) x .skip) := by
  cases a <;> cases b <;> rfl

/-- Loop-control outcome of a body, used for `continue_guard_unwrap`: a body either falls through,
    issues a `continue`, or `return`s a value. -/
inductive Flow where
  | fall
  | cont
  | ret : Int → Flow

/-- Control-flow denotation: the `Flow` outcome of running a statement as a loop body. In a
    sequence, a `return` or `continue` in the first part skips the rest of THIS iteration. -/
def run : Stmt → Flow
  | .ret n     => .ret n
  | .cont      => .cont
  | .skip      => .fall
  | .ife c t e => if c then run t else run e
  | .seq a b   => match run a with
                  | .fall  => run b
                  | other  => other

/-- What the enclosing loop observes: a `return` exits with a value; a `continue` and a
    fall-through are INDISTINGUISHABLE — both advance to the next iteration. -/
def proceeds : Flow → Option Int
  | .ret n => some n
  | _      => none

/-- CONTINUE-GUARD UNWRAP: as a loop body, `if c { continue }; S` drives the loop identically to
    `if !c { S }`. When `c` holds, the first form `continue`s and the second falls through — both
    advance the loop (collapsed by `proceeds`) without running `S`; when `¬c`, both run `S`. So
    cfg_norm.rs rewriting `if c { continue } S` to `if !c { S }` is sound. -/
theorem continue_guard_unwrap (c : Bool) (s : Stmt) :
    proceeds (run (.seq (.ife c .cont .skip) s)) = proceeds (run (.ife (!c) s .skip)) := by
  cases c <;> rfl

end NoseControl
