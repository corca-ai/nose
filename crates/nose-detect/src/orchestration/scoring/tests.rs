use super::*;
use crate::{units_of_file, DetectOptions, StructuralDetector};
use nose_il::{FileId, Interner, Lang};

#[test]
fn memoization_preserves_argument_order_rejected_scores_nesting_and_chunk_order() {
    let interner = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let units = (0..3)
        .map(|i| {
            let il = nose_frontend::lower_source(
                FileId(i),
                &format!("{i}.py"),
                b"def f(x):\n    return x + 1\n",
                Lang::Python,
                &interner,
            )
            .unwrap();
            let mut unit = units_of_file(&il, &interner, &opts).remove(0);
            unit.value.clear();
            unit.lits.clear();
            unit.returns.clear();
            unit.anchors.clear();
            unit.exact_safe = false;
            unit.shapes = vec![1];
            unit.linear = if i == 1 {
                vec![1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1]
            } else {
                vec![0, 0, 1, 1, 1]
            };
            unit
        })
        .collect::<Vec<_>>();
    let pairs = (0..1_025)
        .flat_map(|_| [(0, 1), (1, 0), (2, 1), (1, 2), (0, 0)])
        .collect::<Vec<_>>();
    for detector in [
        StructuralDetector::candidates(0.5),
        StructuralDetector::strict(0.5),
    ] {
        let classes = detector.score_classes(&units).unwrap();
        assert_eq!(classes, [0, 1, 0]);
        assert_ne!(
            detector.score(&units[0], &units[1]),
            detector.score(&units[1], &units[0])
        );
        let threshold =
            (detector.score(&units[0], &units[1]) + detector.score(&units[1], &units[0])) / 2.0;
        let expected = pairs
            .iter()
            .map(|&(left, right)| {
                (
                    left,
                    right,
                    (!is_nested(&units[left], &units[right]))
                        .then(|| detector.score(&units[left], &units[right]).to_bits()),
                )
            })
            .collect::<Vec<_>>();
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let (scored, accepted) = pool.install(|| {
                score_with_classes(&units, &pairs, &detector, threshold, Some(&classes))
            });
            let actual = scored
                .iter()
                .map(|c| (c.left, c.right, c.ordinary_score.map(f64::to_bits)))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert_eq!(
                accepted,
                expected
                    .iter()
                    .filter_map(|&(left, right, bits)| {
                        bits.map(f64::from_bits)
                            .filter(|&score| score >= threshold)
                            .map(|score| (left, right, score))
                    })
                    .collect::<Vec<_>>()
            );
        }
    }
}
