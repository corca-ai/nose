use super::*;
use crate::{units_of_file, DetectOptions};
use nose_il::{FileId, Interner, Lang};

#[test]
fn deferred_signatures_match_immediate_extraction_and_empty_feature_rules() {
    let interner = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "unit.py",
        b"def sum_up(xs):\n    total = 0\n    for x in xs:\n        total += x\n    return total\n",
        Lang::Python,
        &interner,
    )
    .unwrap();
    for shape_features in [false, true] {
        for k in [1, 64, 128] {
            let opts = DetectOptions {
                min_lines: 1,
                min_tokens: 1,
                minhash_k: k,
                shape_features,
                ..Default::default()
            };
            let seeds = minhash::seeds(k);
            let mut units = (0..3)
                .flat_map(|_| units_of_file(&il, &interner, &opts))
                .collect::<Vec<_>>();
            assert!(!units.is_empty());
            let mut expected = units
                .iter()
                .map(|u| (u.shape_minhash.clone(), u.minhash.clone()))
                .collect::<Vec<_>>();
            let first = &mut units[0];
            first.value.clear();
            first.shapes.clear();
            expected[0] = if shape_features {
                (vec![u64::MAX; k], vec![u64::MAX; k])
            } else {
                (Vec::new(), Vec::new())
            };
            for unit in &mut units {
                unit.minhash.clear();
                unit.shape_minhash.clear();
            }
            finish(&mut units, shape_features, &seeds);
            assert_eq!(
                units
                    .iter()
                    .map(|u| (u.shape_minhash.clone(), u.minhash.clone()))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
