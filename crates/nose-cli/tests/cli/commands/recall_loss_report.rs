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
fn recall_loss_report_includes_import_snapshot_miss_census() {
    let project = TempProject::new("recall_loss_import_snapshot_census");
    project.write("maps.py", "LOOKUP = {\"red\": 1, \"blue\": 2}\n");
    project.write(
        "safe_imported.py",
        "from maps import LOOKUP\n\n\
def lookup(value):\n    return LOOKUP.get(value, 0)\n",
    );
    project.write("values.py", "VALUES = [\"red\", \"blue\"]\n");
    project.write(
        "shadowed.py",
        "def set(_values):\n    return object()\n\
VALUES = set([\"red\", \"blue\"])\n",
    );
    project.write(
        "shadowed_imported.py",
        "from shadowed import VALUES\n\n\
def member(value):\n    return value in VALUES\n",
    );
    project.write(
        "mutated_provider.py",
        "VALUES = [\"red\", \"blue\"]\n\
VALUES.append(\"green\")\n",
    );
    project.write(
        "mutated_provider_imported.py",
        "from mutated_provider import VALUES\n\n\
def member(value):\n    return value in VALUES\n",
    );
    project.write(
        "mutated_importer.py",
        "from values import VALUES\n\
VALUES.append(\"green\")\n\n\
def member(value):\n    return value in VALUES\n",
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
    let census = &report["import_snapshot_census"];
    assert!(
        census["summary"]["snapshot_records"].as_u64().unwrap_or(0) >= 1,
        "expected at least one successful imported snapshot: {report}"
    );
    assert_eq!(census["summary"]["reported_misses"], 3);
    let reasons = census["misses_by_reason"]
        .as_array()
        .expect("misses_by_reason should be an array");
    for expected in [
        "provider-library-api-proof-missing",
        "provider-binding-unsafe",
        "importer-binding-mutated",
    ] {
        assert!(
            reasons
                .iter()
                .any(|item| item["key"] == expected && item["count"] == 1),
            "expected import snapshot miss reason {expected}: {report}"
        );
    }
    assert!(
        census["misses"]
            .as_array()
            .expect("misses should be an array")
            .iter()
            .all(|item| item["snapshot_kind"] == "binding-import"
                && item["module_hash"].is_u64()
                && item["exported_hash"].is_u64()),
        "miss rows should expose stable hashes without source snippets: {report}"
    );
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

#[test]
fn recall_loss_report_splits_rust_effect_and_macro_boundaries_from_callee_identity() {
    let project = TempProject::new("recall_loss_rust_callee_boundaries");
    project.write(
        "sample.rs",
        "use std::fs;\n\
use std::path::Path;\n\n\
pub fn write_fixture(dir: &Path) {\n\
    fs::write(dir.join(\"case.txt\"), \"value\").unwrap();\n\
}\n\n\
pub fn format_key(key: u64) -> String {\n\
    format!(\"{key:016x}\")\n\
}\n",
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
    let reasons = report["by_reason"]
        .as_array()
        .expect("by_reason should be an array");
    assert!(
        reasons
            .iter()
            .any(|item| item["reason"] == "mutation-effect-boundary"
                && item["count"].as_u64().unwrap_or(0) >= 1),
        "expected fs::write to be attributed to the effect boundary: {report}"
    );
    assert!(
        reasons
            .iter()
            .any(|item| item["reason"] == "source-surface-proof-missing"
                && item["count"].as_u64().unwrap_or(0) >= 1),
        "expected format! to be attributed to the Rust macro source boundary: {report}"
    );
    assert!(
        report["admission_rejections"]
            .as_array()
            .expect("admission_rejections should be an array")
            .iter()
            .any(|item| item["reason"] == "source-surface-proof-missing"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items
                        .iter()
                        .any(|value| value == "rust-macro-expansion-contract"))),
        "expected macro-specific missing evidence: {report}"
    );
}

#[test]
fn recall_loss_report_classifies_callee_identity_surfaces() {
    let project = TempProject::new("recall_loss_callee_identity_surfaces");
    project.write(
        "sample.rs",
        "pub fn call_local(x: u64) -> u64 {\n\
    helper(x)\n\
}\n\n\
pub fn call_scoped(x: u64) -> u64 {\n\
    helper_mod::value(x)\n\
}\n",
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
    let rejections = report["admission_rejections"]
        .as_array()
        .expect("admission_rejections should be an array");
    for expected in [
        "local-or-parameter-call-target-proof",
        "scoped-path-call-target-proof",
    ] {
        assert!(
            rejections.iter().any(|item| item["reason"]
                == "import-symbol-callee-identity-proof-missing"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|value| value == expected))),
            "expected callee identity evidence label {expected}: {report}"
        );
    }
}
