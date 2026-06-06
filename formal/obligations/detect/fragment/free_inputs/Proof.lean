import formal.lib.Fragment

namespace NoseFragmentFreeInputs

open NoseFormal.Fragment

theorem free_inputs_are_reads_not_bound (reads bound : List Nat) (cid : Nat) :
    cid ∈ freeInputs reads bound <-> cid ∈ reads /\ cid ∉ bound := by
  exact free_inputs_exact reads bound cid

theorem free_input_reads_subset (reads bound : List Nat) (cid : Nat)
    (h : cid ∈ freeInputs reads bound) :
    cid ∈ reads := by
  exact free_input_is_read reads bound cid h

theorem bound_cids_are_removed (reads bound : List Nat) (cid : Nat)
    (h : cid ∈ bound) :
    cid ∉ freeInputs reads bound := by
  exact bound_input_removed reads bound cid h

end NoseFragmentFreeInputs
