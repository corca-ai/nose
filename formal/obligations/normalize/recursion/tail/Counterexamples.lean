/-
Boundary counterexample for tail-recursion to loop rewrites.

Recursive call arguments are parallel bindings. A naive sequential update can
clobber a parameter that a later update still needs; Rust's `ordered_updates`
therefore bails out on cycles such as swaps.
-/

namespace NoseRecursionTailCounterexamples

def parallelSwap (state : Int × Int) : Int × Int :=
  (state.2, state.1)

def naiveSequentialSwap (state : Int × Int) : Int × Int :=
  let nextA := state.2
  let nextB := nextA
  (nextA, nextB)

theorem cyclic_binding_needs_hazard_safe_order :
    naiveSequentialSwap (1, 2) ≠ parallelSwap (1, 2) := by
  decide

end NoseRecursionTailCounterexamples
