use super::*;
use std::collections::BTreeSet;

#[test]
fn masks_preserve_cross_file_sites_and_partial_prefixes() {
    let mut keys = (0..257)
        .map(|i| Some((0, (i % 95 + 1) as u32, 0)))
        .collect::<Vec<_>>();
    keys[0] = Some((0, 20, 0));
    let exact = vec![Some(0); keys.len()];
    let locations = (0..keys.len()).map(|i| (i % 3, 1, 2)).collect::<Vec<_>>();
    let targets = (1..keys.len()).map(|i| (i, 1.0)).collect::<Vec<_>>();
    let mut runs = TargetRuns::new(targets.clone(), &keys, &exact, &locations);
    for left in [0, 1, 17, 63, 127, 128, 191, 255, 256] {
        let mut actual = BTreeSet::new();
        let mut masks = 0;
        let site = keys[left].unwrap().1;
        runs.visit(
            left,
            keys[left].unwrap(),
            exact[left],
            &locations,
            &mut |event| match event {
                SiteEvidence::Complete { .. } => {
                    panic!("a target run does not certify a whole group")
                }
                SiteEvidence::Pair((_, right, score)) => {
                    let other = keys[right].unwrap().1;
                    if site != other {
                        actual.insert((site.min(other), site.max(other), score.to_bits()));
                    }
                }
                SiteEvidence::ExactMask {
                    left: source,
                    block,
                    mask,
                    score,
                } => {
                    assert_eq!(source, left);
                    masks += 1;
                    for bit in 0..64 {
                        if mask & (1 << bit) != 0 {
                            let other = block * 64 + bit;
                            if site != other {
                                actual.insert((site.min(other), site.max(other), score.to_bits()));
                            }
                        }
                    }
                }
            },
        );
        let expected = targets
            .iter()
            .filter_map(|&(right, score)| {
                let other = keys[right].unwrap().1;
                (right > left && locations[right].0 != locations[left].0 && site != other)
                    .then_some((site.min(other), site.max(other), score.to_bits()))
            })
            .collect();
        assert_eq!(actual, expected);
        if left == 0 {
            assert!(masks > 0);
        }
    }
}

#[test]
fn consecutive_score_bits_keep_their_event_order() {
    let mut keys = vec![Some((0, 1, 0)); 33];
    keys[0] = Some((0, 0, 0));
    let exact = vec![Some(0); keys.len()];
    let mut locations = vec![(1, 1, 2); keys.len()];
    locations[0].0 = 0;
    for scores in [[-0.0_f64, 0.0_f64], [0.0_f64, -0.0_f64]] {
        let targets = (1..33).map(|i| (i, scores[(i - 1) / 16])).collect();
        let mut runs = TargetRuns::new(targets, &keys, &exact, &locations);
        let mut observed = Vec::new();
        runs.visit(
            0,
            keys[0].unwrap(),
            exact[0],
            &locations,
            &mut |event| match event {
                SiteEvidence::ExactMask { mask, score, .. } => {
                    assert_eq!(mask, 2);
                    observed.push(score.to_bits());
                }
                SiteEvidence::Pair(_) | SiteEvidence::Complete { .. } => {
                    panic!("long complete exact runs should use masks")
                }
            },
        );
        assert_eq!(observed, scores.map(f64::to_bits));
    }
}

#[test]
fn short_and_nonfinite_runs_keep_scalar_events() {
    let mut keys = vec![Some((0, 1, 0)); 16];
    keys[0] = Some((0, 0, 0));
    let exact = vec![Some(0); keys.len()];
    let mut locations = vec![(1, 1, 2); keys.len()];
    locations[0].0 = 0;
    for score in [f64::INFINITY, f64::MAX, f64::NAN] {
        let targets = (1..16)
            .map(|i| (i, if i < 8 { 1.0 } else { score }))
            .collect::<Vec<_>>();
        let mut runs = TargetRuns::new(targets.clone(), &keys, &exact, &locations);
        let mut observed = Vec::new();
        runs.visit(
            0,
            keys[0].unwrap(),
            exact[0],
            &locations,
            &mut |event| match event {
                SiteEvidence::Pair((_, right, score)) => observed.push((right, score.to_bits())),
                SiteEvidence::ExactMask { .. } | SiteEvidence::Complete { .. } => {
                    panic!("short or nonfinite run was packed")
                }
            },
        );
        let expected = targets
            .iter()
            .map(|&(right, score)| (right, score.to_bits()))
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
    }
}

#[test]
fn two_file_extrema_answer_every_suffix_exclusion() {
    let paths = [1, 1, 2, 1, 3, 3, 1, 2, 2, 3];
    let mut suffix = SiteSuffix {
        site: 0,
        latest: (0, paths[0]),
        other_file: None,
    };
    for (right, &path) in paths.iter().enumerate() {
        suffix.advance(right, path);
        for excluded in 0..4 {
            for left in 0..=paths.len() {
                let expected = paths[..=right]
                    .iter()
                    .enumerate()
                    .any(|(target, &file)| target > left && file != excluded);
                assert_eq!(suffix.admits(left, excluded), expected);
            }
        }
    }
}
