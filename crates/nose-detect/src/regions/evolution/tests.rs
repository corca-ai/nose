use super::*;
use crate::regions::{review_tests::with_pack, tests::three_copy_family};
use nose_il::ContentDigest;
use std::collections::BTreeMap;

fn snapshot(family: &crate::RefactorFamily) -> AnalysisSnapshot {
    AnalysisSnapshot {
        schema: "nose.analysis/v1".into(),
        profile: BTreeMap::from([("engine".into(), "test".into())]),
        roots: vec![".".into()],
        path_base: ".".into(),
        scanned_files: 3,
        skipped_sources: 0,
        population: "admitted-query-families".into(),
        complete: true,
        families: vec![FamilyObservation::capture(family)],
    }
}

#[test]
fn actual_pack_projection_explains_receipt_changes_and_preserves_movement() {
    let mut family = with_pack(true);
    let before = snapshot(&family);
    let loc = &mut family.locations[0];
    loc.file = "moved.py".into();
    loc.semantic_pack_external_exact[0].occurrence_file = loc.file.clone();
    family.semantic_pack_external_exact = family
        .locations
        .iter()
        .flat_map(|l| l.semantic_pack_external_exact.clone())
        .collect();
    let moved = snapshot(&family);
    let result = compare(&before, &moved, 1000).unwrap();
    assert!(result.changes[0].unchanged_evidence);
    family.locations[0].semantic_pack_external_exact[0].receipt_digest = "new receipt".into();
    family.semantic_pack_external_exact = family
        .locations
        .iter()
        .flat_map(|l| l.semantic_pack_external_exact.clone())
        .collect();
    let changed = snapshot(&family);
    let result = compare(&before, &changed, 1000).unwrap();
    assert!(!result.changes[0].unchanged_evidence);
    assert!(result.changes[0]
        .reasons
        .iter()
        .any(|r| r == "packs-changed"));
    assert_eq!(
        changed.families[0].exact_provenance[0].receipt_digest,
        "new receipt"
    );
}

#[test]
fn opaque_analysis_and_member_scope_deltas_cannot_retain_review_evidence() {
    let mut family = three_copy_family();
    let before = snapshot(&family);
    family.locations[0].analysis_digest = Some(ContentDigest::sha256(b"changed analysis"));
    let result = compare(&before, &snapshot(&family), 1000).unwrap();
    assert!(!result.changes[0].unchanged_evidence);
    assert!(result.changes[0]
        .reasons
        .iter()
        .any(|r| r == "analysis-changed"));
    let mut after = before.clone();
    after.families[0].members[0].in_test = true;
    after.families[0].id = after.families[0].address();
    let result = compare(&before, &after, 1000).unwrap();
    assert!(!result.changes[0].unchanged_evidence);
    assert!(result.changes[0]
        .reasons
        .iter()
        .any(|r| r == "scope-changed"));
}

#[test]
fn all_identical_copies_moved_without_anchors_remain_ambiguous() {
    let mut family = three_copy_family();
    let before = snapshot(&family);
    for loc in &mut family.locations {
        loc.file = format!("new/{}", loc.file);
    }
    let result = compare(&before, &snapshot(&family), 1000).unwrap();
    assert!(result.changes.iter().all(|r| !r.unchanged_evidence));
    assert_eq!(result.changes[0].correspondence, "ambiguous");
}

#[test]
fn a_disappearing_family_is_unresolved_and_not_a_refactoring_success() {
    let before = snapshot(&three_copy_family());
    let mut after = before.clone();
    after.families.clear();
    let result = compare(&before, &after, 1000).unwrap();
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].correspondence, "unresolved");
    assert!(!result.changes[0].unchanged_evidence);
}

#[test]
fn serialized_observation_corruption_and_partial_coverage_fail_closed() {
    let before = snapshot(&three_copy_family());
    let mut after = before.clone();
    after.families[0].members[0].name = Some("tampered".into());
    assert!(compare(&before, &after, 1000).is_err());
    let mut after = before.clone();
    after.skipped_sources = 1;
    let result = compare(&before, &after, 1000).unwrap();
    assert!(!result.complete);
    assert!(result.changes.iter().all(|r| !r.unchanged_evidence));
}
