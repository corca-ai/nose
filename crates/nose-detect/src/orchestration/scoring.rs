use crate::{detectors::Detector, locations::is_nested, units::UnitFeat};
use rayon::prelude::*;

#[cfg(test)]
mod tests;

pub(crate) type AcceptedPair = (usize, usize, f64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScoredCandidate {
    pub(crate) left: usize,
    pub(crate) right: usize,
    /// Nested pairs are intentionally not scored by the ordinary detector.
    pub(crate) ordinary_score: Option<f64>,
}

pub(super) fn score_ordinary_candidates(
    units: &[UnitFeat],
    candidates: &[(usize, usize)],
    detector: &dyn Detector,
    threshold: f64,
) -> (Vec<ScoredCandidate>, Vec<AcceptedPair>) {
    score_with_classes(units, candidates, detector, threshold, None)
}

pub(super) fn score_with_classes(
    units: &[UnitFeat],
    candidates: &[(usize, usize)],
    detector: &dyn Detector,
    threshold: f64,
    classes: Option<&[usize]>,
) -> (Vec<ScoredCandidate>, Vec<AcceptedPair>) {
    let scored = if let Some(classes) = classes {
        candidates
            .par_chunks(4096)
            .map(|chunk| {
                // Private to this parallel chunk: no shared locks, and at most one
                // score entry per candidate. Argument order matters to RANSAC.
                let mut memo = rustc_hash::FxHashMap::default();
                chunk
                    .iter()
                    .map(|&(left, right)| ScoredCandidate {
                        left,
                        right,
                        ordinary_score: (!is_nested(&units[left], &units[right])).then(|| {
                            *memo
                                .entry((classes[left], classes[right]))
                                .or_insert_with(|| detector.score(&units[left], &units[right]))
                        }),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    } else {
        candidates
            .par_iter()
            .map(|&(left, right)| ScoredCandidate {
                left,
                right,
                ordinary_score: (!is_nested(&units[left], &units[right]))
                    .then(|| detector.score(&units[left], &units[right])),
            })
            .collect::<Vec<_>>()
    };
    let accepted = scored
        .iter()
        .filter_map(|candidate| {
            candidate
                .ordinary_score
                .filter(|&score| score >= threshold)
                .map(|score| (candidate.left, candidate.right, score))
        })
        .collect();
    (scored, accepted)
}
