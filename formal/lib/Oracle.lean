/-
Shared model for the independent-oracle cutoff.

`NormalizeOptions.oracle` stops after the structural core, before semantic
canonicalizations such as recursion, dataflow, DCE, CFG normalization, and algebra.
-/

namespace NoseFormal.Oracle

inductive Phase where
  | desugar
  | alpha
  | recursion
  | dataflow
  | dce
  | cfgStructure
  | algebra
  | cfgRun
  deriving DecidableEq

def structuralCore : Phase -> Prop
  | .desugar => True
  | .alpha => True
  | _ => False

def semanticCanon : Phase -> Prop
  | .recursion => True
  | .dataflow => True
  | .dce => True
  | .cfgStructure => True
  | .algebra => True
  | .cfgRun => True
  | _ => False

def oraclePipeline : List Phase :=
  [Phase.desugar, Phase.alpha]

theorem oracle_pipeline_structural (phase : Phase)
    (h : phase ∈ oraclePipeline) :
    structuralCore phase := by
  cases phase <;> simp [oraclePipeline, structuralCore] at h ⊢

theorem oracle_excludes_semantic (phase : Phase)
    (h : phase ∈ oraclePipeline) :
    Not (semanticCanon phase) := by
  cases phase <;> simp [oraclePipeline, semanticCanon] at h ⊢

theorem recursion_not_in_oracle :
    Phase.recursion ∉ oraclePipeline := by
  simp [oraclePipeline]

theorem algebra_not_in_oracle :
    Phase.algebra ∉ oraclePipeline := by
  simp [oraclePipeline]

end NoseFormal.Oracle
