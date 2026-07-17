//! Statement, block, loop, and local-reduction processing for value-graph construction.
//!
//! proof-obligation: normalize.control_flow.guard_returns
//! proof-obligation: normalize.value_graph.bool_reduce
// proof-claim: nose.claim.normalize.value_graph.bool_reduce

mod block_return;
mod containers;
mod entry;
mod guards;
mod loop_idioms;
mod loops;
mod reductions;
mod returns;
mod statements;
mod static_errors;
