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
