/-
Soundness of field-write normalization boundaries.

The interpreter records final field state with last-write-wins semantics. Writes to
different fields commute in the final state; writes to the same field do not.
-/

namespace NoseFieldWrites

abbrev Field := Nat
abbrev Value := Int
abbrev Store := Field → Value

def write (field : Field) (value : Value) (store : Store) : Store :=
  fun current => if current = field then value else store current

/-- Two writes to the same field collapse to the last write. -/
theorem same_field_last_write_wins (store : Store) (field : Field) (first second : Value) :
    write field second (write field first store) = write field second store := by
  funext current
  by_cases h : current = field <;> simp [write, h]

/-- Writes to different fields commute when only the final field state is observed. -/
theorem different_field_writes_commute
    (store : Store) (left right : Field) (leftValue rightValue : Value)
    (distinct : left ≠ right) :
    write left leftValue (write right rightValue store)
      = write right rightValue (write left leftValue store) := by
  funext current
  by_cases hleft : current = left
  · by_cases hright : current = right
    · exfalso
      exact distinct (Eq.trans (Eq.symm hleft) hright)
    · simp [write, hleft, distinct]
  · by_cases hright : current = right
    · have right_ne_left : right ≠ left := fun same => distinct (Eq.symm same)
      simp [write, hright, right_ne_left]
    · simp [write, hleft, hright]

end NoseFieldWrites
