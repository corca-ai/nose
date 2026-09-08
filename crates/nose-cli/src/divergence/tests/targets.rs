use super::super::targets::direct_targets;
use super::divergence_family;
use nose_detect::{AcceptedEdge, LineSpan, Loc, LocInit, RefactorFamily};
use std::collections::HashMap;
use std::path::Path;

fn direct_edge(left: u32, right: u32, score: f64, witness_kind: &'static str) -> AcceptedEdge {
    AcceptedEdge {
        left,
        right,
        score,
        witness_kind,
    }
}

fn family_with_direct_edges(locs: Vec<Loc>, edges: Vec<AcceptedEdge>) -> RefactorFamily {
    let mut family = divergence_family(locs);
    family.direct_edges = edges.into();
    family
}

#[test]
fn direct_targets_exclude_transitive_bridge_members() {
    let a = Loc::new(LocInit {
        file: "a.py".into(),
        source_span: LineSpan::new(1, 8),
        lang: "python".into(),
        kind: nose_il::UnitKind::Function,
        origin: Default::default(),
        name: Some("a".into()),
        sem: 8,
        span_tokens: 24,
    });
    let mut b = a.clone();
    b.file = "b.py".into();
    b.name = Some("b".into());
    let mut c = a.clone();
    c.file = "c.py".into();
    c.name = Some("c".into());
    let family = family_with_direct_edges(
        vec![a, b, c],
        vec![
            direct_edge(0, 1, 1.0, "exact-value-graph"),
            direct_edge(1, 2, 0.8, "structural-similarity"),
        ],
    );
    let changed = HashMap::from([("a.py".to_string(), vec![(2, 2)])]);
    let targets = direct_targets(
        &family,
        Path::new("/different/temp/worktree"),
        &mut crate::source_lines::FileLineCache::default(),
        &changed,
    );
    let [target] = targets.as_slice() else {
        panic!("only the accepted a -> b edge should be a target")
    };
    assert_eq!(target.changed.file, "a.py");
    assert_eq!(target.skipped.file, "b.py");
    assert_eq!(target.direct_witness.kind, "exact-value-graph");
    assert_eq!(target.changed.touches_shared, Some(true));
    assert!(targets.iter().all(|target| target.skipped.file != "c.py"));
}

#[test]
fn direct_targets_keep_pair_strength_and_stable_base_identity() {
    let make_loc = |file: &str, lang: &str, name: &str| {
        Loc::new(LocInit {
            file: file.into(),
            source_span: LineSpan::new(10, 20),
            lang: lang.into(),
            kind: nose_il::UnitKind::Function,
            origin: Default::default(),
            name: Some(name.into()),
            sem: 12,
            span_tokens: 36,
        })
    };
    let family = family_with_direct_edges(
        vec![
            make_loc("old/a.rs", "rust", "a"),
            make_loc("old/b.py", "python", "b"),
            make_loc("old/c.rs", "rust", "c"),
        ],
        vec![
            direct_edge(0, 1, 0.97, "exact-value-graph"),
            direct_edge(0, 2, 0.81, "structural-similarity"),
        ],
    );
    let changed = HashMap::from([("old/a.rs".to_string(), vec![(12, 12)])]);
    let build = |root: &Path| {
        direct_targets(
            &family,
            root,
            &mut crate::source_lines::FileLineCache::default(),
            &changed,
        )
    };
    let first = build(Path::new("/tmp/base-one"));
    let second = build(Path::new("/tmp/base-two"));
    assert_eq!(
        first.len(),
        2,
        "both direct edges remain independent targets"
    );
    assert_eq!(
        first
            .iter()
            .map(|target| (
                &target.target_id,
                target.direct_witness.kind,
                target.direct_witness.similarity
            ))
            .collect::<Vec<_>>(),
        second
            .iter()
            .map(|target| (
                &target.target_id,
                target.direct_witness.kind,
                target.direct_witness.similarity
            ))
            .collect::<Vec<_>>(),
        "target ids and pair witnesses ignore temporary worktree roots"
    );
    assert!(first.iter().any(|target| {
        target.skipped.lang == "python"
            && target.direct_witness.kind == "exact-value-graph"
            && target.changed.touches_shared == Some(true)
    }));
    assert!(first.iter().any(|target| {
        target.skipped.file == "old/c.rs"
            && target.direct_witness.kind == "structural-similarity"
            && target.changed.touches_shared.is_none()
    }));
}
