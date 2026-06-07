//! Proof-sensitive value-graph rules.
//!
//! Any new semantic rewrite in this module tree must have a matching
//! `formal/obligations/normalize/value_graph/<rule>/meta.toml` entry. The
//! formal-obligation linter checks that directory/file pairing mechanically.

pub(super) mod factor_distribute;
pub(super) mod promise_then;
