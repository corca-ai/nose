/-
Soundness of nose's comparison-direction and negated-comparison canonicalization.

The value graph reduces the `>`/`>=` family to `<`/`<=` with swapped operands, and rewrites a
negated comparison to its complement (`value_graph.rs` `mk`: `a > b → b < a`, `a >= b → b <= a`,
`!(a <= b) → a > b → b < a`, …). For these to be sound the *Bool* result the IL compares must be
invariant under the rewrite. This file proves that over `Int` (a total order), using the
decidable comparisons the interpreter (`interp.rs`) evaluates.

Self-contained; check:  ~/.elan/bin/lean formal/Compare.lean   (exit 0 = proofs hold)
-/

namespace NoseCompare

/-- The Bool-valued comparisons, mirroring `interp.rs` (which yields a `Value::Bool`). -/
def lt (a b : Int) : Bool := decide (a < b)
def le (a b : Int) : Bool := decide (a ≤ b)
def gt (a b : Int) : Bool := decide (a > b)
def ge (a b : Int) : Bool := decide (a ≥ b)

/-- COMPARISON DIRECTION: `a > b ≡ b < a` — the `Gt → Lt`+swap canon. -/
theorem gt_eq_lt_swap (a b : Int) : gt a b = lt b a := by
  simp [gt, lt]

/-- COMPARISON DIRECTION: `a >= b ≡ b <= a` — the `Ge → Le`+swap canon. -/
theorem ge_eq_le_swap (a b : Int) : ge a b = le b a := by
  simp [ge, le]

/-- NEGATED COMPARISON: `!(a <= b) ≡ a > b` — the `negate_cmp_code` canon for `<=`. Composed
    with `gt_eq_lt_swap` it converges `!(a <= b)` with the bare comparison `b < a`. -/
theorem not_le_eq_gt (a b : Int) : (!le a b) = gt a b := by
  unfold le gt
  by_cases h : a ≤ b
  · rw [decide_eq_true h, decide_eq_false (by omega : ¬ a > b)]; rfl
  · rw [decide_eq_false h, decide_eq_true (by omega : a > b)]; rfl

/-- NEGATED COMPARISON: `!(a < b) ≡ a >= b` — the `negate_cmp_code` canon for `<`. -/
theorem not_lt_eq_ge (a b : Int) : (!lt a b) = ge a b := by
  unfold lt ge
  by_cases h : a < b
  · rw [decide_eq_true h, decide_eq_false (by omega : ¬ a ≥ b)]; rfl
  · rw [decide_eq_false h, decide_eq_true (by omega : a ≥ b)]; rfl

/-- NEGATED EQUALITY: `!(a == b) ≡ a != b` — the `Eq`/`Ne` complement (`not-eq vs !=`). -/
theorem not_eq_eq_ne (a b : Int) : (!decide (a = b)) = decide (a ≠ b) := by
  simp

/-- The Bool-valued (in)equality, mirroring `interp.rs`. -/
def eq (a b : Int) : Bool := decide (a = b)
def ne (a b : Int) : Bool := decide (a ≠ b)

/-- LATTICE CANON: `(a ≤ b) ∧ (a ≠ b) ≡ a < b` on a total order — the
    `lattice_le_ne_to_lt` value-graph rule. Sound for any total order; here on `Int`. -/
theorem le_and_ne_eq_lt (a b : Int) : (le a b && ne a b) = lt a b := by
  unfold le ne lt
  by_cases h : a < b
  · rw [decide_eq_true (by omega : a ≤ b), decide_eq_true (by omega : a ≠ b),
        decide_eq_true h]; rfl
  · rw [decide_eq_false h]
    by_cases h2 : a ≤ b
    · rw [decide_eq_true h2, decide_eq_false (by omega : ¬ a ≠ b)]; rfl
    · rw [decide_eq_false h2]; rfl

/-- LATTICE CANON (dual): `(a < b) ∨ (a = b) ≡ a ≤ b` — the `lattice_lt_eq_to_le` rule. -/
theorem lt_or_eq_eq_le (a b : Int) : (lt a b || eq a b) = le a b := by
  unfold lt eq le
  by_cases h : a ≤ b
  · by_cases h2 : a < b
    · rw [decide_eq_true h2, decide_eq_true h]; rfl
    · rw [decide_eq_false h2, decide_eq_true (by omega : a = b), decide_eq_true h]; rfl
  · rw [decide_eq_false (by omega : ¬ a < b), decide_eq_false (by omega : ¬ a = b),
        decide_eq_false h]; rfl

end NoseCompare
