use super::*;

fn scored(score: Option<f64>) -> ScoredCandidate {
    ScoredCandidate {
        left: 0,
        right: 1,
        ordinary_score: score,
    }
}

fn nested(left: usize, right: usize) -> ScoredCandidate {
    ScoredCandidate {
        left,
        right,
        ordinary_score: None,
    }
}

#[test]
fn raw_connected_audit_keeps_every_seed() {
    let candidates = [scored(Some(0.1)), scored(None), scored(Some(0.9))];
    assert_eq!(
        connected_seed_indices(&candidates, &["a", "b"], &[20, 20], 0.7, false),
        vec![0, 1, 2]
    );
}

#[test]
fn product_connected_work_keeps_nested_and_strongest_scored_seeds() {
    let mut candidates = vec![scored(Some(0.1)); 2_050];
    candidates[0] = scored(None);
    candidates[1] = scored(Some(0.0));
    candidates[2_049] = scored(Some(0.99));
    let selected = connected_seed_indices(&candidates, &["x/a", "y/b"], &[20, 20], 1.0, true);
    assert!(
        selected.contains(&0),
        "nested routes are never budgeted away"
    );
    assert!(
        selected.contains(&2_049),
        "the strongest scored seed is retained"
    );
    assert!(
        !selected.contains(&1),
        "the weakest overflow seed is dropped"
    );
    assert_eq!(selected.len(), 2_049);
}

#[test]
fn product_connected_work_reserves_nested_seeds_per_file() {
    let mut candidates = vec![nested(0, 1); 513];
    candidates.push(nested(2, 3));
    let paths = ["dense/a.rs", "dense/a.rs", "small/b.rs", "small/b.rs"];
    let selected = connected_seed_indices(&candidates, &paths, &[20; 4], 0.7, true);
    assert!(
        selected.contains(&513),
        "a later file keeps its own nested seed after the global cap"
    );
}

fn accepted(left: usize, right: usize, route: ConnectedRoute) -> ConnectedAccepted {
    ConnectedAccepted {
        left,
        right,
        score: 0.8,
        witness: crate::ConnectedWitness {
            left_lines: (10, 20),
            right_lines: (30, 40),
            mapped_nodes: 30,
            holes: 1,
            complete_exit: false,
        },
        route,
    }
}

#[test]
fn same_unit_output_never_displaces_cross_unit_routes() {
    let mut cross = (0..40)
        .map(|index| accepted(index, index + 100, ConnectedRoute::Mapped))
        .collect::<Vec<_>>();
    deduplicate_connected(&AcceptedPairs::default(), &mut cross, true);
    let expected = cross
        .iter()
        .map(|pair| (pair.left, pair.right))
        .collect::<Vec<_>>();

    let mut same_unit = (0..40)
        .map(|index| accepted(index, index, ConnectedRoute::SameUnit))
        .collect::<Vec<_>>();
    deduplicate_same_unit(&[], &mut same_unit, true);
    let mut combined = (0..40)
        .map(|index| accepted(index, index + 100, ConnectedRoute::Mapped))
        .collect::<Vec<_>>();
    deduplicate_connected(&AcceptedPairs::default(), &mut combined, true);
    combined.extend(same_unit);
    let observed = combined
        .iter()
        .filter(|pair| matches!(pair.route, ConnectedRoute::Mapped))
        .map(|pair| (pair.left, pair.right))
        .collect::<Vec<_>>();

    assert_eq!(observed, expected);
}
