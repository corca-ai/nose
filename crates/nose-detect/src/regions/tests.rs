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

#[test]
fn shared_analysis_key_checks_every_serialized_input() {
    let mut reference = units(&[("a.py", SOURCE)]).pop().unwrap();
    reference.review_value = None;
    reference.proof_facts = Some(crate::fragment::ProofFacts::self_field_body());
    let shared = AnalysisKeyReference::new(&reference);
    let legacy = digest(
        b"nose.region-analysis/v1",
        &(
            &reference.value,
            &reference.returns,
            &reference.cond_sinks,
            reference.exact_safe,
            &reference.proof_facts,
            &reference.semantic_laws,
        ),
    );
    assert_eq!(shared.key_for(&reference, false), legacy);
    for field in 0..6 {
        let mut changed = units(&[("a.py", SOURCE)]).pop().unwrap();
        changed.path = "moved.py".into();
        changed.review_value = None;
        changed.proof_facts = reference.proof_facts;
        assert_eq!(shared.key_for(&changed, false), legacy);
        match field {
            0 => changed.value.push(u64::MAX),
            1 => changed.returns.push(u64::MAX),
            2 => changed.cond_sinks.push(u64::MAX),
            3 => changed.exact_safe = !changed.exact_safe,
            4 => changed.proof_facts.as_mut().unwrap().context_safe = true,
            5 => changed
                .semantic_laws
                .push(nose_semantics::ValueLaw::AddCommutativity),
            _ => unreachable!(),
        }
        assert_ne!(unit_analysis_key(&changed), legacy);
        assert_eq!(shared.key_for(&changed, false), unit_analysis_key(&changed));
        if field != 0 {
            assert_eq!(shared.key_for(&changed, true), unit_analysis_key(&changed));
        }
    }
}

#[test]
fn shared_analysis_key_uses_the_selected_review_values() {
    let mut source = units(&[("a.py", SOURCE), ("b.py", SOURCE)]);
    let mut changed = source.pop().unwrap();
    let mut reference = source.pop().unwrap();
    reference.review_value = None;
    changed.review_value = Some(nose_normalize::ReviewValueFingerprint {
        values: reference.value.clone(),
        returns: reference.returns.clone(),
        cond_sinks: reference.cond_sinks.clone(),
    });
    changed.value.push(u64::MAX);
    changed.returns.push(u64::MAX);
    changed.cond_sinks.push(u64::MAX);
    let shared = AnalysisKeyReference::new(&reference);
    assert_eq!(
        shared.key_for(&changed, false),
        unit_analysis_key(&reference)
    );
    changed
        .review_value
        .as_mut()
        .unwrap()
        .returns
        .push(u64::MAX);
    assert_ne!(
        shared.key_for(&changed, false),
        unit_analysis_key(&reference)
    );
    assert_eq!(shared.key_for(&changed, false), unit_analysis_key(&changed));
}

#[test]
fn group_locations_preserve_each_members_key_and_order() {
    let mut source = units(&[("a.py", SOURCE), ("b.py", SOURCE), ("c.py", SOURCE)]);
    source[1].exact_safe = !source[1].exact_safe;
    let enclosing = vec![None; source.len()];
    for threads in [1, 3] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        for count in [0, 1, 4, 255, 256, 257, 513] {
            let members = [2, 0, 1, 2]
                .into_iter()
                .cycle()
                .take(count)
                .collect::<Vec<_>>();
            let actual = pool
                .install(|| crate::locations::group_locations(&source, &members, &enclosing, true));
            let expected = members
                .iter()
                .map(|&index| crate::locations::loc_of(&source[index], None))
                .collect::<Vec<_>>();
            assert_eq!(
                actual
                    .iter()
                    .map(|loc| loc.analysis_digest)
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|loc| loc.analysis_digest)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                rmp_serde::to_vec(&actual).unwrap(),
                rmp_serde::to_vec(&expected).unwrap()
            );
        }
    }
}

#[test]
fn exact_value_evidence_keeps_selected_review_overrides() {
    let mut reference = units(&[("a.py", SOURCE)]).pop().unwrap();
    let mut changed = units(&[("b.py", SOURCE)]).pop().unwrap();
    reference.review_value = None;
    changed.review_value = None;
    assert_eq!(reference.value, changed.value);
    assert_eq!(
        AnalysisKeyReference::new(&reference).key_for(&changed, true),
        unit_analysis_key(&changed)
    );
    changed.review_value = Some(nose_normalize::ReviewValueFingerprint {
        values: vec![u64::MAX],
        returns: vec![1],
        cond_sinks: vec![2],
    });
    assert_ne!(unit_analysis_key(&changed), unit_analysis_key(&reference));
    assert_eq!(
        AnalysisKeyReference::new(&reference).key_for(&changed, true),
        unit_analysis_key(&changed)
    );
    // An override on the reference side likewise prevents raw-value reuse.
    assert_eq!(
        AnalysisKeyReference::new(&changed).key_for(&reference, true),
        unit_analysis_key(&reference)
    );
}
