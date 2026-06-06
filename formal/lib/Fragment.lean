/-
Shared model for exact-fragment free-input extraction.

The Rust oracle collects canonical ids read by a fragment and removes ids bound
inside the fragment. The resulting free inputs become wrapper parameters.
-/

namespace NoseFormal.Fragment

def freeInputs (reads bound : List Nat) : List Nat :=
  reads.filter (fun cid => cid ∉ bound)

theorem free_inputs_exact (reads bound : List Nat) (cid : Nat) :
    cid ∈ freeInputs reads bound <-> cid ∈ reads /\ cid ∉ bound := by
  simp [freeInputs]

theorem free_input_is_read (reads bound : List Nat) (cid : Nat)
    (h : cid ∈ freeInputs reads bound) :
    cid ∈ reads := by
  exact (free_inputs_exact reads bound cid).mp h |>.left

theorem free_input_not_bound (reads bound : List Nat) (cid : Nat)
    (h : cid ∈ freeInputs reads bound) :
    cid ∉ bound := by
  exact (free_inputs_exact reads bound cid).mp h |>.right

theorem bound_input_removed (reads bound : List Nat) (cid : Nat)
    (h : cid ∈ bound) :
    cid ∉ freeInputs reads bound := by
  intro free
  exact free_input_not_bound reads bound cid free h

end NoseFormal.Fragment
