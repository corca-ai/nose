use super::*;
use crate::orchestration::accepted::AcceptedPairs;
use std::collections::BTreeSet;

type Edge = (u32, u32, u64);

fn projected(
    pairs: &AcceptedPairs,
    keys: &[Option<SiteKey>],
    exact: &[Option<usize>],
) -> BTreeSet<Edge> {
    let mut found = BTreeSet::new();
    pairs.visit_projected_evidence(keys, exact, |evidence| match evidence {
        SiteEvidence::Complete { sites, score, .. } => {
            for left in 0..sites {
                for right in left + 1..sites {
                    found.insert((left, right, score.to_bits()));
                }
            }
        }
        SiteEvidence::Pair((left, right, score)) => {
            if let (Some((group, a, _)), Some((other, b, _))) = (keys[left], keys[right]) {
                if group == other && a != b {
                    found.insert((a.min(b), a.max(b), score.to_bits()));
                }
            }
        }
        SiteEvidence::ExactMask {
            left,
            block,
            mask,
            score,
        } => {
            let site = keys[left].unwrap().1;
            for bit in 0..64 {
                let other = block * 64 + bit;
                if mask & (1 << bit) != 0 && site != other {
                    found.insert((site.min(other), site.max(other), score.to_bits()));
                }
            }
        }
    });
    found
}

#[test]
fn complete_rows_preserve_every_site_edge_with_aliases_and_nesting() {
    let mut units = crate::test_support::scoring_units(600);
    let paths = (0..units.len())
        .map(|index| (index % 130) % 7)
        .collect::<Vec<_>>();
    for (index, unit) in units.iter_mut().enumerate() {
        unit.start_line = ((index / 7) % 19) as u32;
        unit.end_line = unit.start_line + (index % 11) as u32;
    }
    let keys = (0..units.len())
        .map(|index| (index % 11 != 0).then_some((0, (index % 130) as u32, index % 3)))
        .collect::<Vec<_>>();
    for score in [-0.0, 0.0, 0.875, f64::INFINITY] {
        let pairs = AcceptedPairs::rows(
            &units,
            &paths,
            &[(0..units.len()).collect()],
            vec![(0, 0, score)],
        );
        let AcceptedPairs::Rows(rows) = &pairs else {
            unreachable!()
        };
        let exact = vec![Some(7); units.len()];
        assert_eq!(prepare(rows, &keys, &exact)[0].is_some(), score.is_finite());
        let expected = AcceptedPairs::Explicit(pairs.iter().collect());
        assert_eq!(
            projected(&pairs, &keys, &exact),
            projected(&expected, &keys, &exact)
        );
        let mut ambiguous = keys.clone();
        ambiguous[2] = ambiguous[1];
        assert!(prepare(rows, &ambiguous, &exact)[0].is_none());
        assert_eq!(
            projected(&pairs, &ambiguous, &exact),
            projected(&expected, &ambiguous, &exact)
        );
        let mut mixed = exact.clone();
        mixed[1] = Some(8);
        assert!(prepare(rows, &keys, &mixed)[0].is_none());
        assert_eq!(
            projected(&pairs, &keys, &mixed),
            projected(&expected, &keys, &mixed)
        );
    }
}

#[test]
fn cross_row_connectivity_does_not_prove_a_complete_row() {
    let units = crate::test_support::scoring_units(32);
    let paths = (0..units.len()).collect::<Vec<_>>();
    let members = [(0..32).step_by(2).collect(), (1..32).step_by(2).collect()];
    let pairs = AcceptedPairs::rows(&units, &paths, &members, vec![(0, 1, 0.75), (1, 0, 0.75)]);
    let keys = (0..32).map(|index| Some((0, index, 0))).collect::<Vec<_>>();
    let exact = vec![Some(0); 32];
    let AcceptedPairs::Rows(rows) = &pairs else {
        unreachable!()
    };
    assert!(prepare(rows, &keys, &exact).iter().all(Option::is_none));
    let expected = AcceptedPairs::Explicit(pairs.iter().collect());
    assert_eq!(
        projected(&pairs, &keys, &exact),
        projected(&expected, &keys, &exact)
    );
}

fn first_winners(
    pairs: &AcceptedPairs,
    keys: &[Option<SiteKey>],
) -> std::collections::BTreeMap<(u32, u32), u64> {
    let mut found = std::collections::BTreeMap::new();
    let mut add = |a: u32, b: u32, score: f64| {
        if a != b {
            found.entry((a.min(b), a.max(b))).or_insert(score.to_bits());
        }
    };
    pairs.visit_projected_evidence(
        keys,
        &vec![Some(0); keys.len()],
        |evidence| match evidence {
            SiteEvidence::Complete { sites, score, .. } => {
                for left in 0..sites {
                    for right in left + 1..sites {
                        add(left, right, score);
                    }
                }
            }
            SiteEvidence::Pair((left, right, score)) => {
                add(keys[left].unwrap().1, keys[right].unwrap().1, score)
            }
            SiteEvidence::ExactMask {
                left,
                block,
                mask,
                score,
            } => {
                for bit in 0..64 {
                    if mask & (1 << bit) != 0 {
                        add(keys[left].unwrap().1, block * 64 + bit, score);
                    }
                }
            }
        },
    );
    found
}

#[test]
fn intersecting_rows_preserve_first_winning_float_bits() {
    let units = crate::test_support::scoring_units(128);
    let paths = (0..units.len())
        .map(|index| index / 2 % 32)
        .collect::<Vec<_>>();
    let members = [(1..128).step_by(2).collect(), (0..128).step_by(2).collect()];
    let keys = paths
        .iter()
        .map(|&site| Some((0, site as u32, 0)))
        .collect::<Vec<_>>();
    // Numerical ties and unordered comparisons retain the first exact witness.
    // Canonical aliases span rows whose source order differs from row-id order.
    for scores in [[0.0, -0.0], [-0.0, 0.0], [1.0, f64::NAN], [f64::NAN, 1.0]] {
        let pairs = AcceptedPairs::rows(
            &units,
            &paths,
            &members,
            vec![(0, 0, scores[0]), (1, 1, scores[1])],
        );
        let expected = AcceptedPairs::Explicit(pairs.iter().collect());
        let winners = first_winners(&expected, &keys);
        assert_eq!(winners.len(), 32 * 31 / 2);
        assert_eq!(first_winners(&pairs, &keys), winners);
    }
}

#[test]
fn complete_site_proof_requires_dense_coordinates_and_non_nested_representatives() {
    let mut units = crate::test_support::scoring_units(128);
    let paths = (0..units.len()).map(|index| index % 4).collect::<Vec<_>>();
    let keys = (0..units.len())
        .map(|index| Some((0, (index % 16) as u32, 0)))
        .collect::<Vec<_>>();
    let exact = vec![Some(0); units.len()];
    for variation in 0..5 {
        for (index, unit) in units.iter_mut().enumerate() {
            unit.start_line = (index % 16) as u32 * 10;
            unit.end_line = unit.start_line + if index >= 16 { 4 } else { 1 };
            if variation == 1 {
                unit.start_line = 0;
            }
            if variation == 2 {
                unit.end_line = 200;
            }
            if variation == 4 {
                unit.end_line = unit.start_line + 150;
            }
        }
        let keys = keys
            .iter()
            .map(|key| {
                key.map(|(group, site, class)| {
                    (group, if variation == 3 { site * 2 } else { site }, class)
                })
            })
            .collect::<Vec<_>>();
        for score in [-0.0, 0.0, 0.875] {
            let pairs = AcceptedPairs::rows(
                &units,
                &paths,
                &[(0..units.len()).collect()],
                vec![(0, 0, score)],
            );
            let AcceptedPairs::Rows(rows) = &pairs else {
                unreachable!()
            };
            let cliques = prepare(rows, &keys, &exact);
            assert_eq!(
                cliques[0].as_ref().unwrap().is_complete(),
                variation == 0 || variation == 4
            );
            let expected = AcceptedPairs::Explicit(pairs.iter().collect());
            assert_eq!(
                projected(&pairs, &keys, &exact),
                projected(&expected, &keys, &exact)
            );
            assert_eq!(
                first_winners(&pairs, &keys),
                first_winners(&expected, &keys)
            );
        }
    }
}
