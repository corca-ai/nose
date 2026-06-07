/-
Counterexample for treating same-place field writes as order-insensitive.
-/

namespace NoseFieldWriteCounterexamples

abbrev Place := Nat
abbrev Value := Int
abbrev Store := Place → Value

def write (place : Place) (value : Value) (store : Store) : Store :=
  fun current => if current = place then value else store current

/-- Same receiver+field-place writes are not commutative; the last value wins. -/
theorem same_field_write_order_matters :
    ∃ store place first second,
      write place second (write place first store)
        ≠ write place first (write place second store) :=
  ⟨fun _ => 0, 0, 1, 2, by
    intro h
    have pointwise := congrFun h 0
    simp [write] at pointwise⟩

end NoseFieldWriteCounterexamples
