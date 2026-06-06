import formal.lib.Il

namespace NoseIlArenaDeepCopy

open NoseFormal.Il

theorem copy_preserves_existing_bounds (arena : Arena) (node : Node) (id : Nat)
    (h : inBounds arena id) :
    inBounds (appendNode arena node) id := by
  exact old_id_in_bounds_after_append arena node id h

theorem copied_root_is_in_bounds (arena : Arena) (node : Node) :
    inBounds (appendNode arena node) arena.nodes.length := by
  exact appended_id_in_bounds arena node

end NoseIlArenaDeepCopy
