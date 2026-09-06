use super::*;
use crate::{DetectOptions, UnitFeat};
use nose_il::{FileId, Interner, Lang, UnitKind};

const SOURCE: &str = "def compute(x):\n    return (x * x + 7) // 3\n";

fn units(sources: &[(&str, &str)]) -> Vec<UnitFeat> {
    let interner = Interner::new();
    let options = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    sources
        .iter()
        .enumerate()
        .flat_map(|(i, (path, source))| {
            let il = nose_frontend::lower_source(
                FileId(i as u32),
                path,
                source.as_bytes(),
                Lang::Python,
                &interner,
            )
            .unwrap();
            crate::units_of_file(&il, &interner, &options)
                .into_iter()
                .filter(|u| u.kind == UnitKind::Function && u.fragment_kind.is_none())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn snapshot(sources: &[(&str, &str)]) -> RegionSnapshot {
    let snapshot = RegionSnapshot::from_units(&units(sources), "test-profile-v1".into());
    assert_eq!(snapshot.regions.len(), sources.len());
    snapshot
}

#[test]
fn moving_and_shifting_preserve_content_not_addresses() {
    let before = snapshot(&[("a.py", SOURCE), ("b.py", SOURCE)]);
    let padded = format!("# α header\r\n\n{SOURCE}");
    let after = snapshot(&[("moved.py", &padded), ("b.py", SOURCE)]);
    let result = reconcile(&before, &after, 100).unwrap();
    assert!(result.complete);
    assert_eq!(result.correspondences.len(), 2);
    assert!(result.correspondences.iter().all(|r| r.unchanged_evidence));
    assert_eq!(
        result
            .correspondences
            .iter()
            .filter(|r| r.kind == ChangeKind::ContentMatch)
            .count(),
        1
    );
    let moved = result
        .correspondences
        .iter()
        .find(|r| r.kind == ChangeKind::ContentMatch)
        .unwrap();
    assert_ne!(moved.before.unwrap(), moved.after[0]);
}

#[test]
fn copies_are_distinct_and_do_not_inherit_evidence() {
    let before = snapshot(&[("a.py", SOURCE)]);
    let after = snapshot(&[("a.py", SOURCE), ("b.py", SOURCE)]);
    assert_ne!(
        after.regions[0].observation_id,
        after.regions[1].observation_id
    );
    assert_eq!(after.regions[0].content_key, after.regions[1].content_key);
    let result = reconcile(&before, &after, 100).unwrap();
    let copy = result
        .correspondences
        .iter()
        .find(|r| r.kind == ChangeKind::CopiedCandidate)
        .unwrap();
    assert!(!copy.unchanged_evidence);
}

#[test]
fn indistinguishable_moves_and_many_to_one_abstain() {
    let before = snapshot(&[("a.py", SOURCE), ("b.py", SOURCE)]);
    for after in [
        snapshot(&[("c.py", SOURCE), ("d.py", SOURCE)]),
        snapshot(&[("c.py", SOURCE)]),
    ] {
        let result = reconcile(&before, &after, 100).unwrap();
        assert!(result
            .correspondences
            .iter()
            .all(|r| r.kind == ChangeKind::Ambiguous && !r.unchanged_evidence));
    }
}

#[test]
fn edit_scope_analysis_and_profile_changes_require_review() {
    let before = snapshot(&[("a.py", SOURCE)]);
    let changed = SOURCE.replace("+ 7", "+ 8");
    let after = snapshot(&[("a.py", &changed)]);
    let result = reconcile(&before, &after, 100).unwrap();
    assert_eq!(
        result.correspondences[0].kind,
        ChangeKind::ModifiedCandidate
    );
    assert!(!result.correspondences[0].unchanged_evidence);
    for mut after in [
        snapshot(&[("tests/a.py", SOURCE)]),
        before.clone(),
        before.clone(),
    ] {
        if after.regions[0].file == "a.py" {
            after.regions[0].analysis_key = ContentDigest::sha256(b"changed dependency");
        }
        assert!(reconcile(&before, &after, 100)
            .unwrap()
            .correspondences
            .iter()
            .all(|r| !r.unchanged_evidence));
    }
    let mut after = before.clone();
    after.profile = "other-analysis-profile".into();
    assert!(!reconcile(&before, &after, 100).unwrap().correspondences[0].unchanged_evidence);
}

#[test]
fn budgets_missing_provenance_and_invalid_snapshots_fail_closed() {
    let before = snapshot(&[("a.py", SOURCE)]);
    let mut after = snapshot(&[("b.py", SOURCE)]);
    let result = reconcile(&before, &after, 0).unwrap();
    assert!(!result.complete);
    assert!(result
        .correspondences
        .iter()
        .any(|r| r.kind == ChangeKind::BudgetExceeded));
    assert!(result.correspondences.iter().all(|r| !r.unchanged_evidence));
    after.unavailable_regions = 1;
    assert!(reconcile(&before, &after, 100)
        .unwrap()
        .correspondences
        .iter()
        .all(|r| !r.unchanged_evidence));
    after.regions.push(after.regions[0].clone());
    assert!(reconcile(&before, &after, 100).is_err());
    after.regions.pop();
    after.regions[0].source.end_byte = 0;
    assert!(reconcile(&before, &after, 100).is_err());
}

#[test]
fn reordering_input_never_changes_correspondence() {
    let before = snapshot(&[("a.py", SOURCE), ("b.py", SOURCE)]);
    let after = snapshot(&[("c.py", SOURCE), ("d.py", SOURCE)]);
    let first = rmp_serde::to_vec_named(&reconcile(&before, &after, 100).unwrap()).unwrap();
    let mut before = before;
    let mut after = after;
    before.regions.reverse();
    after.regions.reverse();
    assert_eq!(
        first,
        rmp_serde::to_vec_named(&reconcile(&before, &after, 100).unwrap()).unwrap()
    );
}

pub(super) fn three_copy_family() -> RefactorFamily {
    let interner = Interner::new();
    let files = ["a.py", "b.py", "c.py"]
        .iter()
        .enumerate()
        .map(|(i, path)| {
            nose_frontend::lower_source(
                FileId(i as u32),
                path,
                SOURCE.as_bytes(),
                Lang::Python,
                &interner,
            )
            .unwrap()
        })
        .collect();
    let corpus = nose_il::Corpus::new(interner, files);
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let detector = crate::StructuralDetector::strict(opts.jaccard_weight);
    let report = crate::detect_with_accepted_coverage(&corpus, &opts, &detector);
    crate::rank_families(&report)
        .into_iter()
        .find(|f| f.members == 3)
        .expect("three-copy family")
}

#[test]
fn family_signature_preserves_multiplicity_evidence_and_order_independence() {
    let family = &mut three_copy_family();
    let key = review_key(family).expect("source-backed review key");
    family.locations.reverse();
    let mut edges = family.direct_edges.iter().collect::<Vec<_>>();
    for edge in &mut edges {
        edge.left = 2 - edge.left;
        edge.right = 2 - edge.right;
    }
    edges.reverse();
    family.direct_edges = edges.into();
    family.value += 50.0;
    assert_eq!(review_key(family), Some(key));
    let mut changed = family.clone();
    changed.locations[0]
        .source_region
        .as_mut()
        .unwrap()
        .content_digest = ContentDigest::sha256(b"changed body");
    assert_ne!(review_key(&changed), Some(key));
    changed = family.clone();
    changed.witness.as_mut().unwrap().evidence = crate::WitnessEvidence::CopyPasteRun;
    assert_ne!(review_key(&changed), Some(key));
    changed = family.clone();
    changed.locations.push(changed.locations[0].clone());
    assert_ne!(review_key(&changed), Some(key));
    changed.locations[0].source_region = None;
    assert_eq!(review_key(&changed), None);
}

#[test]
fn a_moved_rewritten_equivalent_is_a_candidate_without_inheriting_review() {
    let before = snapshot(&[("a.py", SOURCE)]);
    let after = snapshot(&[(
        "extracted.py",
        "def renamed(z):\n    tmp = z * z\n    return (tmp + 7) // 3\n",
    )]);
    let result = reconcile(&before, &after, 100).unwrap();
    assert_eq!(result.correspondences[0].kind, ChangeKind::ValueCandidate);
    assert!(!result.correspondences[0].unchanged_evidence);
}

#[test]
fn repeated_content_uses_local_indexes_with_linear_candidate_cost() {
    let paths: Vec<_> = (0..1000).map(|i| format!("p{i}/a.py")).collect();
    let sources: Vec<_> = paths.iter().map(|p| (p.as_str(), SOURCE)).collect();
    let before = snapshot(&sources);
    let shifted = format!("# shifted\n{SOURCE}");
    let sources: Vec<_> = paths
        .iter()
        .map(|p| (p.as_str(), shifted.as_str()))
        .collect();
    let after = snapshot(&sources);
    let result = reconcile(&before, &after, 1000).unwrap();
    assert!(result.complete);
    assert_eq!(result.candidates_examined, 1000);
    assert!(result.correspondences.iter().all(|r| r.unchanged_evidence));
}
