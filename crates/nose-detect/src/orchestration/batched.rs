//! Preserve candidate order and all accepted edges without retaining rejected pairs.
use super::{
    connected_pricing::connected_seed_indices, scoring::score_with_classes, stages::DetectionStages,
};
use crate::{
    candidates::{source_span_groups, structural_buckets},
    DetectOptions, Detector, UnitFeat,
};

pub(super) fn score(
    units: &[UnitFeat],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> DetectionStages {
    score_with_batch_size(units, opts, detector, 262_144)
}

fn score_with_batch_size(
    units: &[UnitFeat],
    opts: &DetectOptions,
    detector: &dyn Detector,
    batch_size: usize,
) -> DetectionStages {
    let buckets = structural_buckets(units, opts);
    let groups = source_span_groups(units);
    let paths = units
        .iter()
        .map(|unit| unit.path.as_str())
        .collect::<Vec<_>>();
    let weights = units
        .iter()
        .map(|unit| unit.connected_tokens.len())
        .collect::<Vec<_>>();
    let mut result = DetectionStages::fresh(Vec::new(), Vec::new(), Vec::new());
    let classes = detector.score_classes(units).filter(|ids| {
        assert_eq!(
            ids.len(),
            units.len(),
            "score classes must cover every unit"
        );
        ids.iter().collect::<rustc_hash::FxHashSet<_>>().len() <= units.len() / 2
    });
    crate::lsh::visit_batches(units.len(), &buckets, &groups, batch_size, |batch| {
        let (scored, accepted) =
            score_with_classes(units, batch, detector, opts.threshold, classes.as_deref());
        result.candidate_count += batch.len();
        result.accepted.extend(accepted);
        if opts.connected_witnesses {
            result.scored.extend(scored);
            // Each seed policy selects a top-k (or first-k) globally or per file.
            // The top-k of a union is unchanged by discarding non-top-k prefixes.
            // Stable pair order preserves every score tie and nested-seed priority.
            retain_connected_seeds(&mut result.scored, &paths, &weights, opts.threshold);
        }
    });
    result
}

fn retain_connected_seeds(
    scored: &mut Vec<super::ScoredCandidate>,
    paths: &[&str],
    weights: &[usize],
    threshold: f64,
) {
    let selected = connected_seed_indices(scored, paths, weights, threshold, true);
    *scored = selected.into_iter().map(|index| scored[index]).collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::scoring::score_ordinary_candidates;
    use crate::{candidates::structural_candidates, StructuralDetector};
    use nose_il::{Corpus, FileId, Interner, Lang};

    #[test]
    fn batch_boundaries_preserve_every_accepted_pair_and_connected_seed() {
        let opts = DetectOptions {
            min_lines: 1,
            min_tokens: 1,
            shape_candidates: true,
            connected_witnesses: true,
            ..Default::default()
        };
        let interner = Interner::new();
        let files = (0..32).map(|i| {
            let source = format!("def compute(xs):\n    total = 0\n    for x in xs:\n        if x > {}:\n            total += x * x\n    return total\n", i % 3);
            nose_frontend::lower_source(FileId(i), &format!("{i}.py"), source.as_bytes(), Lang::Python, &interner).unwrap()
        }).collect();
        let units = crate::corpus_features(&Corpus::new(interner, files), &opts).units;
        let detector = StructuralDetector::candidates(opts.jaccard_weight);
        let pairs = structural_candidates(&units, &opts);
        let (scored, accepted) =
            score_ordinary_candidates(&units, &pairs, &detector, opts.threshold);
        assert!(!accepted.is_empty());
        let paths = units
            .iter()
            .map(|unit| unit.path.as_str())
            .collect::<Vec<_>>();
        let weights = units
            .iter()
            .map(|unit| unit.connected_tokens.len())
            .collect::<Vec<_>>();
        let key = |s: &super::super::ScoredCandidate| (s.left, s.right, s.ordinary_score);
        let expected = connected_seed_indices(&scored, &paths, &weights, opts.threshold, true)
            .into_iter()
            .map(|i| key(&scored[i]))
            .collect::<Vec<_>>();
        assert!(!expected.is_empty());
        for size in [17, 257, 4096] {
            let result = score_with_batch_size(&units, &opts, &detector, size);
            assert_eq!(result.candidate_count, pairs.len());
            assert_eq!(result.accepted, accepted);
            assert_eq!(result.scored.iter().map(key).collect::<Vec<_>>(), expected);
            assert!(
                result.candidates.is_empty(),
                "rejected pair storage must be released"
            );
        }
    }
    #[test]
    fn compaction_preserves_overflow_ties_and_per_file_reservations() {
        let paths = (0..10_001)
            .map(|i| if i < 8_000 { "dense.rs" } else { "small.rs" })
            .collect::<Vec<_>>();
        let weights = vec![24; paths.len()];
        let candidates = (0..10_000)
            .map(|i| super::super::ScoredCandidate {
                left: i,
                right: i + 1,
                ordinary_score: (i % 3 != 0).then_some((i % 7) as f64 / 10.0),
            })
            .collect::<Vec<_>>();
        let key = |s: &super::super::ScoredCandidate| (s.left, s.right, s.ordinary_score);
        let expected = connected_seed_indices(&candidates, &paths, &weights, 0.7, true)
            .into_iter()
            .map(|i| key(&candidates[i]))
            .collect::<Vec<_>>();
        assert!(expected.len() < candidates.len());
        assert!(expected.iter().any(|(left, _, _)| *left >= 8_000));
        for size in [137, 1_027] {
            let mut retained = Vec::new();
            for batch in candidates.chunks(size) {
                retained.extend_from_slice(batch);
                retain_connected_seeds(&mut retained, &paths, &weights, 0.7);
            }
            assert_eq!(retained.iter().map(key).collect::<Vec<_>>(), expected);
        }
    }
}
