/-
Soundness of indexed-element READ-FORWARDING (#337, #342).

The value graph forwards a `base[index]` read to the value most recently written to that exact
`(base, index)` place (`index_env`, `forwarded_index_read`/`record_index_write`). Two facts make
this sound:

1. forwarding the just-written place returns the written value; and
2. a write to a DISTINCT place leaves a place's value (hence its forward) unchanged — so the
   forward need only be invalidated by a POSSIBLY-ALIASING write.

The implementation is strictly more conservative than (2) demands: it clears every forward on any
ordered effect / branch / loop and only installs a forward for an unconditional straight-line
write, so a forward fires only when it is the most-recent write with NO intervening write at all —
a subset of "no intervening distinct write". Element WRITES themselves stay ordered effects, sound
under index aliasing (see the companion counterexample).
-/

namespace NoseIndexWrites

/-- A `(base, index)` place, abstracted to a key. -/
abbrev Place := Nat
abbrev Value := Int
abbrev Store := Place → Value

def write (place : Place) (value : Value) (store : Store) : Store :=
  fun current => if current = place then value else store current

def read (place : Place) (store : Store) : Value := store place

/-- Read-forwarding is sound: a read of the just-written place returns the written value. -/
theorem forward_read_after_write_sound (store : Store) (place : Place) (value : Value) :
    read place (write place value store) = value := by
  simp [read, write]

/-- A write to a DISTINCT place preserves a place's value, so a forward for `place` survives a
    write to any provably-different place; only a possibly-aliasing write can invalidate it. -/
theorem distinct_write_preserves_read
    (store : Store) (place other : Place) (value : Value) (distinct : place ≠ other) :
    read place (write other value store) = read place store := by
  simp [read, write, distinct]

end NoseIndexWrites
