/-
Shared model for exact-fragment observable effects.

This is a small Lean mirror of `fragment/contract.rs`: append/index effects are
observed directly by the behavior trace, while field writes need a proven receiver
place because final field state is keyed by receiver+field place.
-/

namespace NoseFormal.Effects

inductive Effect where
  | append
  | indexWrite
  | fieldWrite
  | other
  deriving DecidableEq

inductive Place where
  | this
  | param : Nat -> Place
  | localAlloc : Nat -> Place
  | field : Place -> Nat -> Place
  | index : Place -> Nat -> Place
  | unknown
  deriving DecidableEq

def requiresProvenPlace : Effect -> Prop
  | .fieldWrite => True
  | _ => False

def exactSafe : Place -> Prop
  | .this => True
  | .param _ => True
  | .localAlloc _ => True
  | .field base _ => exactSafe base
  | .index base _ => exactSafe base
  | .unknown => False

structure EffectSite where
  effect : Effect
  place : Option Place

def siteProven (site : EffectSite) : Prop :=
  requiresProvenPlace site.effect ->
    exists place, site.place = some place /\ exactSafe place

def writesProven (sites : List EffectSite) : Prop :=
  forall site, site ∈ sites -> siteProven site

theorem append_site_proven (place : Option Place) :
    siteProven { effect := Effect.append, place := place } := by
  intro impossible
  cases impossible

theorem index_site_proven (place : Option Place) :
    siteProven { effect := Effect.indexWrite, place := place } := by
  intro impossible
  cases impossible

theorem other_site_proven (place : Option Place) :
    siteProven { effect := Effect.other, place := place } := by
  intro impossible
  cases impossible

theorem field_site_proven_iff (place : Option Place) :
    siteProven { effect := Effect.fieldWrite, place := place } <->
      exists proven, place = some proven /\ exactSafe proven := by
  constructor
  · intro h
    exact h trivial
  · intro h _
    exact h

theorem field_unknown_not_proven :
    Not (siteProven { effect := Effect.fieldWrite, place := some Place.unknown }) := by
  intro h
  rcases h trivial with ⟨place, hplace, safe⟩
  cases hplace
  simp [exactSafe] at safe

theorem field_over_unknown_not_safe (field : Nat) :
    Not (exactSafe (Place.field Place.unknown field)) := by
  intro h
  exact h

theorem nested_unknown_not_safe (field key : Nat) :
    Not (exactSafe (Place.field (Place.index Place.unknown key) field)) := by
  intro h
  exact h

theorem writes_proven_cons (site : EffectSite) (rest : List EffectSite) :
    writesProven (site :: rest) <-> siteProven site /\ writesProven rest := by
  constructor
  · intro h
    constructor
    · exact h site (by simp)
    · intro item mem
      exact h item (by simp [mem])
  · intro h item mem
    rcases h with ⟨head, tail⟩
    simp at mem
    rcases mem with same | inRest
    · cases same
      exact head
    · exact tail item inRest

end NoseFormal.Effects
