use super::*;

/// Exhaustive sorting defines the policy independently of the streaming heaps.
fn reference(
    candidates: &[ScoredCandidate],
    paths: &[usize],
    weights: &[usize],
    threshold: f64,
) -> Vec<usize> {
    let eligible = |i: usize| {
        weights[candidates[i].left].min(weights[candidates[i].right]) >= MIN_PRODUCT_SEED_NODES
    };
    let scored = |i: usize| candidates[i].ordinary_score.is_some_and(|s| s < threshold);
    let nested = |i: usize| candidates[i].ordinary_score.is_none();
    let rank = |ids: &mut Vec<usize>| {
        ids.sort_unstable_by(|&a, &b| {
            candidates[b]
                .ordinary_score
                .unwrap_or(0.0)
                .total_cmp(&candidates[a].ordinary_score.unwrap_or(0.0))
                .then(a.cmp(&b))
        })
    };
    let select = |predicate: &dyn Fn(usize) -> bool, cap: usize| {
        let mut ids = (0..candidates.len())
            .filter(|&i| eligible(i) && predicate(i))
            .collect::<Vec<_>>();
        rank(&mut ids);
        ids.truncate(cap);
        ids
    };
    let mut selected = select(&scored, GENERAL);
    selected.extend(select(&nested, NESTED));
    for path in 0..=paths.iter().copied().max().unwrap() {
        selected.extend(select(
            &|i| nested(i) && paths[candidates[i].left] == path,
            NESTED_PER_FILE,
        ));
        for same in [false, true] {
            selected.extend(select(
                &|i| {
                    let (a, b) = (paths[candidates[i].left], paths[candidates[i].right]);
                    scored(i) && (a == b) == same && (a == path || b == path)
                },
                SCORED_PER_FILE,
            ));
        }
    }
    selected.sort_unstable();
    selected.dedup();
    selected
}

#[test]
fn streaming_selection_matches_every_global_and_file_reservation_in_any_arrival_order() {
    let paths = (0..180)
        .map(|i| if i < 100 { 0 } else { i % 17 })
        .collect::<Vec<_>>();
    let weights = (0..180)
        .map(|i| if i % 7 == 0 { 17 } else { 30 })
        .collect::<Vec<_>>();
    let candidates = (0..180)
        .flat_map(|left| {
            (left + 1..180).map(move |right| ScoredCandidate {
                left,
                right,
                ordinary_score: ((left + right) % 3 != 0)
                    .then_some(((left * 3 + right) % 11) as f64 / 10.0),
            })
        })
        .collect::<Vec<_>>();
    let expected = reference(&candidates, &paths, &weights, 0.8);
    assert!(
        expected.len() > GENERAL + NESTED,
        "per-file reservations add candidates"
    );
    for order in [
        (0..candidates.len()).collect::<Vec<_>>(),
        (0..candidates.len()).rev().collect(),
        (0..13)
            .flat_map(|offset| (offset..candidates.len()).step_by(13))
            .collect(),
    ] {
        let mut selected = SeedSelection::new(&paths, &weights, 0.8);
        for i in order {
            selected.push(candidates[i], (i, 0), i);
        }
        assert_eq!(selected.finish(), expected);
        let mut bounded = SeedSelection::new(&paths, &weights, 0.8);
        for (i, &candidate) in candidates.iter().enumerate() {
            if candidate.ordinary_score.is_none()
                || bounded.may_select(
                    candidate.ordinary_score.unwrap(),
                    (i, 0),
                    &[paths[candidate.left]],
                    &[paths[candidate.right]],
                )
            {
                bounded.push(candidate, (i, 0), i);
            }
        }
        assert_eq!(bounded.finish(), expected);
    }
}
