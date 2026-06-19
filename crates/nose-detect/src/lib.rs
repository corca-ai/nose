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
mod contiguous;
mod detectors;
mod fragment;
mod il_utils;
mod locations;
mod lsh;
mod minhash;
mod model;
mod options;
mod orchestration;
mod reinvented;
mod report;
mod strict_exact;
mod units;
mod witness;

pub use contiguous::Stream;
pub(crate) use detectors::env_or;
pub use detectors::{
    exact_claim_eligible, exact_claim_eligible_parts, exact_safe_roots_by_span, CopyPasteDetector,
    Detector, ExactBehaviorDetector, StructuralDetector,
};
pub use fragment::{
    fragment_behavior, free_input_cids, synthesize_wrapper, Effect, EffectSite, Exit,
    FragmentContract, FragmentKind, Place, ProofFacts,
};
pub use model::{
    AbstractionHole, AbstractionWitness, Dump, DupPair, EnclosingUnit, EquivalenceWitness, Group,
    LineSpan, Loc, LocInit, Metrics, Report, UnitLoc,
};
pub use options::DetectOptions;
pub use orchestration::{detect, detect_from_units, detect_with_dump, file_stream, units_of_file};
pub use reinvented::{reinvented_helpers, ReinventedHelper};
pub use report::{is_test_loc, is_test_path, rank_families, RefactorFamily, VaryingSpot};
pub use units::{unit_dags_at, UnitFeat};
pub use witness::{graded_witness, GradedWitness, WitnessHole};
