use super::*;
use crate::orchestration::accepted::AcceptedPairs;

fn fixture(score: f64) -> (Vec<Vec<usize>>, Vec<AcceptedPair>, Vec<Group>) {
    let mut units = crate::test_support::scoring_units(24);
    for (i, unit) in units.iter_mut().enumerate() {
        unit.path = format!("{}.py", i % 3);
        unit.start_line = i as u32 * 2;
        unit.end_line = unit.start_line + 7;
        unit.exact_safe = true;
    }
    units[1].path = units[0].path.clone();
    units[1].start_line = units[0].start_line;
    units[1].end_line = units[0].end_line;
    units[2].start_line = 0;
    units[2].end_line = 100;
    units[3].start_line = 9;
    units[3].end_line = 2;
    let raw = vec![(0..units.len()).collect::<Vec<_>>()];
    let mut pairs = Vec::new();
    for left in 0..units.len() {
        for right in left + 1..units.len() {
            if !crate::locations::is_nested(&units[left], &units[right]) {
                pairs.push((left, right, score));
            }
        }
    }
    let accepted = AcceptedPairs::Explicit(pairs.clone());
    let (groups, _) = crate::candidates::build_groups(
        &units,
        &accepted,
        &raw,
        &vec![None; units.len()],
        &crate::DetectOptions::default(),
        false,
    );
    (raw, pairs, groups)
}

#[test]
fn complete_explicit_relations_preserve_canonical_edges_and_score_bits() {
    for score in [-0.0_f64, 0.0, 0.875, -0.875] {
        let (raw, pairs, groups) = fixture(score);
        let actual = AcceptedPairs::Explicit(pairs.clone()).uniform_groups(&raw, &groups);
        assert_eq!(actual[0].unwrap().to_bits(), round3(score).to_bits());
        let sites = crate::report::sites::collapsed_sites(&groups[0]);
        let original = pairs
            .iter()
            .map(|&(left, right, score)| crate::AcceptedEdge {
                left: left as u32,
                right: right as u32,
                score: round3(score),
                witness_kind: "exact-value-graph",
            })
            .collect::<Vec<_>>();
        let expected = crate::report::collapsed_accepted_edges(&groups[0], &sites, &original);
        let compact =
            crate::report::edges::uniform_source_edges(&groups[0], &sites, actual[0].unwrap());
        let key = |edge: crate::AcceptedEdge| {
            (
                edge.left,
                edge.right,
                edge.score.to_bits(),
                edge.witness_kind,
            )
        };
        assert_eq!(
            compact.iter().map(key).collect::<Vec<_>>(),
            expected.into_iter().map(key).collect::<Vec<_>>()
        );
    }
}

#[test]
fn cardinality_alone_never_certifies_missing_duplicate_or_forbidden_edges() {
    let (raw, pairs, groups) = fixture(1.0);
    assert!(!pairs.iter().any(|pair| (pair.0, pair.1) == (0, 1)));
    let mut missing = pairs.clone();
    missing.pop();
    let mut duplicate = pairs.clone();
    duplicate[0] = duplicate[1];
    let mut nested = pairs.clone();
    nested[0] = (0, 1, 1.0);
    let mut same_unit = pairs.clone();
    same_unit[0] = (0, 0, 1.0);
    let mut reversed = pairs.clone();
    reversed[0] = (pairs[0].1, pairs[0].0, 1.0);
    let mut unknown = pairs.clone();
    unknown.push((0, 24, 1.0));
    for mut changed in [missing, duplicate, nested, same_unit, reversed, unknown] {
        changed.sort_by_key(|&(left, right, _)| (left, right));
        assert_eq!(
            AcceptedPairs::Explicit(changed).uniform_groups(&raw, &groups),
            vec![None]
        );
    }
    let mut unordered = pairs;
    unordered.reverse();
    assert_eq!(
        AcceptedPairs::Explicit(unordered).uniform_groups(&raw, &groups),
        vec![None]
    );
}

#[test]
fn explicit_uniform_proof_requires_uniform_finite_scores_and_exact_membership() {
    let (raw, pairs, mut groups) = fixture(0.0);
    for score in [-0.0, 0.0001, f64::MAX, f64::INFINITY, f64::NAN] {
        let mut changed = pairs.clone();
        changed[0].2 = score;
        assert_eq!(
            AcceptedPairs::Explicit(changed).uniform_groups(&raw, &groups),
            vec![None]
        );
    }
    let accepted = AcceptedPairs::Explicit(pairs);
    assert_eq!(accepted.uniform_groups(&[vec![0; 24]], &groups), vec![None]);
    assert_eq!(accepted.uniform_groups(&[vec![0, 1]], &groups), vec![None]);
    groups[0].witness = None;
    assert_eq!(accepted.uniform_groups(&raw, &groups), vec![None]);
}
