use crate::{detectors::Detector, locations::is_nested, units::UnitFeat};
use rayon::prelude::*;

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
    let scored = candidates
        .par_iter()
        .map(|&(left, right)| ScoredCandidate {
            left,
            right,
            ordinary_score: (!is_nested(&units[left], &units[right]))
                .then(|| detector.score(&units[left], &units[right])),
        })
        .collect::<Vec<_>>();
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
