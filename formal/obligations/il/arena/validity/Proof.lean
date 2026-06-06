import formal.lib.Il

namespace NoseIlArenaValidity

open NoseFormal.Il

theorem validate_root_in_bounds (arena : Arena) (h : valid arena) :
    inBounds arena arena.root := by
  exact valid_root_in_bounds arena h

theorem validate_unit_in_bounds (arena : Arena) (unit : Nat)
    (h : valid arena) (mem : unit ∈ arena.units) :
    inBounds arena unit := by
  exact valid_unit_in_bounds arena unit h mem

theorem validate_child_in_bounds (arena : Arena) (id child : Nat) (node : Node)
    (h : valid arena)
    (found : nodeAt? arena id = some node)
    (mem : child ∈ node.children) :
    inBounds arena child := by
  exact valid_child_in_bounds arena id child node h found mem

end NoseIlArenaValidity
