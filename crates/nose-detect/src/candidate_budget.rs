//! Preflight candidate work without allocating the quadratic pair arrays.
use crate::{DetectOptions, UnitFeat};
use rustc_hash::FxHashMap;

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

/// Counts pair emissions before cross-channel deduplication, matching the work
/// and peak allocation of clean generation. The same limit protects persistent
/// score indexes. It never accepts a truncated candidate set.
pub fn ensure_candidate_budget(
    units: &[UnitFeat],
    opts: &DetectOptions,
    limit: usize,
) -> Result<(), CandidateBudgetExceeded> {
    if !opts.structural {
        return Ok(());
    }
    let mut remaining = limit;
    let mut charge = |count: usize| {
        remaining = remaining
            .checked_sub(count)
            .ok_or(CandidateBudgetExceeded { limit })?;
        Ok(())
    };
    if opts.value_candidates {
        for bucket in crate::lsh::buckets(units.len(), |i| &units[i].minhash, opts.bands) {
            charge(pair_count(bucket.len()))?;
        }
        let mut exact: FxHashMap<&[u64], usize> = FxHashMap::default();
        for unit in units
            .iter()
            .filter(|unit| crate::exact_policy::exact_claim_eligible(unit))
        {
            *exact.entry(&unit.value).or_default() += 1;
        }
        for count in exact.into_values() {
            charge(pair_count(count))?;
        }
    }
    if opts.shape_candidates {
        for bucket in crate::lsh::buckets(units.len(), |i| &units[i].shape_minhash, opts.bands) {
            charge(pair_count(bucket.len()))?;
        }
        let mut anchors: FxHashMap<u64, usize> = FxHashMap::default();
        for anchor in units
            .iter()
            .flat_map(|unit| &unit.anchors)
            .filter(|a| a.weight >= nose_normalize::anchor_min_weight())
        {
            *anchors.entry(anchor.hash).or_default() += 1;
        }
        for count in anchors
            .into_values()
            .filter(|&n| n <= crate::candidates::anchor_max_df())
        {
            charge(pair_count(count).min(crate::candidates::ANCHOR_PAIR_CAP))?;
        }
    }
    Ok(())
}

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
