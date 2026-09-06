use super::*;
use crate::orchestration::{
    connected_pricing::connected_seed_indices, scoring::score_ordinary_candidates,
};
use nose_il::{FileId, Interner, Lang};

struct OrderedDetector;
impl Detector for OrderedDetector {
    fn name(&self) -> &str {
        "ordered-input-fixture"
    }
    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64 {
        ((a.linear[0] * 3 + b.linear[0] * 5) % 11) as f64 / 10.0
    }
}

#[test]
fn compressed_rows_preserve_location_exclusions_asymmetry_and_seed_reservations() {
    let interner = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        connected_witnesses: true,
        ..Default::default()
    };
    let il = nose_frontend::lower_source(FileId(0), "fixture.py", b"def compute(xs):\n    total = 0\n    for x in xs:\n        if x > 2:\n            total += x * x\n    return total\n", Lang::Python, &interner).unwrap();
    let template = crate::units_of_file(&il, &interner, &opts).remove(0);
    assert!(!template.connected_tokens.is_empty());
    let units = (0..160)
        .map(|i| {
            let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
            unit.path = format!("{}.py", i % 7);
            unit.start_line = (i / 14 * 3) as u32;
            unit.end_line = unit.start_line + if i % 5 == 0 { 15 } else { 2 };
            unit.linear = vec![(i % 4) as u64];
            unit.connected_tokens.resize(
                if i % 3 == 0 { 0 } else { 24 },
                template.connected_tokens[0],
            );
            unit
        })
        .collect::<Vec<_>>();
    let classes = units
        .iter()
        .map(|u| 1000 + u.linear[0] as usize)
        .collect::<Vec<_>>();
    let buckets = vec![
        (0..120).collect(),
        (35..160).collect(),
        (0..160).step_by(3).collect(),
        vec![0, 64, 129],
    ];
    let spans = crate::candidates::source_span_groups(&units);
    let pairs = crate::lsh::pairs(units.len(), &buckets, &spans);
    let (scored, accepted) =
        score_ordinary_candidates(&units, &pairs, &OrderedDetector, opts.threshold);
    let paths = units.iter().map(|u| u.path.as_str()).collect::<Vec<_>>();
    let weights = units
        .iter()
        .map(|u| u.connected_tokens.len())
        .collect::<Vec<_>>();
    let key = |s: &crate::orchestration::ScoredCandidate| {
        (s.left, s.right, s.ordinary_score.map(f64::to_bits))
    };
    let expected = connected_seed_indices(&scored, &paths, &weights, opts.threshold, true)
        .into_iter()
        .map(|i| key(&scored[i]))
        .collect::<Vec<_>>();
    assert!(!accepted.is_empty());
    assert!(!expected.is_empty());
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        for batch in [31, 4096] {
            let actual = pool.install(|| {
                score(
                    &units,
                    &opts,
                    &OrderedDetector,
                    &buckets,
                    &spans,
                    &classes,
                    batch,
                )
            });
            assert_eq!(actual.candidate_count, pairs.len());
            assert_eq!(actual.accepted, accepted);
            assert_eq!(actual.scored.iter().map(key).collect::<Vec<_>>(), expected);
        }
    }
}
