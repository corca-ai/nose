use super::*;
use crate::orchestration::scoring::score_ordinary_candidates;
use nose_il::{Corpus, FileId, Interner, Lang};

struct ClassifiedDetector;

impl Detector for ClassifiedDetector {
    fn name(&self) -> &str {
        "classified-test"
    }
    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64 {
        if a.exact_safe && b.exact_safe {
            0.875
        } else {
            0.125
        }
    }
}

fn test_units(opts: &DetectOptions) -> Vec<UnitFeat> {
    let interner = Interner::new();
    let source = "def compute(xs):\n    total = 0\n    for x in xs:\n        if x > 0:\n            total += x * x\n    return total\n";
    let files = (0..5)
        .map(|id| {
            nose_frontend::lower_source(
                FileId(id),
                &format!("{id}.py"),
                source.as_bytes(),
                Lang::Python,
                &interner,
            )
            .unwrap()
        })
        .collect();
    crate::corpus_features(&Corpus::new(interner, files), opts).units
}

#[test]
fn disjoint_rows_preserve_full_pairs_counts_and_unrepresented_units() {
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        connected_witnesses: false,
        ..Default::default()
    };
    let mut units = test_units(&opts);
    assert!(units.len() >= 8);
    for (index, unit) in units.iter_mut().enumerate() {
        unit.exact_safe = index % 2 == 0;
    }
    units[0].start_line = 10;
    units[0].end_line = 20;
    for (index, start, end) in [(2, 10, 20), (4, 12, 15)] {
        units[index].path = units[0].path.clone();
        units[index].start_line = start;
        units[index].end_line = end;
    }
    let classes = units
        .iter()
        .map(|unit| usize::from(unit.exact_safe))
        .collect::<Vec<_>>();
    // Leave the last unit outside every bucket; it must never inherit row one's edges.
    let buckets = [false, true]
        .map(|safe| {
            (0..units.len() - 1)
                .filter(|&index| units[index].exact_safe == safe)
                .map(|index| index as u32)
                .collect::<Vec<_>>()
        })
        .to_vec();
    let spans = crate::candidates::source_span_groups(&units);
    let pairs = crate::lsh::pairs(units.len(), &buckets, &spans);
    assert!(!pairs.is_empty());
    assert!(!pairs.contains(&(0, 2)), "equal spans are not candidates");
    assert!(
        pairs.contains(&(0, 4)),
        "strict nesting is excluded only during scoring"
    );
    for threshold in [0.0, 0.5, 0.9] {
        let opts = DetectOptions { threshold, ..opts };
        let (_, expected) =
            score_ordinary_candidates(&units, &pairs, &ClassifiedDetector, threshold);
        let actual = score(
            &units,
            &opts,
            &ClassifiedDetector,
            &buckets,
            &spans,
            &classes,
        )
        .unwrap();
        assert_eq!(actual.candidate_count, pairs.len());
        assert_eq!(actual.accepted, expected);
        assert!(actual.scored.is_empty());
    }
}

#[test]
fn noncandidate_units_do_not_disable_homogeneous_score_reuse() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct CountingDetector(AtomicUsize);
    impl Detector for CountingDetector {
        fn name(&self) -> &str {
            "homogeneous-counting-test"
        }
        fn score(&self, _a: &UnitFeat, _b: &UnitFeat) -> f64 {
            self.0.fetch_add(1, Ordering::Relaxed);
            1.0
        }
        fn score_classes(&self, units: &[UnitFeat]) -> Option<Vec<usize>> {
            Some(
                units
                    .iter()
                    .enumerate()
                    .map(|(i, u)| if u.exact_safe { 0 } else { i + 1 })
                    .collect(),
            )
        }
    }
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        connected_witnesses: false,
        value_lsh_candidates: false,
        shape_candidates: false,
        ..Default::default()
    };
    let mut units = test_units(&opts);
    assert!(units.len() >= 8);
    for (index, unit) in units.iter_mut().enumerate() {
        unit.exact_safe = index < 3;
        unit.value = (0..crate::exact_policy::EXACT_VALUE_MIN as u64).collect();
        unit.path = format!("member-{index}.py");
    }
    let detector = CountingDetector(AtomicUsize::new(0));
    let classes = detector.score_classes(&units).unwrap();
    assert!(
        classes
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > units.len() / 2
    );
    let result = super::super::score_with_batch_size(&units, &opts, &detector, 17);
    assert_eq!(result.candidate_count, 3);
    assert_eq!(result.accepted, vec![(0, 1, 1.0), (0, 2, 1.0), (1, 2, 1.0)]);
    assert_eq!(detector.0.load(Ordering::Relaxed), 1);
}

#[test]
fn overlapping_or_mixed_buckets_require_general_refinement() {
    assert!(homogeneous_disjoint(
        4,
        &[vec![0, 2], vec![1, 3]],
        &[0, 1, 0, 1]
    ));
    assert!(!homogeneous_disjoint(
        4,
        &[vec![0, 2], vec![2, 3]],
        &[0, 1, 0, 0]
    ));
    assert!(!homogeneous_disjoint(4, &[vec![0, 1]], &[0, 1, 0, 1]));
    assert!(!homogeneous_disjoint(4, &[vec![]], &[0, 1, 0, 1]));
}
