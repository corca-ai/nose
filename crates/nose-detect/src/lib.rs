//! Clone detection over the normalized IL.
//!
//! Pipeline: normalize every file → extract units + features (value fingerprints,
//! subtree-shape multisets, linearized tags, MinHash signatures) → channel-specific
//! candidate generation (value for semantic, shape for near, token streams for syntax)
//! → scoring/acceptance → union-find clustering. The [`Detector`] trait makes the unit
//! scorer pluggable so simhash / tf-idf / graph variants can be compared later.

mod abstraction;
mod align;
mod candidates;
mod cluster;
mod connected;
mod contiguous;
mod detectors;
mod divergence_policy;
mod exact_policy;
mod fragment;
mod il_utils;
mod incremental;
mod locations;
mod lsh;
mod minhash;
mod model;
mod options;
mod orchestration;
mod reinvented;
mod report;
mod strict_exact;
mod test_paths;
mod units;
mod witness;

pub use align::multiset_jaccard;
pub use contiguous::Stream;
pub(crate) use detectors::env_or;
pub use detectors::{
    exact_safe_roots_by_span, CopyPasteDetector, Detector, ExactBehaviorDetector,
    StructuralDetector,
};
pub use divergence_policy::{
    divergence_policy, DivergenceGateDecision, DivergenceLane, DivergencePolicyDecision,
    DivergencePolicyInput, DivergenceScope, DivergenceTier, SharedLogicEvidence,
    DIVERGENT_EDIT_V2_POLICY,
};
pub use exact_policy::{exact_claim_eligible, exact_claim_eligible_parts};
pub use fragment::{
    fragment_behavior, fragment_input_projections, fragment_observes_mixed_exit, free_input_cids,
    recognized_fragment_contracts, synthesize_wrapper, synthesize_wrapper_with_module_strings,
    Effect, EffectSite, Exit, FragmentContract, FragmentKind, OracleInputProjection, Place,
    ProofFacts,
};
pub use incremental::{IncrementalDetectionState, IncrementalDetectionStats};
pub use model::{
    AbstractionHole, AbstractionWitness, ConnectedWitness, Dump, DupPair, EnclosingUnit,
    EquivalenceWitness, Group, LineSpan, Loc, LocInit, Metrics, Report, UnitLoc,
};
pub use options::DetectOptions;
pub use orchestration::{
    corpus_features, corpus_features_with_normalized, detect, detect_from_units,
    detect_from_units_incremental_session_with_accepted_coverage,
    detect_from_units_incremental_with_accepted_coverage, detect_from_units_with_accepted_coverage,
    detect_from_units_with_direct_accepted_coverage, detect_with_accepted_coverage,
    detect_with_direct_accepted_coverage, detect_with_dump, file_stream, units_of_file,
    CorpusFeatures,
};
pub use reinvented::{reinvented_helpers, ReinventedHelper};
pub use report::{
    is_test_loc, is_test_path, rank_families, AcceptedCoverage, AcceptedEdge, RefactorFamily,
    VaryingSpot,
};
pub use units::{
    default_product_oracle_fragment_candidates, default_product_unit_admission,
    default_product_value_fingerprint_context, unit_dags_at, ProductOracleFragment,
    ProductUnitAdmission, ProductUnitAdmissionInput, UnitFeat,
};
pub use witness::{graded_witness, GradedWitness, WitnessHole};
