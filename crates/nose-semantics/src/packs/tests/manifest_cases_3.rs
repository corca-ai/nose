use super::*;

#[test]
fn passed_executable_gates_clear_only_executable_preflight_blocker() {
    let dir = unique_dir("external_preflight_executable");
    let fixture_dir = dir.join("fixtures");
    fs::create_dir_all(&fixture_dir).unwrap();
    fs::write(
        fixture_dir.join("positive.py"),
        "def positive(xs):\n    return sum(xs)\n",
    )
    .unwrap();
    fs::write(
        fixture_dir.join("negative.py"),
        "def negative(xs):\n    return list(xs)\n",
    )
    .unwrap();
    let path = dir.join("pack.json");
    fs::write(&path, manifest_with_executable_gates("com.example.gated")).unwrap();

    let conformance = check_semantic_pack_conformance(std::slice::from_ref(&path))
        .expect("manifest with executable gates loads");
    assert!(conformance.passed());
    assert_eq!(conformance.executable_conformance_count(), 2);
    assert_eq!(conformance.passed_executable_conformance_count(), 2);
    assert_eq!(conformance.executable_conformance_issue_count(), 0);

    let set = SemanticPackSet::new_local(std::slice::from_ref(&path)).expect("pack loads");
    let plain = set.external_influence_preflight();
    assert!(plain.rows.iter().all(|row| row
        .blockers
        .contains(&ExternalInfluenceBlocker::ExecutableConformanceUnavailable)));

    let report = set.external_influence_preflight_with_conformance(&conformance);
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.blocked_count(), 2);
    for row in &report.rows {
        assert_eq!(row.pack_id, "com.example.gated");
        assert!(row
            .blockers
            .contains(&ExternalInfluenceBlocker::DataOnlyRegistration));
        assert!(row
            .blockers
            .contains(&ExternalInfluenceBlocker::DependencyBackedEvidenceUnavailable));
        assert!(row
            .blockers
            .contains(&ExternalInfluenceBlocker::ExplicitInfluenceTrustGateMissing));
        assert!(!row
            .blockers
            .contains(&ExternalInfluenceBlocker::ExecutableConformanceUnavailable));
        assert!(!row
            .blockers
            .contains(&ExternalInfluenceBlocker::RowConflict));
        assert!(!row.passed());
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn executable_gates_support_value_law_rows() {
    let dir = unique_dir("external_preflight_executable_law");
    let fixture_dir = dir.join("fixtures");
    fs::create_dir_all(&fixture_dir).unwrap();
    fs::write(
        fixture_dir.join("positive.py"),
        "def positive(x):\n    return x + 0\n",
    )
    .unwrap();
    fs::write(
        fixture_dir.join("negative.py"),
        "def negative(x):\n    return x + 1\n",
    )
    .unwrap();
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest_with_value_law_executable_gate("com.example.gated-laws"),
    )
    .unwrap();

    let conformance =
        check_semantic_pack_conformance(std::slice::from_ref(&path)).expect("value-law gate loads");
    assert!(conformance.passed());
    assert_eq!(conformance.executable_conformance_count(), 1);
    let gate = &conformance.manifests[0].executable[0];
    assert_eq!(gate.kind, ExternalRowKind::ValueLaw);
    assert_eq!(gate.row_id, "python.example.numeric-law");

    let set = SemanticPackSet::new_local(std::slice::from_ref(&path)).expect("pack loads");
    let report = set.external_influence_preflight_with_conformance(&conformance);
    let law = report
        .rows
        .iter()
        .find(|row| row.kind == ExternalRowKind::ValueLaw)
        .expect("value-law preflight row");
    assert!(!law
        .blockers
        .contains(&ExternalInfluenceBlocker::ExecutableConformanceUnavailable));
    assert!(law
        .blockers
        .contains(&ExternalInfluenceBlocker::DataOnlyRegistration));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn executable_conformance_fails_on_hard_negative_oracle_mismatch() {
    let dir = unique_dir("conformance_executable_mismatch");
    let fixture_dir = dir.join("fixtures");
    fs::create_dir_all(&fixture_dir).unwrap();
    fs::write(
        fixture_dir.join("positive.py"),
        "def positive(xs):\n    return sum(xs)\n",
    )
    .unwrap();
    fs::write(
        fixture_dir.join("negative.py"),
        "def negative(xs):\n    return list(xs)\n",
    )
    .unwrap();
    let path = dir.join("pack.json");
    fs::write(
        &path,
        manifest_with_executable_gates("com.example.pack").replace(
            r#""expected_hard_negative": "exact-contract-closed""#,
            r#""expected_hard_negative": "exact-contract-open""#,
        ),
    )
    .unwrap();

    let report = check_semantic_pack_conformance(&[path]).expect("manifest is structurally valid");

    assert!(!report.passed());
    assert_eq!(report.fixture_issue_count(), 0);
    assert_eq!(report.executable_conformance_count(), 2);
    assert_eq!(report.executable_conformance_issue_count(), 2);
    assert!(report.manifests[0].executable.iter().all(|gate| gate
        .issues
        .contains(&SemanticPackExecutableConformanceIssue::ExpectationMismatch)));
    let _ = fs::remove_dir_all(dir);
}
