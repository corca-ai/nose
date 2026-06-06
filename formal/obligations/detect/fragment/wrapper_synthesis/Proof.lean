import formal.lib.Il
import formal.lib.Fragment

namespace NoseFragmentWrapper

open NoseFormal.Il

def wrapperArity (freeInputs : List Nat) : Nat :=
  freeInputs.length

theorem wrapper_arity_matches_free_inputs (freeInputs : List Nat) :
    wrapperArity freeInputs = freeInputs.length := by
  rfl

theorem wrapper_body_root_in_bounds (arena : Arena) (body : Node) :
    inBounds (appendNode arena body) arena.nodes.length := by
  exact appended_id_in_bounds arena body

theorem wrapper_preserves_existing_bounds (arena : Arena) (body : Node) (id : Nat)
    (h : inBounds arena id) :
    inBounds (appendNode arena body) id := by
  exact old_id_in_bounds_after_append arena body id h

end NoseFragmentWrapper
