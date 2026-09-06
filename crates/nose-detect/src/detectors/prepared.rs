//! Exact multiset intersections shared across a row of structural comparisons.
use super::{ScoreInputs, StructuralDetector, UnitFeat};
use rustc_hash::FxHashMap;
use std::cell::OnceCell;

#[derive(Clone, Copy)]
pub(super) struct Overlap<'a> {
    pub value: f64,
    pub equal_value: bool,
    pub shape: f64,
    pub shared_anchor: u32,
    pub alignment: &'a OnceCell<f64>,
}

pub trait PreparedScores: Sync {
    /// Scores in the order of `right`, exactly equal to the original detector.
    fn row(&self, left: usize, right: &[usize]) -> Vec<f64>;
}

pub(super) struct ExactScores(Vec<Option<usize>>);

impl ExactScores {
    pub(super) fn new(units: &[&UnitFeat]) -> Self {
        let mut values = FxHashMap::default();
        Self(
            units
                .iter()
                .map(|unit| {
                    if !crate::exact_policy::exact_claim_eligible(unit) {
                        return None;
                    }
                    let next = values.len();
                    Some(*values.entry(unit.value.as_slice()).or_insert(next))
                })
                .collect(),
        )
    }
}

impl PreparedScores for ExactScores {
    fn row(&self, left: usize, right: &[usize]) -> Vec<f64> {
        right
            .iter()
            .map(|&right| {
                if self.0[left].is_some() && self.0[left] == self.0[right] {
                    1.0
                } else {
                    0.0
                }
            })
            .collect()
    }
}

struct Multisets<'a> {
    classes: Vec<usize>,
    values: Vec<&'a [u64]>,
    postings: FxHashMap<u64, Postings>,
}

#[derive(Default)]
struct Postings {
    // Common features start at one everywhere and record only missing classes.
    complement: bool,
    members: Vec<usize>,
    repeated: Vec<(usize, usize)>,
}

impl<'a> Multisets<'a> {
    fn new(inputs: impl Iterator<Item = &'a [u64]>) -> Self {
        let mut ids = FxHashMap::default();
        let mut values = Vec::new();
        let classes = inputs
            .map(|input| {
                let next = values.len();
                let id = *ids.entry(input).or_insert(next);
                if id == next {
                    values.push(input);
                }
                id
            })
            .collect();
        let mut postings: FxHashMap<u64, Postings> = FxHashMap::default();
        for (id, values) in values.iter().enumerate() {
            for run in values.chunk_by(|a, b| a == b) {
                let posting = postings.entry(run[0]).or_default();
                posting.members.push(id);
                if run.len() > 1 {
                    posting.repeated.push((id, run.len() - 1));
                }
            }
        }
        for posting in postings.values_mut() {
            if posting.members.len() > values.len() / 2 {
                let mut present = posting.members.iter().copied().peekable();
                posting.members = (0..values.len())
                    .filter(|id| {
                        if present.peek() == Some(id) {
                            present.next();
                            false
                        } else {
                            true
                        }
                    })
                    .collect();
                posting.complement = true;
            }
        }
        Self {
            classes,
            values,
            postings,
        }
    }

    fn estimated_row_work(&self, left: usize) -> usize {
        let values = self.values[self.classes[left]];
        values.chunk_by(|a, b| a == b).fold(
            values
                .len()
                .saturating_mul(2)
                .saturating_add(self.values.len()),
            |work, run| {
                let posting = &self.postings[&run[0]];
                work.saturating_add(posting.members.len())
                    .saturating_add(if run.len() > 1 {
                        posting.repeated.len()
                    } else {
                        0
                    })
            },
        )
    }

    fn row(&self, left: usize) -> Vec<f64> {
        let values = self.values[self.classes[left]];
        let baseline = values
            .chunk_by(|a, b| a == b)
            .filter(|run| self.postings[&run[0]].complement)
            .count();
        let mut intersections = vec![baseline; self.values.len()];
        for run in values.chunk_by(|a, b| a == b) {
            let posting = &self.postings[&run[0]];
            if posting.complement {
                for &id in &posting.members {
                    intersections[id] -= 1;
                }
            } else {
                for &id in &posting.members {
                    intersections[id] += 1;
                }
            }
            if run.len() > 1 {
                for &(id, extra) in &posting.repeated {
                    intersections[id] += extra.min(run.len() - 1);
                }
            }
        }
        intersections
            .into_iter()
            .zip(&self.values)
            .map(|(intersection, other)| {
                let union = values.len() + other.len() - intersection;
                if union == 0 {
                    1.0
                } else {
                    intersection as f64 / union as f64
                }
            })
            .collect()
    }
}

pub(super) struct StructuralScores<'a> {
    detector: &'a StructuralDetector,
    inputs: Vec<ScoreInputs<'a>>,
    values: Multisets<'a>,
    shapes: Multisets<'a>,
    linear: Vec<usize>,
    linear_count: usize,
    anchors: Option<Anchors<'a>>,
}

struct Anchors<'a> {
    inputs: Vec<&'a [nose_normalize::Anchor]>,
    postings: FxHashMap<(u64, usize), Vec<(usize, u32)>>,
}

impl<'a> Anchors<'a> {
    fn new(inputs: impl Iterator<Item = &'a [nose_normalize::Anchor]>) -> Self {
        let inputs = inputs.collect::<Vec<_>>();
        let mut postings: FxHashMap<_, Vec<_>> = FxHashMap::default();
        for (id, anchors) in inputs.iter().enumerate() {
            for run in anchors.chunk_by(|a, b| a.hash == b.hash) {
                for (occurrence, anchor) in run.iter().enumerate() {
                    postings
                        .entry((anchor.hash, occurrence))
                        .or_default()
                        .push((id, anchor.weight));
                }
            }
        }
        Self { inputs, postings }
    }

    fn row(&self, left: usize) -> Vec<u32> {
        let floor = nose_normalize::anchor_min_weight();
        let mut shared = vec![0; self.inputs.len()];
        for run in self.inputs[left].chunk_by(|a, b| a.hash == b.hash) {
            for (occurrence, anchor) in run.iter().enumerate() {
                if anchor.weight < floor {
                    continue;
                }
                for &(id, weight) in &self.postings[&(anchor.hash, occurrence)] {
                    if weight >= floor {
                        shared[id] = shared[id].max(anchor.weight.min(weight));
                    }
                }
            }
        }
        shared
    }
}

impl<'a> StructuralScores<'a> {
    pub(super) fn new(detector: &'a StructuralDetector, units: &[&'a UnitFeat]) -> Self {
        let inputs = units
            .iter()
            .map(|&u| ScoreInputs::from(u))
            .collect::<Vec<_>>();
        let values = Multisets::new(inputs.iter().map(|i| i.value));
        let shapes = Multisets::new(inputs.iter().map(|i| i.shapes));
        let mut linear_ids = FxHashMap::default();
        let linear = inputs
            .iter()
            .map(|i| {
                let next = linear_ids.len();
                *linear_ids.entry(i.linear).or_insert(next)
            })
            .collect();
        let linear_count = linear_ids.len();
        let anchors = detector
            .candidate_mode
            .then(|| Anchors::new(inputs.iter().map(|i| i.anchors)));
        Self {
            detector,
            inputs,
            values,
            shapes,
            linear,
            linear_count,
            anchors,
        }
    }
}

impl PreparedScores for StructuralScores<'_> {
    fn row(&self, left: usize, right: &[usize]) -> Vec<f64> {
        // A small fraction of representatives can still contain very long
        // fingerprints. Compare feature work before choosing repeated merges.
        let direct = right.len() < 8
            || (right.len() < self.inputs.len() / 8 && {
                let a = &self.inputs[left];
                let merges = right.iter().fold(0usize, |work, &right| {
                    let b = &self.inputs[right];
                    work.saturating_add(a.value.len().min(b.value.len()))
                        .saturating_add(a.shapes.len().min(b.shapes.len()))
                });
                let indexed = self
                    .values
                    .estimated_row_work(left)
                    .saturating_add(self.shapes.estimated_row_work(left));
                merges < indexed
            });
        if direct {
            return right
                .iter()
                .map(|&right| {
                    self.detector
                        .score_inputs(&self.inputs[left], &self.inputs[right], None)
                })
                .collect();
        }
        let values = self.values.row(left);
        let shapes = self.shapes.row(left);
        let alignment = vec![OnceCell::new(); self.linear_count];
        let anchors = self.anchors.as_ref().map(|a| a.row(left));
        right
            .iter()
            .map(|&right| {
                self.detector.score_inputs(
                    &self.inputs[left],
                    &self.inputs[right],
                    Some(Overlap {
                        value: values[self.values.classes[right]],
                        equal_value: self.values.classes[left] == self.values.classes[right],
                        shape: shapes[self.shapes.classes[right]],
                        shared_anchor: anchors.as_ref().map_or(0, |a| a[right]),
                        alignment: &alignment[self.linear[right]],
                    }),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DetectOptions, Detector};
    use nose_il::{FileId, Interner, Lang};

    #[test]
    fn indexed_anchors_preserve_ordered_duplicate_matching_and_weight_floor() {
        let floor = nose_normalize::anchor_min_weight();
        let data = (0..32)
            .map(|i| {
                (0..i % 13)
                    .map(|j| nose_normalize::Anchor {
                        hash: (j / 3) as u64,
                        weight: floor.saturating_sub(2) + (i + j) % 7,
                        line_start: i,
                        line_end: i + j,
                        source_is_local: i % 2 == 0,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let index = Anchors::new(data.iter().map(Vec::as_slice));
        for (left, a) in data.iter().enumerate() {
            let row = index.row(left);
            for (right, b) in data.iter().enumerate() {
                assert_eq!(row[right], crate::candidates::shared_anchor_weight(a, b));
            }
        }
    }

    #[test]
    fn indexed_multisets_preserve_multiplicity_and_empty_identity() {
        let data = [
            vec![],
            vec![1],
            vec![1, 1, 2],
            vec![1, 2, 2, 2],
            vec![7, 8],
            vec![1, 1, 2],
        ];
        let index = Multisets::new(data.iter().map(Vec::as_slice));
        for (left, a) in data.iter().enumerate() {
            let row = index.row(left);
            for (right, b) in data.iter().enumerate() {
                assert_eq!(
                    row[index.classes[right]].to_bits(),
                    crate::align::multiset_jaccard(a, b).to_bits()
                );
            }
        }
    }

    #[test]
    fn sparse_rows_with_long_repeated_fingerprints_keep_direct_scores() {
        let units = crate::test_support::scoring_units(128)
            .into_iter()
            .map(|mut unit| {
                unit.value = vec![7; 4096];
                unit.exact_safe = false;
                unit
            })
            .collect::<Vec<_>>();
        let detector = StructuralDetector::candidates(0.75);
        let prepared = detector
            .prepare_scores(&units.iter().collect::<Vec<_>>())
            .unwrap();
        let rights = (0..8).collect::<Vec<_>>();
        for (&right, score) in rights.iter().zip(prepared.row(0, &rights)) {
            assert_eq!(
                score.to_bits(),
                detector.score(&units[0], &units[right]).to_bits()
            );
        }
    }

    #[test]
    fn alignment_classes_ignore_only_the_existing_unread_suffix() {
        let mut units = crate::test_support::scoring_units(2);
        let (left, right) = units.split_at_mut(1);
        let (left, right) = (&mut left[0], &mut right[0]);
        left.linear = vec![7; 4096];
        right.linear = left.linear.clone();
        let limit = crate::align::alignment_input(&left.linear).len();
        right.linear[limit..].fill(11);
        let detector = StructuralDetector::strict(0.75);
        let classes = detector.score_classes(&units).unwrap();
        assert_eq!(classes[0], classes[1]);
        assert_eq!(
            crate::align::ransac_ratio(&units[0].linear, &units[1].linear),
            1.0
        );
        units[1].linear[limit - 1] = 13;
        let classes = detector.score_classes(&units).unwrap();
        assert_ne!(classes[0], classes[1]);
    }

    #[test]
    fn prepared_scores_match_direct_scores_including_rejected_pairs() {
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(0),
            "f.py",
            b"def f(x):\n    y = x * x\n    return y + 1\n",
            Lang::Python,
            &interner,
        )
        .unwrap();
        let opts = DetectOptions {
            min_lines: 1,
            min_tokens: 1,
            ..Default::default()
        };
        let units = (0..80)
            .map(|i| {
                let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
                let features = |salt| {
                    let mut values = (0..i % 19)
                        .map(|j| ((i * salt + j * j) % 23) as u64)
                        .collect::<Vec<_>>();
                    values.sort_unstable();
                    values
                };
                unit.value = features(3);
                unit.shapes = features(5);
                unit.lits = features(7);
                unit.returns = features(11);
                unit.linear = (0..i % 23).map(|j| ((i + j) % 7) as u64).collect();
                unit.exact_safe = i % 3 == 0;
                unit
            })
            .collect::<Vec<_>>();
        let representatives = units.iter().collect::<Vec<_>>();
        let exact = crate::ExactBehaviorDetector;
        let prepared = exact.prepare_scores(&representatives).unwrap();
        let rights = (0..units.len()).collect::<Vec<_>>();
        for left in 0..units.len() {
            for (&right, score) in rights.iter().zip(prepared.row(left, &rights)) {
                assert_eq!(
                    score.to_bits(),
                    exact.score(&units[left], &units[right]).to_bits()
                );
            }
        }
        for candidate in [false, true] {
            for exact in [false, true] {
                for threshold in [0.0, 0.7, 0.95] {
                    let mut detector = if candidate {
                        StructuralDetector::candidates(0.75)
                    } else {
                        StructuralDetector::strict(0.75)
                    }
                    .with_threshold(threshold);
                    detector.exact_behavior = exact;
                    let prepared = detector.prepare_scores(&representatives).unwrap();
                    for left in 0..units.len() {
                        for rights in [(0..units.len()).rev().collect::<Vec<_>>(), vec![0, 3, 7]] {
                            let scores = prepared.row(left, &rights);
                            for (&right, score) in rights.iter().zip(scores) {
                                assert_eq!(
                                    score.to_bits(),
                                    detector.score(&units[left], &units[right]).to_bits(),
                                    "{candidate}/{exact}/{threshold}: {left}/{right}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
