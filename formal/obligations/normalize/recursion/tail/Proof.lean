import formal.lib.Recursion

namespace NoseRecursionTail

open NoseFormal.Recursion

theorem tail_recursion_loop_sound
    (step : state -> state) (ret : state -> out)
    (fuel : Nat) (start : state) :
    tailLoop step ret fuel start = tailRecursive step ret fuel start := by
  exact tail_loop_matches_tail_recursion step ret fuel start

end NoseRecursionTail
