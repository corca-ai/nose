/-
Soundness of the graded witness's anti-unification core (#315).

The witness aligns two units' value graphs by their least general generalization
(anti-unification): identical sub-structure is kept; each spot where the two differ
becomes a HOLE. The `equal_modulo_holes` grade then claims the two are equal except at
those holes. This file machine-checks that structural core over a term model:

  1. `au_matches_left` / `au_matches_right` — both inputs MATCH their anti-unification:
     it is a genuine generalization (a hole matches anything; non-hole structure must
     coincide). So everything the generalization pins down (its non-hole part) is shared
     by both, and the holes are the only freedom — exactly "equal modulo the holes".
  2. `holefree_au_eq` — when the anti-unification has NO holes, the two terms are equal.
     This is the degenerate k = 0 case the witness reports as fully-equal (and the exact
     channel's `equal_modulo_holes` with zero holes).

Boundary (modeled as a precondition, like `factor_distribute`'s Num gate): this proves
the STRUCTURAL claim over abstract op/leaf identity. The Rust witness additionally
requires that names both units consume resolve to the SAME referent (`compare_referents`)
— two leaves with equal abstract key but different referents are split by that check, not
by this model. That gate's correctness stays empirical (the referent/decorator/sink
checks and the soundness battery); this file is the structural half it rests on.
-/

namespace NoseGradedWitness

/-- A value term: a leaf identified by an abstract key (op/const identity), or an
op-keyed binary node. Binary is without loss of generality for the alignment core. -/
inductive Term where
  | leaf : Nat → Term
  | bin : Nat → Term → Term → Term

/-- A generalization: a hole (a varying spot), or structure mirroring `Term`. -/
inductive Pat where
  | hole : Pat
  | leaf : Nat → Pat
  | bin : Nat → Pat → Pat → Pat

/-- Anti-unification (least general generalization): identical structure is kept,
any disagreement collapses to a hole. -/
def au : Term → Term → Pat
  | .leaf a, .leaf b => if a = b then .leaf a else .hole
  | .bin oa la ra, .bin ob lb rb =>
      if oa = ob then .bin oa (au la lb) (au ra rb) else .hole
  | _, _ => .hole

/-- `Matches t p`: the term `t` conforms to the generalization `p` — a hole accepts
anything; non-hole structure must coincide node-for-node. -/
def Matches : Term → Pat → Prop
  | _, .hole => True
  | .leaf a, .leaf b => a = b
  | .bin oa la ra, .bin ob lb rb => oa = ob ∧ Matches la lb ∧ Matches ra rb
  | _, _ => False

/-- A generalization with no holes — it pins down the whole term. -/
def HoleFree : Pat → Prop
  | .hole => False
  | .leaf _ => True
  | .bin _ l r => HoleFree l ∧ HoleFree r

/-- The left input always matches its anti-unification: the LGG is a generalization. -/
theorem au_matches_left : ∀ t1 t2 : Term, Matches t1 (au t1 t2)
  | .leaf a, .leaf b => by
      by_cases h : a = b <;> simp [au, Matches, h]
  | .leaf _, .bin _ _ _ => by simp [au, Matches]
  | .bin _ _ _, .leaf _ => by simp [au, Matches]
  | .bin oa la ra, .bin ob lb rb => by
      by_cases h : oa = ob
      · simp [au, Matches, h, au_matches_left la lb, au_matches_left ra rb]
      · simp [au, Matches, h]

/-- Symmetrically, the right input matches its anti-unification. -/
theorem au_matches_right : ∀ t1 t2 : Term, Matches t2 (au t1 t2)
  | .leaf a, .leaf b => by
      by_cases h : a = b <;> simp [au, Matches, h] <;> omega
  | .leaf _, .bin _ _ _ => by simp [au, Matches]
  | .bin _ _ _, .leaf _ => by simp [au, Matches]
  | .bin oa la ra, .bin ob lb rb => by
      by_cases h : oa = ob
      · subst h
        simp [au, Matches, au_matches_right la lb, au_matches_right ra rb]
      · simp [au, Matches, h]

/-- If the anti-unification has no holes, the two terms are equal — the k = 0 case the
witness reports as fully-equal. -/
theorem holefree_au_eq : ∀ t1 t2 : Term, HoleFree (au t1 t2) → t1 = t2
  | .leaf a, .leaf b => by
      by_cases h : a = b <;> simp [au, HoleFree, h]
  | .leaf _, .bin _ _ _ => by simp [au, HoleFree]
  | .bin _ _ _, .leaf _ => by simp [au, HoleFree]
  | .bin oa la ra, .bin ob lb rb => by
      by_cases h : oa = ob
      · subst h
        intro hf
        simp [au, HoleFree] at hf
        rw [holefree_au_eq la lb hf.1, holefree_au_eq ra rb hf.2]
      · simp [au, HoleFree, h]

end NoseGradedWitness
