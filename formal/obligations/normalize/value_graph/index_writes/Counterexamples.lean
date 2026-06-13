/-
Counterexample: indexed-element writes to the SAME (aliasing) place are order-sensitive, so a
forward must not survive a possibly-aliasing write, and element writes cannot be treated as an
order-insensitive place-merge. This is why the value graph keeps element writes as ORDERED
effects and clears `index_env` on every write.
-/

namespace NoseIndexWriteCounterexamples

abbrev Place := Nat
abbrev Value := Int
abbrev Store := Place → Value

def write (place : Place) (value : Value) (store : Store) : Store :=
  fun current => if current = place then value else store current

def read (place : Place) (store : Store) : Value := store place

/-- Writing the same place twice is order-sensitive — the read sees the LAST write — so a forward
    established before a same-place (aliasing) write would be stale; it must be invalidated. -/
theorem same_place_write_order_matters :
    ∃ (store : Store) (place : Place) (first second : Value),
      read place (write place second (write place first store))
        ≠ read place (write place first (write place second store)) :=
  ⟨fun _ => 0, 0, 1, 2, by simp [read, write]⟩

end NoseIndexWriteCounterexamples
