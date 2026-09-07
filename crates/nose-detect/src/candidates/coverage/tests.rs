use super::*;
use nose_il::{FileId, Interner, Lang};

#[test]
fn exact_group_proof_reuse_preserves_mixed_group_witnesses() {
    let interner = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "f.py",
        b"def f(x):\n    return x * x + 7\n",
        Lang::Python,
        &interner,
    )
    .unwrap();
    let opts = crate::DetectOptions {
        min_tokens: 1,
        min_lines: 1,
        ..Default::default()
    };
    let units = (0..9)
        .map(|i| {
            let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
            unit.path = format!("{i}.py");
            unit.exact_safe = i < 4 || i % 2 == 0;
            if i % 2 == 1 {
                unit.anchors.clear();
            }
            unit
        })
        .collect::<Vec<_>>();
    let raw = vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]];
    let pairs = raw
        .iter()
        .flat_map(|members| {
            members.iter().enumerate().flat_map(|(index, &left)| {
                members[index + 1..]
                    .iter()
                    .map(move |&right| (left, right, 0.75 + right as f64 / 100.0))
            })
        })
        .collect::<Vec<_>>();
    let accepted = AcceptedPairs::from(pairs.clone());
    let (groups, _) = crate::candidates::build_groups(
        &units,
        &accepted,
        &raw,
        &vec![None; units.len()],
        &opts,
        false,
    );
    assert_eq!(
        groups[0].witness.as_ref().unwrap().kind(),
        "exact-value-graph"
    );
    assert_ne!(
        groups[1].witness.as_ref().unwrap().kind(),
        "exact-value-graph"
    );
    let projection = Projection::new(&units, &raw, &groups, &accepted, &vec![None; raw.len()]);
    assert!(projection.keys[..4]
        .iter()
        .all(|key| key.unwrap().2 == projection.keys[0].unwrap().2));
    assert_ne!(projection.exact[0], projection.exact[4]);
    assert_eq!(projection.keys[8], None);
    assert_eq!(projection.exact[8], None);
    let actual = projection.materialize(|_| true);
    let expanded = expanded_edges(&units, &raw, &groups, &accepted, &vec![None; raw.len()]);
    for (group, members) in raw.iter().enumerate() {
        let expected = pairs
            .iter()
            .filter_map(|&(left, right, score)| {
                let a = members.iter().position(|&index| index == left)?;
                let b = members.iter().position(|&index| index == right)?;
                Some(AcceptedEdge {
                    left: a as u32,
                    right: b as u32,
                    score: round3(score),
                    witness_kind: witness_kind(&[left, right], &units),
                })
            })
            .collect::<Vec<_>>();
        let GroupEdges::Members(actual_expanded) = &expanded[group] else {
            unreachable!()
        };
        assert_eq!(actual_expanded, &expected);
        let expected = crate::report::collapsed_accepted_edges(
            &groups[group],
            &sites::collapsed_sites(&groups[group]),
            &expected,
        );
        assert_eq!(
            actual[group].as_ref().unwrap().iter().collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn unmapped_bridge_does_not_claim_direct_site_evidence() {
    let mut projection = Projection {
        accepted: vec![(0, 1, 0.8), (1, 2, 0.8)].into(),
        keys: vec![Some((0, 0, 0)), None, Some((0, 1, 0))],
        exact: vec![None; 3],
        anchors: vec![Vec::new()],
        floor: 1,
        sizes: vec![2],
    };
    assert!(!projection.has_edges(0, &[0, 1, 2]));
    let edges = projection.materialize(|_| true)[0].take().unwrap();
    assert!(crate::AcceptedEdges::from_packed(edges).is_empty());
    projection.keys[1] = Some((0, 0, 0));
    assert!(projection.has_edges(0, &[0, 1, 2]));
    let edges = projection.materialize(|_| true)[0].take().unwrap();
    assert_eq!(crate::AcceptedEdges::from_packed(edges).len(), 1);
}

#[test]
fn deferred_large_site_graph_matches_expanded_reference_after_sources_are_dropped() {
    let interner = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "f.py",
        b"def f(x):\n    return x * x + 7\n",
        Lang::Python,
        &interner,
    )
    .unwrap();
    let opts = crate::DetectOptions {
        min_tokens: 1,
        min_lines: 1,
        ..Default::default()
    };
    let units = (0..1500)
        .map(|i| {
            let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
            unit.path = format!("{i}.py");
            unit.exact_safe = i % 3 == 0;
            unit
        })
        .collect::<Vec<_>>();
    let pairs = (0..units.len() - 1)
        .map(|i| (i, i + 1, 0.75))
        .collect::<Vec<_>>();
    let accepted = AcceptedPairs::from(pairs.clone());
    let raw = vec![(0..units.len()).collect::<Vec<_>>()];
    let (groups, _) = crate::candidates::build_groups(
        &units,
        &accepted,
        &raw,
        &vec![None; units.len()],
        &opts,
        false,
    );
    let projected = projected_edges(&units, &raw, &groups, &accepted, &vec![None; raw.len()]);
    let expanded = pairs
        .iter()
        .map(|&(left, right, score)| AcceptedEdge {
            left: left as u32,
            right: right as u32,
            score,
            witness_kind: witness_kind(&[left, right], &units),
        })
        .collect::<Vec<_>>();
    let expected = crate::report::collapsed_accepted_edges(
        &groups[0],
        &sites::collapsed_sites(&groups[0]),
        &expanded,
    );
    drop(units);
    drop(accepted);
    drop(groups);
    let GroupEdges::Sites(edges) = &projected[0] else {
        unreachable!()
    };
    assert!(!edges.is_empty());
    assert_eq!(edges.len(), expected.len());
    assert_eq!(edges.iter().collect::<Vec<_>>(), expected);
}

#[test]
fn projecting_before_materialization_keeps_the_same_direct_site_evidence() {
    let interner = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "f.py",
        b"def f(x):\n    a = x * x\n    b = a + 1\n    return b * 2\n",
        Lang::Python,
        &interner,
    )
    .unwrap();
    let opts = crate::DetectOptions {
        min_tokens: 1,
        min_lines: 1,
        ..Default::default()
    };
    let mut units = (0..48)
        .map(|i| {
            let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
            unit.path = format!("{}.py", i % 4);
            unit.start_line = (i / 8 * 2) as u32;
            unit.end_line = unit.start_line + if i % 3 == 0 { 9 } else { 2 };
            unit.exact_safe = i % 3 == 0;
            if i % 5 == 0 {
                unit.anchors.clear();
            }
            unit
        })
        .collect::<Vec<_>>();
    let pairs = (0..units.len())
        .flat_map(|left| {
            let units = &units;
            (left + 1..units.len()).filter_map(move |right| {
                (!crate::locations::is_nested(&units[left], &units[right])).then_some((
                    left,
                    right,
                    0.7 + ((left + right) % 4) as f64 / 20.0,
                ))
            })
        })
        .collect::<Vec<_>>();
    let raw = vec![(0..units.len()).collect::<Vec<_>>()];
    let mut unrelated = crate::units_of_file(&il, &interner, &opts).remove(0);
    unrelated.path = "outside-group.py".into();
    unrelated.anchors.clear();
    unrelated.exact_safe = false;
    units.push(unrelated);
    let accepted = AcceptedPairs::from(pairs.clone());
    let (groups, _) = crate::candidates::build_groups(
        &units,
        &accepted,
        &raw,
        &vec![None; units.len()],
        &opts,
        false,
    );
    let projected = projected_edges(&units, &raw, &groups, &accepted, &vec![None; raw.len()]);
    let expanded = pairs
        .iter()
        .map(|&(left, right, score)| AcceptedEdge {
            left: left as u32,
            right: right as u32,
            score: round3(score),
            witness_kind: witness_kind(&[left, right], &units),
        })
        .collect::<Vec<_>>();
    let GroupEdges::Members(actual) =
        &expanded_edges(&units, &raw, &groups, &accepted, &vec![None; raw.len()])[0]
    else {
        unreachable!()
    };
    assert_eq!(actual, &expanded);
    let sites = sites::collapsed_sites(&groups[0]);
    let collapse =
        |edges: &[AcceptedEdge]| crate::report::collapsed_accepted_edges(&groups[0], &sites, edges);
    let GroupEdges::Sites(projected) = &projected[0] else {
        unreachable!()
    };
    let projected = projected.iter().collect::<Vec<_>>();
    assert_eq!(projected, collapse(&expanded));
    assert!(projected.len() < expanded.len());
}
