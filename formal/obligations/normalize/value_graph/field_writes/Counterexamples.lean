/-
Counterexample for treating same-field writes as order-insensitive.
-/

namespace NoseFieldWriteCounterexamples

abbrev Field := Nat
abbrev Value := Int
abbrev Store := Field → Value

def write (field : Field) (value : Value) (store : Store) : Store :=
  fun current => if current = field then value else store current

/-- Same-field writes are not commutative; the last value wins. -/
theorem same_field_write_order_matters :
    ∃ store field first second,
      write field second (write field first store)
        ≠ write field first (write field second store) :=
  ⟨fun _ => 0, 0, 1, 2, by
    intro h
    have pointwise := congrFun h 0
    simp [write] at pointwise⟩

end NoseFieldWriteCounterexamples
