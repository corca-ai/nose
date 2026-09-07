use crate::UnitFeat;
use std::hash::Hash;

/// The complete input surface available to structural scoring. Class equality
/// compares every field, including metadata stricter than the score requires.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ScoreInputs<'a> {
    pub value: &'a [u64],
    pub shapes: &'a [u64],
    pub linear: &'a [u64],
    pub lits: &'a [u64],
    pub returns: &'a [u64],
    pub exact_safe: bool,
    pub anchors: &'a [nose_normalize::Anchor],
    pub semantic_pack_near_protocols: &'a [nose_semantics::SemanticPackNearProtocol],
}

impl<'a> From<&'a UnitFeat> for ScoreInputs<'a> {
    fn from(unit: &'a UnitFeat) -> Self {
        Self {
            value: &unit.value,
            shapes: &unit.shapes,
            linear: crate::align::alignment_input(&unit.linear),
            lits: &unit.lits,
            returns: &unit.returns,
            exact_safe: unit.exact_safe,
            anchors: &unit.anchors,
            semantic_pack_near_protocols: &unit.semantic_pack_near_protocols,
        }
    }
}

pub(super) fn classes<T: Eq + Hash>(inputs: impl Iterator<Item = T>) -> Vec<usize> {
    let mut seen = rustc_hash::FxHashMap::default();
    inputs
        .enumerate()
        .map(|(i, input)| *seen.entry(input).or_insert(i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_zero_class_is_interchangeable_in_both_arguments() {
        use crate::{DetectOptions, Detector, ExactBehaviorDetector};
        use nose_il::{FileId, Interner, Lang};
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(0),
            "f.py",
            b"def f(x):\n    return x * x + 7\n",
            Lang::Python,
            &interner,
        )
        .unwrap();
        let opts = DetectOptions {
            min_tokens: 1,
            min_lines: 1,
            ..Default::default()
        };
        let cases = [
            (false, vec![]),
            (false, (0..1000).collect()),
            (true, vec![]),
            (true, vec![1, 2, 3]),
            (true, vec![1, 2, 3, 4]),
            (true, vec![1, 2, 3, 4]),
            (true, vec![1, 2, 3, 5]),
            (false, vec![1, 2, 3, 4]),
        ];
        let units = cases
            .into_iter()
            .map(|(safe, values)| {
                let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
                unit.exact_safe = safe;
                unit.value = values;
                unit
            })
            .collect::<Vec<_>>();
        let detector = ExactBehaviorDetector;
        let classes = detector.score_classes(&units).unwrap();
        let prepared = crate::candidates::prepared_candidates(
            &units,
            &DetectOptions {
                value_candidates: true,
                value_lsh_candidates: false,
                shape_candidates: false,
                ..opts
            },
        );
        assert_eq!(prepared.buckets, vec![vec![4, 5]]);
        let evidence = prepared.exact_values.unwrap();
        assert!(std::ptr::eq(evidence.units(), units.as_slice()));
        let reused = detector.score_classes_from_exact(evidence).unwrap();
        for left in 0..units.len() {
            for right in 0..units.len() {
                assert_eq!(
                    classes[left] == classes[right],
                    reused[left] == reused[right]
                );
            }
        }
        for index in [1, 2, 3, 7] {
            assert_eq!(classes[index], classes[0]);
        }
        assert_eq!(classes[4], classes[5]);
        assert_ne!(classes[0], classes[4]);
        assert_ne!(classes[4], classes[6]);
        for (left, original) in units.iter().enumerate() {
            for (right, replacement) in units.iter().enumerate() {
                if classes[left] != classes[right] {
                    continue;
                }
                for other in &units {
                    assert_eq!(
                        detector.score(original, other).to_bits(),
                        detector.score(replacement, other).to_bits()
                    );
                    assert_eq!(
                        detector.score(other, original).to_bits(),
                        detector.score(other, replacement).to_bits()
                    );
                }
            }
        }
    }

    #[derive(PartialEq, Eq)]
    struct Collision<T>(T);

    impl<T> Hash for Collision<T> {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            0u8.hash(state);
        }
    }

    #[test]
    fn every_score_input_separates_classes_even_when_lookup_hashes_collide() {
        let empty = ScoreInputs {
            value: &[],
            shapes: &[],
            linear: &[],
            lits: &[],
            returns: &[],
            exact_safe: false,
            anchors: &[],
            semantic_pack_near_protocols: &[],
        };
        let anchors = [nose_normalize::Anchor {
            hash: 1,
            weight: 8,
            line_start: 1,
            line_end: 2,
            source_is_local: true,
        }];
        let protocols = [nose_semantics::SemanticPackNearProtocol {
            operation: nose_semantics::SemanticPackV1ProtocolOperation::CollectionFactory,
            provenance: None,
        }];
        let variants = [
            empty,
            ScoreInputs {
                value: &[1],
                ..empty
            },
            ScoreInputs {
                shapes: &[1],
                ..empty
            },
            ScoreInputs {
                linear: &[1],
                ..empty
            },
            ScoreInputs {
                lits: &[1],
                ..empty
            },
            ScoreInputs {
                returns: &[1],
                ..empty
            },
            ScoreInputs {
                exact_safe: true,
                ..empty
            },
            ScoreInputs {
                anchors: &anchors,
                ..empty
            },
            ScoreInputs {
                semantic_pack_near_protocols: &protocols,
                ..empty
            },
        ];
        let inputs = variants.iter().copied().chain(variants.iter().copied());
        let ids = classes(inputs.clone());
        assert_eq!(ids, classes(inputs.map(Collision)));
        assert_eq!(
            &ids[..variants.len()],
            &(0..variants.len()).collect::<Vec<_>>()
        );
        assert_eq!(&ids[..variants.len()], &ids[variants.len()..]);
    }
}
