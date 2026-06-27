use super::*;

#[test]
fn verify_writes_local_recall_loss_report_without_source_snippets() {
    let project = TempProject::new("recall_loss_report");
    project.write(
        "sample.py",
        "def plus_left(x):\n    return x + 1\n\n\
def plus_right(y):\n    return 1 + y\n\n\
def tiny(z):\n    return z\n\n\
def custom_call(v):\n    return helper(v)\n",
    );
    let report_path = project.path().join("recall-loss.json");
    let out = run_raw(&[
        "verify",
        project.path().to_str().unwrap(),
        "--max-violations",
        "0",
        "--recall-loss-report",
        report_path.to_str().unwrap(),
    ]);
    assert!(out.contains("GATE: 0"));

    let report_text = fs::read_to_string(&report_path).expect("recall-loss report");
    let report: serde_json::Value =
        serde_json::from_str(&report_text).expect("recall-loss report JSON");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["report_kind"], "recall-loss-diagnostics");
    assert_eq!(report["privacy"]["local_artifact"], true);
    assert_eq!(report["privacy"]["remote_collection"], false);
    assert_eq!(report["privacy"]["raw_source_snippets_included"], false);
    assert_eq!(report["soundness_gate"]["false_merges"], 0);
    assert_eq!(report["soundness_gate"]["canon_preservation_violations"], 0);
    assert_eq!(report["soundness_gate"]["gate_passed"], true);

    let reasons = report["by_reason"]
        .as_array()
        .expect("by_reason should be an array");
    assert!(
        reasons.iter().any(
            |item| item["reason"] == "import-symbol-callee-identity-proof-missing"
                && item["count"].as_u64().unwrap_or(0) >= 1
        ),
        "expected callee identity rejection: {report}"
    );
    assert!(
        reasons
            .iter()
            .any(|item| item["reason"] == "value-fingerprint-too-small"
                && item["count"].as_u64().unwrap_or(0) >= 1),
        "expected value fingerprint floor rejection: {report}"
    );
    assert!(
        report["admission_rejections"]
            .as_array()
            .expect("admission_rejections should be an array")
            .iter()
            .any(|item| item["capability_id"] == "callee-identity-evidence"
                && item["oracle_status"] == "interpretable"),
        "expected structured exact-admission rejection: {report}"
    );

    assert!(!report_text.contains("def custom_call"));
    assert!(!report_text.contains("return helper"));
}

#[test]
fn recall_loss_report_ratchets_representative_admission_buckets() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let paths = [
        "crates/nose-cli/src/baseline.rs",
        "crates/nose-cli/src/recall_loss_report.rs",
        "crates/nose-cli/src/oracle_gate.rs",
        "crates/nose-detect/src/model.rs",
        "crates/nose-cli/tests/fixtures/string_affix_552/param_python.py",
        "crates/nose-frontend/src/js_ts/globals.rs",
    ];
    let report_dir = TempProject::new("recall_loss_bucket_ratchet");
    let report_path = report_dir.path().join("recall-loss.json");
    let mut args = vec!["verify"];
    let path_strings = paths
        .iter()
        .map(|path| workspace.join(path).display().to_string())
        .collect::<Vec<_>>();
    args.extend(path_strings.iter().map(String::as_str));
    args.extend([
        "--max-violations",
        "0",
        "--recall-loss-report",
        report_path.to_str().unwrap(),
    ]);
    let out = run_raw(&args);
    assert!(out.contains("GATE: 0"));

    let report_text = fs::read_to_string(&report_path).expect("recall-loss report");
    let report: serde_json::Value =
        serde_json::from_str(&report_text).expect("recall-loss report JSON");
    assert_eq!(report["soundness_gate"]["false_merges"], 0);
    assert_eq!(report["soundness_gate"]["canon_preservation_violations"], 0);

    let reasons = report["by_reason"]
        .as_array()
        .expect("by_reason should be an array");
    for expected in [
        "import-symbol-callee-identity-proof-missing",
        "receiver-domain-proof-missing",
        "hof-demand-effect-proof-missing",
        "mutation-effect-boundary",
        "source-surface-proof-missing",
    ] {
        assert!(
            reasons
                .iter()
                .any(|item| item["reason"] == expected && item["count"].as_u64().unwrap_or(0) >= 1),
            "expected representative bucket {expected}: {report}"
        );
    }
    assert!(
        reasons
            .iter()
            .all(|item| item["reason"] != "unattributed-strict-exact-unsafe"),
        "bucket ratchet should not leave opaque strict-exact rejections: {report}"
    );
}
