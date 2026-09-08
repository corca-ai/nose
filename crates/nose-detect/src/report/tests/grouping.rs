use crate::Group;

use super::super::{
    rank_families,
    ranking::{family_min_loc, subsumes},
};
use super::support::{accepted_edge, fam, loc, report};

#[test]
fn subsumes_collapses_window_shift() {
    // A block family whose sites are a few-line-shifted window of a larger family's
    // sites (the contiguous channel finding the same run at different starts) is
    // subsumed — the field-eval double-counting case.
    let outer = fam(
        10.0,
        7,
        7,
        0,
        vec![loc("a.rs", 11, 17, "rs"), loc("b.rs", 10, 14, "rs")],
    );
    let inner = fam(
        10.0,
        10,
        10,
        0,
        vec![loc("a.rs", 8, 17, "rs"), loc("b.rs", 7, 14, "rs")],
    );
    assert!(subsumes(&outer, &inner), "≥60%-overlapping sites collapse");
    let distinct = fam(
        10.0,
        11,
        11,
        0,
        vec![loc("a.rs", 40, 50, "rs"), loc("b.rs", 40, 50, "rs")],
    );
    assert!(
        !subsumes(&outer, &distinct),
        "non-overlapping family is kept"
    );
}

#[test]
fn subsumes_when_one_outer_site_covers_several_inner_sites() {
    // A larger family with FEWER but bigger sites can still cover an inner family's
    // MORE numerous smaller sites — every inner site lands inside an outer one, so it
    // is double-counting and must be subsumed. (A site-count early-out used to reject
    // this, leaving both families in the report.)
    let outer = fam(
        10.0,
        100,
        100,
        0,
        vec![loc("a.rs", 1, 100, "rs"), loc("b.rs", 1, 100, "rs")],
    );
    let inner = fam(
        10.0,
        30,
        30,
        0,
        vec![
            loc("a.rs", 10, 40, "rs"),
            loc("a.rs", 60, 90, "rs"),
            loc("b.rs", 20, 50, "rs"),
        ],
    );
    assert!(
        subsumes(&outer, &inner),
        "one big outer site may cover several inner sites"
    );
}

#[test]
fn dedups_colocated_units() {
    // a function and an inner block with the same span = one site
    let g = Group {
        score: 1.0,
        members: vec![
            loc("a.rs", 1, 20, "rust"),
            loc("a.rs", 1, 20, "rust"),
            loc("b.rs", 1, 20, "rust"),
        ],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let f = &rank_families(&report(vec![g]))[0];
    assert_eq!(
        f.members, 2,
        "co-located identical spans collapse to one site"
    );
    assert_eq!(f.files, 2);
}

#[test]
fn accepted_edges_follow_collapsed_sites_across_files() {
    let g = Group {
        score: 1.0,
        members: vec![
            loc("a.rs", 1, 20, "rust"),
            loc("a.rs", 3, 18, "rust"),
            loc("b.rs", 1, 20, "rust"),
            loc("c.rs", 1, 20, "rust"),
        ],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let mut report = report(vec![g]);
    report.accepted_group_edges = vec![vec![
        accepted_edge(0, 2),
        accepted_edge(1, 2),
        accepted_edge(2, 3),
        accepted_edge(0, 1),
    ]
    .into()];

    let families = rank_families(&report);
    let [family] = families.as_slice() else {
        panic!("the collapsed group must remain one family")
    };
    assert!(
        family.accepted_coverage.is_empty(),
        "a retained family's own edges need no copied coverage obligation"
    );
    assert_eq!(
        family
            .direct_edges
            .iter()
            .map(|edge| (edge.left, edge.right))
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 2)]
    );
}

#[test]
fn subsumed_family_is_dropped() {
    // An outer family of two functions, and an inner family of blocks contained
    // within them (same regions, reported twice) — only the outer survives.
    let outer = Group {
        score: 0.9,
        members: vec![loc("a.rs", 10, 40, "rust"), loc("b.rs", 10, 40, "rust")],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: Some(crate::EquivalenceWitness {
            evidence: crate::WitnessEvidence::CopyPasteRun,
            graded: None,
            graded_pair: None,
        }),
    };
    let inner = Group {
        score: 1.0,
        members: vec![loc("a.rs", 15, 25, "rust"), loc("b.rs", 15, 25, "rust")],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let mut report = report(vec![inner, outer]);
    report.accepted_group_edges = vec![vec![accepted_edge(0, 1)].into(), Vec::new().into()];
    let fams = rank_families(&report);
    assert_eq!(fams.len(), 1, "the contained family should be dropped");
    assert_eq!(
        fams[0].mean_lines, 31,
        "the surviving family is the outer one"
    );
    let [obligation] = fams[0].accepted_coverage.as_slice() else {
        panic!("the syntax survivor must inherit exactly one structural obligation")
    };
    assert_eq!(obligation.sites.len(), 2);
    assert_eq!(obligation.edges.len(), 1);
    let edge = obligation.edges.iter().next().unwrap();
    assert_eq!((edge.left, edge.right), (0, 1));
    assert!(obligation
        .sites
        .iter()
        .any(|loc| loc.file == "a.rs" && loc.start_line == 15 && loc.end_line == 25));
    assert!(obligation
        .sites
        .iter()
        .any(|loc| loc.file == "b.rs" && loc.start_line == 15 && loc.end_line == 25));
}

#[test]
fn connected_family_does_not_subsume_existing_output() {
    for kind in ["connected-mapped-sub-dag", "bounded-same-unit-window"] {
        let connected = Group {
            score: 0.8,
            members: vec![loc("a.rs", 10, 40, "rust"), loc("b.rs", 10, 40, "rust")],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: Some(crate::EquivalenceWitness {
                evidence: if kind == "bounded-same-unit-window" {
                    crate::WitnessEvidence::BoundedSameUnitWindow { value_nodes: 60 }
                } else {
                    crate::WitnessEvidence::ConnectedMappedSubDag { value_nodes: 60 }
                },
                graded: None,
                graded_pair: None,
            }),
        };
        let existing = Group {
            score: 0.9,
            members: vec![loc("a.rs", 15, 25, "rust"), loc("b.rs", 15, 25, "rust")],
            semantic_laws: Vec::new(),
            abstraction_witness: None,
            witness: Some(crate::EquivalenceWitness {
                evidence: crate::WitnessEvidence::StructuralSimilarity {
                    mean_value_jaccard: 0.8,
                    mean_shape_jaccard: 0.9,
                },
                graded: None,
                graded_pair: None,
            }),
        };

        assert_eq!(
            rank_families(&report(vec![connected, existing])).len(),
            2,
            "witness kind {kind}"
        );
    }
}

#[test]
fn existing_family_only_hides_a_same_unit_route_at_identical_sites() {
    let same_unit = |start, end| Group {
        score: 0.8,
        members: vec![
            loc("a.rs", start, end, "rust"),
            loc("b.rs", start, end, "rust"),
        ],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: Some(crate::EquivalenceWitness {
            evidence: crate::WitnessEvidence::BoundedSameUnitWindow { value_nodes: 30 },
            graded: None,
            graded_pair: None,
        }),
    };
    let existing = |start, end| Group {
        score: 0.9,
        members: vec![
            loc("a.rs", start, end, "rust"),
            loc("b.rs", start, end, "rust"),
        ],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: Some(crate::EquivalenceWitness {
            evidence: crate::WitnessEvidence::StructuralSimilarity {
                mean_value_jaccard: 0.8,
                mean_shape_jaccard: 0.9,
            },
            graded: None,
            graded_pair: None,
        }),
    };

    assert_eq!(
        rank_families(&report(vec![existing(1, 40), same_unit(15, 25)])).len(),
        2,
        "an enclosing family must not replace the bounded locations"
    );
    assert_eq!(
        rank_families(&report(vec![existing(15, 25), same_unit(15, 25)])).len(),
        1,
        "identical sites stay one product family"
    );
}

#[test]
fn single_site_family_does_not_subsume_reportable_family() {
    // A contiguous-channel group can be one long same-file window after
    // `family_of` coalesces overlapping matches. It is not itself reportable, so
    // it must not hide the exact semantic method family it covers.
    let syntax_window = Group {
        score: 1.0,
        members: vec![loc("Param.java", 1589, 1611, "java")],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let semantic_methods = Group {
        score: 1.0,
        members: vec![
            loc("Param.java", 1589, 1593, "java"),
            loc("Param.java", 1595, 1599, "java"),
            loc("Param.java", 1601, 1605, "java"),
            loc("Param.java", 1607, 1611, "java"),
        ],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };

    let fams = rank_families(&report(vec![syntax_window, semantic_methods]));

    assert_eq!(fams.len(), 1);
    assert_eq!(
        fams[0].members, 4,
        "the reportable semantic family should survive"
    );
    assert!(fams[0]
        .locations
        .iter()
        .any(|loc| loc.start_line <= 1592 && loc.end_line >= 1592));
}

#[test]
fn dedup_order_is_independent_of_group_input_order() {
    // Two families that tie on span AND value but live in different files. The dedup sort's
    // min-location tie-break must order them deterministically by source position, NOT by
    // group-iteration order — otherwise the kept set (and so the dup-gate count) would be
    // sensitive to incidental ordering.
    let mk = |f1: &'static str, f2: &'static str| Group {
        score: 1.0,
        members: vec![loc(f1, 1, 20, "rust"), loc(f2, 1, 20, "rust")],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let keys = |groups| {
        rank_families(&report(groups))
            .iter()
            .map(|f| family_min_loc(f).map(|(file, s, e)| (file.to_string(), s, e)))
            .collect::<Vec<_>>()
    };
    let forward = keys(vec![mk("a.rs", "a2.rs"), mk("b.rs", "b2.rs")]);
    let reversed = keys(vec![mk("b.rs", "b2.rs"), mk("a.rs", "a2.rs")]);
    assert_eq!(
        forward, reversed,
        "dedup result must not depend on group input order"
    );
    assert_eq!(
        forward[0].as_ref().unwrap().0,
        "a.rs",
        "tied families order by source position",
    );
}

#[test]
fn collapses_overlapping_and_nested_sites() {
    // A function (247-273) and an inner block (259-271), plus a near-identical
    // off-by-one span (143-167 vs 144-167) all collapse to their enclosing site.
    let g = Group {
        score: 0.9,
        members: vec![
            loc("seg.py", 247, 273, "python"), // function
            loc("seg.py", 259, 271, "python"), // inner block — contained
            loc("seg.py", 276, 304, "python"), // a distinct second function
            loc("seg.py", 290, 302, "python"), // inner block — contained
            loc("con.py", 143, 167, "python"),
            loc("con.py", 144, 167, "python"), // off-by-one near-duplicate
        ],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let f = &rank_families(&report(vec![g]))[0];
    assert_eq!(
        f.members, 3,
        "two functions in seg.py + one region in con.py = 3 sites"
    );
    assert_eq!(f.files, 2);
}

#[test]
fn keeps_adjacent_distinct_sites() {
    // Adjacent but non-overlapping regions are genuinely separate sites.
    let g = Group {
        score: 0.9,
        members: vec![
            loc("p.py", 714, 762, "python"),
            loc("p.py", 763, 794, "python"),
            loc("p.py", 795, 818, "python"),
        ],
        semantic_laws: Vec::new(),
        abstraction_witness: None,
        witness: None,
    };
    let f = &rank_families(&report(vec![g]))[0];
    assert_eq!(
        f.members, 3,
        "adjacent non-overlapping regions stay distinct"
    );
}
