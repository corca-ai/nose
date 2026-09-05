//! Preflight candidate work without allocating the quadratic pair arrays.
use crate::{DetectOptions, UnitFeat};

#[derive(Debug)]
pub struct CandidateBudgetExceeded {
    pub limit: usize,
}
impl std::fmt::Display for CandidateBudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "candidate work exceeds limit of {} pairs; narrow analysis roots or increase NOSE_MAX_CANDIDATE_PAIRS", self.limit)
    }
}
impl std::error::Error for CandidateBudgetExceeded {}

/// Counts the union of candidate pairs across all channels without allocating
/// the pair array. The same limit protects persistent score indexes. No accepted
/// search is truncated; repeated routes to one pair consume one budget slot.
pub fn ensure_candidate_budget(
    units: &[UnitFeat],
    opts: &DetectOptions,
    limit: usize,
) -> Result<(), CandidateBudgetExceeded> {
    if !opts.structural {
        return Ok(());
    }
    let buckets = crate::candidates::structural_buckets(units, opts);
    let count = crate::lsh::candidate_count(
        units.len(),
        &buckets,
        &crate::candidates::source_span_groups(units),
        limit,
    )
    .ok_or(CandidateBudgetExceeded { limit })?;
    if std::env::var_os("NOSE_TIME").is_some() {
        eprintln!(
            "  [candidate-budget] units={} pairs={} limit={limit}",
            units.len(),
            count
        );
    }
    Ok(())
}

#[cfg(test)]
fn pair_count(n: usize) -> usize {
    n.saturating_mul(n.saturating_sub(1)) / 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn dense_bucket_work_is_counted_without_pairs() {
        let signature = [7; 128];
        let buckets = crate::lsh::buckets(10_000, |_| &signature, 32);
        assert_eq!(buckets.len(), 1);
        assert_eq!(super::pair_count(buckets[0].len()), 49_995_000);
    }
}
