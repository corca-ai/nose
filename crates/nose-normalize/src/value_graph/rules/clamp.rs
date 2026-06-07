//! Proof-backed clamp canonicalization: `min(max(x, lo), hi)` / `max(min(x, hi), lo)` → a single
//! canonical clamp, but ONLY when the bound order `lo ≤ hi` is a proven fact and every operand is a
//! safe clamp integer. Distribution of the nested min/max is unsound without `lo ≤ hi` (it would
//! reorder the saturation), so the order fact is part of the proof obligation; the operands must be
//! integers because the float min/max NaN semantics differ.
//!
//! proof-obligation: normalize.value_graph.clamp

use super::super::{Builder, ValOp, ValueId, MAX_CODE, MIN_CODE};

pub(in super::super) fn apply(builder: &mut Builder<'_>, value: ValueId) -> Option<ValueId> {
    if !matches!(builder.nodes[value as usize].op, ValOp::Bin(o) if o == MIN_CODE || o == MAX_CODE)
    {
        return None;
    }
    let candidates = builder.clamp_minmax_candidates(value);
    if candidates.is_empty() {
        return None;
    }
    builder.clamp_candidate_count += 1;
    let mut proven: Vec<_> = candidates
        .into_iter()
        .filter(|&(x, lo, hi)| {
            builder.is_safe_clamp_integer_value(x)
                && builder.is_safe_clamp_integer_value(lo)
                && builder.is_safe_clamp_integer_value(hi)
                && builder.has_bound_order_fact(lo, hi)
        })
        .collect();
    proven.sort_unstable();
    proven.dedup();
    if proven.len() != 1 {
        return None;
    }
    builder.clamp_proof_backed_candidate_count += 1;
    let (x, lo, hi) = proven[0];
    builder.proof_backed_clamp_value(x, lo, hi)
}
