//! Disjoint buckets with one exact scoring class need no general row refinement.
//! The scorer's existing class contract proves interchangeability; location-based
//! admission and complete accepted-pair order still belong to AcceptedPairs.
use super::super::{accepted::AcceptedPairs, connected_pricing::path_classes};
use super::DetectionStages;
use crate::{DetectOptions, Detector, UnitFeat};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

pub(super) fn score(
    units: &[UnitFeat],
    opts: &DetectOptions,
    detector: &dyn Detector,
    buckets: &[Vec<u32>],
    spans: &[usize],
    classes: &[usize],
) -> Option<DetectionStages> {
    if opts.connected_witnesses || !homogeneous_disjoint(units.len(), buckets, classes) {
        return None;
    }
    let observations = buckets
        .par_iter()
        .map(|bucket| {
            let mut span_counts = FxHashMap::default();
            for &unit in bucket {
                *span_counts.entry(spans[unit as usize]).or_insert(0usize) += 1;
            }
            let count = bucket.len() * (bucket.len() - 1) / 2
                - span_counts.values().map(|n| n * (n - 1) / 2).sum::<usize>();
            let representative = &units[bucket[0] as usize];
            (count, detector.score(representative, representative))
        })
        .collect::<Vec<_>>();
    // Unrepresented units point to row zero, whose target set stays empty. This
    // avoids one empty row and several maps for every noncandidate unit.
    let mut members = vec![Vec::new()];
    members.extend(
        buckets
            .iter()
            .map(|bucket| bucket.iter().map(|&unit| unit as usize).collect()),
    );
    let relations = observations
        .iter()
        .enumerate()
        .filter_map(|(index, &(_, score))| {
            (score >= opts.threshold).then_some((index + 1, index + 1, score))
        })
        .collect();
    let paths = units
        .iter()
        .map(|unit| unit.path.as_str())
        .collect::<Vec<_>>();
    let mut result = DetectionStages::fresh(Vec::new(), Vec::new(), Vec::new());
    result.candidate_count = observations.iter().map(|&(count, _)| count).sum();
    result.accepted = AcceptedPairs::rows(units, &path_classes(&paths), &members, relations);
    Some(result)
}

fn homogeneous_disjoint(count: usize, buckets: &[Vec<u32>], classes: &[usize]) -> bool {
    let mut seen = vec![false; count];
    for bucket in buckets {
        let Some(&first) = bucket.first() else {
            return false;
        };
        for &unit in bucket {
            let unit = unit as usize;
            if seen[unit] || classes[unit] != classes[first as usize] {
                return false;
            }
            seen[unit] = true;
        }
    }
    true
}

#[cfg(test)]
mod tests;
