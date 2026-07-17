//! Value interning and proof-backed value canonicalization.
//!
//! proof-obligation: normalize.value_graph.algebra
// proof-claim: nose.claim.normalize.value_graph.algebra
//! proof-obligation: normalize.value_graph.clamp
//! proof-obligation: normalize.value_graph.compare
// proof-claim: nose.claim.normalize.value_graph.compare
//! proof-obligation: normalize.value_graph.min_max
// proof-claim: nose.claim.normalize.value_graph.min_max

mod binary;
mod byte_pack;
mod constants;
mod core;
mod lattice;
mod ordering;
mod references;
mod selection;
mod unary;
