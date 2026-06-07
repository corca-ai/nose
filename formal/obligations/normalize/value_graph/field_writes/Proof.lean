/-
Soundness of field-write normalization boundaries.

The value graph records final field state per receiver+field place with last-write-wins
semantics. Writes to different places commute in the final state; writes to the same
place do not.
-/

namespace NoseFieldWrites

abbrev Place := Nat
abbrev Value := Int
abbrev Store := Place → Value

def write (place : Place) (value : Value) (store : Store) : Store :=
  fun current => if current = place then value else store current

/-- Two writes to the same receiver+field place collapse to the last write. -/
theorem same_field_last_write_wins (store : Store) (place : Place) (first second : Value) :
    write place second (write place first store) = write place second store := by
  funext current
  by_cases h : current = place <;> simp [write, h]

/-- Writes to different receiver+field places commute when only final state is observed. -/
theorem different_field_writes_commute
    (store : Store) (left right : Place) (leftValue rightValue : Value)
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
