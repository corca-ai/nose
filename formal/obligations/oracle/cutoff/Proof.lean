import formal.lib.Oracle

namespace NoseOracleCutoff

open NoseFormal.Oracle

theorem oracle_core_contains_only_structural_phases (phase : Phase)
    (h : phase ∈ oraclePipeline) :
    structuralCore phase := by
  exact oracle_pipeline_structural phase h

theorem oracle_cutoff_excludes_semantic_canons (phase : Phase)
    (h : phase ∈ oraclePipeline) :
    Not (semanticCanon phase) := by
  exact oracle_excludes_semantic phase h

theorem oracle_cutoff_excludes_recursion :
    Phase.recursion ∉ oraclePipeline := by
  exact recursion_not_in_oracle

theorem oracle_cutoff_excludes_algebra :
    Phase.algebra ∉ oraclePipeline := by
  exact algebra_not_in_oracle

end NoseOracleCutoff
