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
    assert!(
        report["oracle_exclusions"]["by_classification"]
            .as_array()
            .is_some(),
        "oracle exclusions should expose classification rollups: {report}"
    );

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
                && item["obligation_family"] == "ambiguous-selector-boundary"
                && item["obligation_subreason"].is_string()
                && item["oracle_status"] == "interpretable"),
        "expected structured exact-admission rejection: {report}"
    );
    assert!(
        report["by_obligation"]
            .as_array()
            .expect("by_obligation should be an array")
            .iter()
            .any(
                |item| item["obligation_family"] == "ambiguous-selector-boundary"
                    && item["count"].as_u64().unwrap_or(0) >= 1
            ),
        "expected obligation rollup for selector/callee proof: {report}"
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
    report_dir.write(
        "hof_callback_member_call.rs",
        "use std::path::PathBuf;\n\n\
fn command_paths(paths: &[PathBuf]) -> Vec<String> {\n\
    paths.iter().map(|p| p.display().to_string()).collect()\n\
}\n",
    );
    let report_path = report_dir.path().join("recall-loss.json");
    let mut args = vec!["verify"];
    let path_strings = paths
        .iter()
        .map(|path| workspace.join(path).display().to_string())
        .collect::<Vec<_>>();
    args.extend(path_strings.iter().map(String::as_str));
    let hof_callback_path = report_dir
        .path()
        .join("hof_callback_member_call.rs")
        .display()
        .to_string();
    args.push(hof_callback_path.as_str());
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
    assert_representative_admission_buckets(&report);
    assert_representative_obligation_buckets(&report);
}

fn assert_representative_admission_buckets(report: &serde_json::Value) {
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

fn assert_representative_obligation_buckets(report: &serde_json::Value) {
    let obligations = report["by_obligation"]
        .as_array()
        .expect("by_obligation should be an array");
    for expected in [
        "ambiguous-selector-boundary",
        "callback-demand-effect",
        "receiver-mutation",
        "source-protocol-boundary",
    ] {
        assert!(
            obligations
                .iter()
                .any(|item| item["obligation_family"] == expected
                    && item["count"].as_u64().unwrap_or(0) >= 1),
            "expected representative obligation family {expected}: {report}"
        );
    }
    assert!(
        obligations
            .iter()
            .any(|item| item["obligation_family"] == "callback-demand-effect"
                && item["obligation_subreason"] == "callback-member-call-effect-proof-missing"
                && item["count"].as_u64().unwrap_or(0) >= 1),
        "expected callback-demand/effect rollup to expose callback member call-effect proof misses: {report}"
    );
    assert!(
        report["admission_rejections"]
            .as_array()
            .expect("admission_rejections should be an array")
            .iter()
            .any(|item| item["reason"] == "hof-demand-effect-proof-missing"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items
                        .iter()
                        .any(|value| value == "hof-callback-call-effect-proof")
                        && items
                            .iter()
                            .any(|value| value == "hof-callback-member-call-effect-proof"))),
        "expected HOF rejections to include generic and member callback call-effect missing evidence: {report}"
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
                && item["obligation_family"] == "source-protocol-boundary"
                && item["obligation_subreason"] == "rust-macro-expansion-contract-missing"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items
                        .iter()
                        .any(|value| value == "rust-macro-expansion-contract"))),
        "expected macro-specific missing evidence: {report}"
    );
}

#[test]
fn recall_loss_report_splits_promise_protocol_boundaries() {
    let project = TempProject::new("recall_loss_promise_protocol_boundaries");
    project.write(
        "promise.js",
        "function makePromise(x) { return new Promise(x); }\n\
function resolveIt(x) { return Promise.resolve(x); }\n\
function allIt(xs) { return Promise.all(xs); }\n\
function raceIt(xs) { return Promise.race(xs); }\n\
function allSettledIt(xs) { return Promise.allSettled(xs); }\n\
function anyIt(xs) { return Promise.any(xs); }\n\
function rejectIt(e) { return Promise.reject(e); }\n\
function callPromise(x) { return Promise(x); }\n\
function intervalIt(f) { return setInterval(f, 10); }\n",
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
    for (family, subreason) in [
        (
            "executor-callback",
            "promise-executor-timing-contract-missing",
        ),
        (
            "success-error-result-channel",
            "promise-factory-settled-value-contract-missing",
        ),
        (
            "success-error-result-channel",
            "promise-aggregate-all-fulfilled-contract-missing",
        ),
        (
            "cancellation-liveness-boundary",
            "promise-aggregate-first-settled-contract-missing",
        ),
        (
            "success-error-result-channel",
            "promise-aggregate-all-settled-contract-missing",
        ),
        (
            "success-error-result-channel",
            "promise-aggregate-first-fulfilled-contract-missing",
        ),
        (
            "rejection-channel",
            "promise-reject-rejected-value-channel-contract-missing",
        ),
        (
            "scheduling-boundary",
            "promise-non-construct-call-boundary-contract-missing",
        ),
        (
            "lifecycle-materialization-boundary",
            "interval-async-iteration-lifecycle-contract-missing",
        ),
    ] {
        assert_obligation(&report, family, subreason);
    }

    for expected in [
        "promise-executor-timing-contract",
        "promise-executor-resolve-reject-callback-contract",
        "promise-executor-throw-to-rejection-contract",
        "promise-executor-callback-effect-contract",
        "promise-factory-settled-value-contract",
        "promise-aggregate-all-fulfilled-contract",
        "promise-aggregate-ordered-values-contract",
        "promise-aggregate-first-settled-contract",
        "promise-aggregate-cancellation-liveness-contract",
        "promise-aggregate-all-settled-contract",
        "promise-aggregate-settled-record-shape-contract",
        "promise-aggregate-first-fulfilled-contract",
        "promise-aggregate-error-channel-contract",
        "promise-aggregate-result-channel-contract",
        "promise-reject-rejected-value-channel-contract",
        "promise-non-construct-call-boundary-contract",
        "interval-async-iteration-lifecycle-contract",
    ] {
        assert_runtime_missing_evidence(&report, expected);
    }
    let generic = "promise-rejection-channel-contract";
    let rejections = report["admission_rejections"]
        .as_array()
        .expect("admission_rejections should be an array");
    assert!(
        !rejections
            .iter()
            .any(|item| item["reason"] == "unsupported-runtime-boundary"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|value| value == generic))),
        "generic Promise rejection evidence label should stay split: {report}"
    );
}

fn assert_obligation(report: &serde_json::Value, family: &str, subreason: &str) {
    let obligations = report["by_obligation"]
        .as_array()
        .expect("by_obligation should be an array");
    assert!(
        obligations
            .iter()
            .any(|item| item["obligation_family"] == family
                && item["obligation_subreason"] == subreason
                && item["count"].as_u64().unwrap_or(0) >= 1),
        "expected Promise obligation {family}/{subreason}: {report}"
    );
}

fn assert_runtime_missing_evidence(report: &serde_json::Value, expected: &str) {
    let rejections = report["admission_rejections"]
        .as_array()
        .expect("admission_rejections should be an array");
    assert!(
        rejections
            .iter()
            .any(|item| item["reason"] == "unsupported-runtime-boundary"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|value| value == expected))),
        "expected Promise missing evidence label {expected}: {report}"
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
                && item["obligation_family"] == "ambiguous-selector-boundary"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|value| value == expected))),
            "expected callee identity evidence label {expected}: {report}"
        );
    }
}
