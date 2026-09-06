use super::*;
use crate::query_dataset::preserve_query_accepted_coverage;

fn accepted_pair(sites: Vec<Loc>) -> nose_detect::AcceptedCoverage {
    nose_detect::AcceptedCoverage {
        sites,
        edges: vec![nose_detect::AcceptedEdge {
            left: 0,
            right: 1,
            score: 1.0,
            witness_kind: "exact-value-graph",
        }]
        .into(),
    }
}

#[test]
fn ordinary_query_keeps_the_pre_target_accepted_coverage_contract() {
    let mut family = fam_at(&[("t/a.go", 1, 20), ("t/b.go", 1, 20)]);
    family.direct_edges.push(nose_detect::AcceptedEdge {
        left: 0,
        right: 1,
        score: 1.0,
        witness_kind: "exact-value-graph",
    });

    preserve_query_accepted_coverage(std::slice::from_mut(&mut family));

    assert!(family.direct_edges.is_empty());
    let [coverage] = family.accepted_coverage.as_slice() else {
        panic!("ordinary query should retain one accepted-pair obligation");
    };
    assert_eq!(coverage.sites.len(), 2);
    assert_eq!(coverage.edges.len(), 1);
}

#[test]
fn connected_witness_never_hides_an_existing_family() {
    for kind in ["connected-mapped-sub-dag", "bounded-same-unit-window"] {
        let mut connected = fam_at(&[("t/a.go", 10, 30), ("t/b.go", 10, 30)]);
        connected.witness = Some(nose_detect::EquivalenceWitness {
            evidence: if kind == "bounded-same-unit-window" {
                nose_detect::WitnessEvidence::BoundedSameUnitWindow { value_nodes: 42 }
            } else {
                nose_detect::WitnessEvidence::ConnectedMappedSubDag { value_nodes: 42 }
            },
            graded: None,
            graded_pair: None,
        });
        let existing = fam_at(&[("t/a.go", 1, 40), ("t/b.go", 1, 40)]);
        let groups = OpportunityGroups::from_ranked(&[&connected, &existing]);

        assert!(!groups.is_slice(&existing), "witness kind {kind}");
    }
}

#[test]
fn overlapping_slices_fold_under_their_primary() {
    // B's members are both shifted slices of A's regions → one opportunity.
    // C shares only ONE region with A (its other member lives elsewhere) —
    // a single shared region can be coincidence, so C stays its own entry.
    let a = fam_at(&[("t/a.go", 100, 130), ("t/b.go", 50, 70)]);
    let mut b = fam_at(&[("t/a.go", 105, 128), ("t/b.go", 52, 66)]);
    b.accepted_coverage.push(accepted_pair(b.locations.clone()));
    let c = fam_at(&[("t/a.go", 100, 130), ("t/z.go", 5, 25)]);
    let ranked = [&a, &b, &c];
    let groups = OpportunityGroups::from_ranked(&ranked);
    assert!(groups.is_slice(&b), "b is a slice of a");
    assert!(
        !groups.is_slice(&a),
        "the best-ranked family is the primary"
    );
    assert!(!groups.is_slice(&c), "one shared region must not group");
    assert_eq!(
        groups.slices(&a),
        Some(&[baseline::family_id(&b)][..]),
        "a lists exactly b as its folded slice"
    );
}

#[test]
fn a_slice_stays_visible_when_its_primary_leaves_the_default_surface() {
    let primary = fam_at(&[("docs/a.html", 100, 130), ("docs/b.html", 50, 70)]);
    let slice = fam_at(&[("docs/a.html", 105, 128), ("docs/b.html", 52, 66)]);
    let groups = OpportunityGroups::from_ranked_with_default(&[&primary, &slice], |family| {
        std::ptr::eq(family, &slice)
    });

    assert!(
        groups.is_slice(&slice),
        "the all-surface fold forest remains stable"
    );
    assert!(
        !groups.is_default_slice(&slice),
        "a default family cannot fold under a primary absent from that view"
    );
}

#[test]
fn dense_opportunity_bucket_stays_bounded() {
    let families: Vec<RefactorFamily> = (0..=200)
        .map(|offset| {
            fam_at(&[
                ("t/a.go", 1 + offset, 40 + offset),
                ("t/b.go", 1 + offset, 40 + offset),
            ])
        })
        .collect();
    let ranked: Vec<&RefactorFamily> = families.iter().collect();
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert!(
        families.iter().all(|family| !groups.is_slice(family)),
        "files above the per-file cap skip quadratic opportunity folding"
    );
}

#[test]
fn overlapping_slices_do_not_fold_transitively() {
    // A overlaps B on both members, and B overlaps C on both members, but A
    // does not overlap C at all. B must not bridge C into A's opportunity:
    // doing so hides C even though A does not cover either of C's regions.
    let a = fam_at(&[("t/a.go", 1, 30), ("t/b.go", 1, 30)]);
    let b = fam_at(&[("t/a.go", 13, 42), ("t/b.go", 13, 42)]);
    let mut c = fam_at(&[("t/a.go", 25, 54), ("t/b.go", 25, 54)]);
    c.accepted_coverage.push(accepted_pair(c.locations.clone()));
    let ranked = [&a, &b, &c];
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert!(groups.is_slice(&b), "b is a direct slice of a");
    assert!(
        !groups.is_slice(&c),
        "b must not transitively fold c under non-overlapping a"
    );
    assert_eq!(
        groups.slices(&a),
        Some(&[baseline::family_id(&b)][..]),
        "a lists only the slice it directly covers"
    );
}

#[test]
fn syntax_only_chain_keeps_direct_suppression_provenance() {
    let a = fam_at(&[("t/a.go", 1, 30), ("t/b.go", 1, 30)]);
    let b = fam_at(&[("t/a.go", 13, 42), ("t/b.go", 13, 42)]);
    let c = fam_at(&[("t/a.go", 25, 54), ("t/b.go", 25, 54)]);
    let ranked = [&a, &b, &c];
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert_eq!(
        groups.primary_of.get(&baseline::family_id(&c)),
        Some(&baseline::family_id(&b)),
        "c points to the family it directly overlaps, not transitive root a"
    );
    assert_eq!(
        groups.slices(&a),
        Some(&[baseline::family_id(&b)][..]),
        "a does not manufacture a direct a-c relation"
    );
    assert_eq!(
        groups.slices(&b),
        Some(&[baseline::family_id(&c)][..]),
        "the direct b-c suppression edge remains navigable"
    );
}

#[test]
fn partial_overlap_does_not_hide_a_wider_family() {
    // A overlaps the leading edge of B on both files, but most of B lies beyond
    // A. This is the sqlite sqlEvalFunc loss shape: B covers a separately
    // accepted pair farther down each file, so hiding B under A erases those
    // endpoints from the final query result.
    let a = fam_at(&[("t/a.go", 1, 30), ("t/b.go", 1, 30)]);
    let mut b = fam_at(&[("t/a.go", 16, 80), ("t/b.go", 16, 80)]);
    b.accepted_coverage.push(accepted_pair(b.locations.clone()));
    let ranked = [&a, &b];
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert!(
        !groups.is_slice(&b),
        "a family must stay visible when the proposed primary does not cover it"
    );
}

#[test]
fn syntax_only_partial_overlap_keeps_existing_fold_policy() {
    let a = fam_at(&[("t/a.go", 1, 30), ("t/b.go", 1, 30)]);
    let b = fam_at(&[("t/a.go", 16, 80), ("t/b.go", 16, 80)]);
    let ranked = [&a, &b];
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert!(
        groups.is_slice(&b),
        "a syntax-only overlap carries no accepted endpoint obligation"
    );
}

#[test]
fn one_primary_member_may_cover_multiple_accepted_sites() {
    let a = fam_at(&[("t/a.go", 1, 100), ("t/b.go", 1, 30)]);
    let mut b = fam_at(&[("t/a.go", 10, 40), ("t/b.go", 1, 30)]);
    b.accepted_coverage.push(accepted_pair(vec![
        loc_at("t/a.go", 10, 20, nose_il::UnitKind::Block),
        loc_at("t/a.go", 30, 40, nose_il::UnitKind::Block),
    ]));
    let ranked = [&a, &b];
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert!(
        groups.is_slice(&b),
        "one outer source site may cover multiple accepted inner sites"
    );
}

#[test]
fn accepted_graph_ignores_sites_without_a_direct_edge() {
    let a = fam_at(&[("t/a.go", 1, 30), ("t/b.go", 1, 30)]);
    let mut b = fam_at(&[("t/a.go", 1, 30), ("t/b.go", 1, 30)]);
    b.accepted_coverage.push(nose_detect::AcceptedCoverage {
        sites: vec![
            loc_at("t/a.go", 1, 30, nose_il::UnitKind::Block),
            loc_at("t/b.go", 1, 30, nose_il::UnitKind::Block),
            loc_at("t/c.go", 1, 30, nose_il::UnitKind::Block),
        ],
        edges: vec![nose_detect::AcceptedEdge {
            left: 0,
            right: 1,
            score: 1.0,
            witness_kind: "exact-value-graph",
        }]
        .into(),
    });
    let ranked = [&a, &b];
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert!(
        groups.is_slice(&b),
        "a collapsed site with no direct accepted edge adds no coverage obligation"
    );
}

#[test]
fn existing_root_suppresses_a_pair_redundant_carrier() {
    let a = fam_at(&[("t/a.go", 1, 30), ("t/b.go", 1, 30)]);
    let existing = fam_at(&[("t/a.go", 50, 80), ("t/z.go", 1, 30)]);
    let mut carrier = fam_at(&[("t/a.go", 16, 80), ("t/b.go", 16, 80)]);
    carrier.accepted_coverage.push(accepted_pair(vec![
        loc_at("t/a.go", 51, 60, nose_il::UnitKind::Block),
        loc_at("t/a.go", 70, 80, nose_il::UnitKind::Block),
    ]));
    let ranked = [&a, &existing, &carrier];
    let groups = OpportunityGroups::from_ranked(&ranked);

    assert!(
        groups.is_slice(&carrier),
        "an existing visible root already covers both endpoints of the carrier's accepted edge"
    );
}
