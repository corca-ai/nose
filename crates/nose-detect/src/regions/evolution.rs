//! Explicit family observations and bounded change evidence. This is a family
//! census within an analysis profile, never a census of every source region.
mod compare;
mod model;
pub use compare::{compare, Change, Comparison};
pub use model::{AnalysisSnapshot, FamilyObservation, MemberObservation};

#[cfg(test)]
mod tests;
