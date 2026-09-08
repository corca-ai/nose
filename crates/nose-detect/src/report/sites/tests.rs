use super::*;
use crate::{LineSpan, LocInit};

fn site(file: &str, start: u32, end: u32, name: &str) -> Loc {
    Loc::new(LocInit {
        file: file.into(),
        source_span: LineSpan::new(start, end),
        lang: "rust".into(),
        kind: nose_il::UnitKind::Function,
        origin: Default::default(),
        name: Some(name.into()),
        sem: 4,
        span_tokens: 8,
    })
}

fn group(members: Vec<Loc>) -> Group {
    Group {
        members,
        score: 1.0,
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    }
}

#[test]
fn canonical_sites_preserve_first_twins_and_file_order_at_equal_lengths() {
    let group = group(vec![
        site("b.rs", 1, 20, "b"),
        site("a.rs", 4, 20, "small"),
        site("a.rs", 1, 20, "first"),
        site("a.rs", 1, 20, "twin"),
        site("a.rs", 50, 59, "far"),
        site("c.rs", 1, 5, "c"),
        site("b.rs", 5, 12, "alias"),
    ]);
    let sites = collapsed_sites(&group);
    assert_eq!(
        sites
            .iter()
            .map(|site| site.name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["first", "b", "far", "c"]
    );
    assert_eq!(
        member_sites(&group, &sites),
        [
            Some(1),
            Some(0),
            Some(0),
            Some(0),
            Some(2),
            Some(3),
            Some(1)
        ]
    );
}

#[test]
fn member_mapping_keeps_last_overlap_ties_and_exact_file_boundaries() {
    let group = group(vec![
        site("x.rs", 4, 6, "equal overlap"),
        site("x.rs", 2, 3, "first only"),
        site("y.rs", 4, 6, "other file"),
        site("x.rs", 7, 6, "reversed"),
    ]);
    let sites = [
        site("x.rs", 1, 5, "left"),
        site("x.rs", 5, 9, "right"),
        site("y.rs", 1, 10, "other"),
    ];
    let expected = [
        [None, None, None, None],
        [Some(0), Some(0), None, None],
        [Some(1), Some(0), None, None],
        [Some(1), Some(0), Some(2), None],
    ];
    for (count, expected) in expected.into_iter().enumerate() {
        assert_eq!(
            member_sites(&group, &sites[..count]),
            expected,
            "{count} sites"
        );
    }
}
