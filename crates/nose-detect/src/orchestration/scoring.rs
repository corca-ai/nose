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
    let mut scored = Vec::with_capacity(candidates.len());
    let mut accepted = Vec::new();
    for batch in candidates.chunks(4096) {
        let batch = batch
            .par_iter()
            .map(|&(left, right)| ScoredCandidate {
                left,
                right,
                ordinary_score: (!is_nested(&units[left], &units[right]))
                    .then(|| detector.score(&units[left], &units[right])),
            })
            .collect::<Vec<_>>();
        accepted.extend(batch.iter().filter_map(|candidate| {
            candidate
                .ordinary_score
                .filter(|&score| score >= threshold)
                .map(|score| (candidate.left, candidate.right, score))
        }));
        scored.extend(batch);
    }
    (scored, accepted)
}
