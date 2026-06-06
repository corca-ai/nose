/-
Shared model for the IL arena validity contract.

The Rust `Il::validate` checks root, unit roots, and edge targets against the
node arena. This model captures those invariants without attempting to prove the
entire Rust implementation.
-/

namespace NoseFormal.Il

structure Node where
  children : List Nat

structure Arena where
  nodes : List Node
  root : Nat
  units : List Nat

def inBounds (arena : Arena) (id : Nat) : Prop :=
  id < arena.nodes.length

def listGet? : List Node -> Nat -> Option Node
  | [], _ => none
  | node :: _, 0 => some node
  | _ :: rest, id + 1 => listGet? rest id

def nodeAt? (arena : Arena) (id : Nat) : Option Node :=
  listGet? arena.nodes id

def nodeValid (arena : Arena) (node : Node) : Prop :=
  forall child, child ∈ node.children -> inBounds arena child

def valid (arena : Arena) : Prop :=
  inBounds arena arena.root /\
    (forall id node, nodeAt? arena id = some node -> nodeValid arena node) /\
    (forall unit, unit ∈ arena.units -> inBounds arena unit)

theorem valid_root_in_bounds (arena : Arena) (h : valid arena) :
    inBounds arena arena.root := by
  exact h.left

theorem valid_unit_in_bounds (arena : Arena) (unit : Nat)
    (h : valid arena) (mem : unit ∈ arena.units) :
    inBounds arena unit := by
  exact h.right.right unit mem

theorem valid_child_in_bounds (arena : Arena) (id child : Nat) (node : Node)
    (h : valid arena)
    (found : nodeAt? arena id = some node)
    (mem : child ∈ node.children) :
    inBounds arena child := by
  exact h.right.left id node found child mem

def appendNode (arena : Arena) (node : Node) : Arena :=
  { arena with nodes := arena.nodes ++ [node] }

theorem old_id_in_bounds_after_append (arena : Arena) (node : Node) (id : Nat)
    (h : inBounds arena id) :
    inBounds (appendNode arena node) id := by
  unfold inBounds at h
  unfold inBounds appendNode
  simp
  exact Nat.lt_trans h (Nat.lt_succ_self arena.nodes.length)

theorem appended_id_in_bounds (arena : Arena) (node : Node) :
    inBounds (appendNode arena node) arena.nodes.length := by
  simp [inBounds, appendNode]

end NoseFormal.Il
