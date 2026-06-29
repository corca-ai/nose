use super::*;

#[test]
fn recall_loss_report_attributes_await_oracle_exclusions_to_shared_scheduling_obligation() {
    let project = TempProject::new("recall_loss_cross_language_await_exclusions");
    project.write(
        "await.js",
        "async function idAsync(x) {\n  return await x + 1;\n}\n",
    );
    project.write(
        "await.ts",
        "async function idAsync(x: Promise<number>) {\n  return await x + 1;\n}\n",
    );
    project.write(
        "await.py",
        "async def id_async(x):\n    return await x + 1\n",
    );
    project.write(
        "await.rs",
        "async fn id_async(x: i32) -> i32 { async move { x + 1 }.await }\n",
    );
    project.write(
        "await.swift",
        "func idAsync(_ x: Int) async -> Int { return await x + 1 }\n",
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
    let exclusion_obligations = report["oracle_exclusions"]["by_obligation"]
        .as_array()
        .expect("oracle_exclusions.by_obligation should be an array");
    assert!(
        exclusion_obligations.iter().any(|item| item["exclusion_reason"] == "uninterpretable"
            && item["attribution_reason"] == "unsupported-runtime-boundary"
            && item["obligation_family"] == "scheduling-boundary"
            && item["obligation_subreason"] == "async-await-scheduling-contract-missing"
            && item["oracle_excluded"].as_u64().unwrap_or(0) >= 5),
        "expected cross-language await exclusions to roll up under the shared scheduling obligation: {report}"
    );

    let excluded_units = report["oracle_exclusions"]["units"]
        .as_array()
        .expect("oracle_exclusions.units should be an array");
    for language in ["javascript", "typescript", "python", "rust", "swift"] {
        assert!(
            excluded_units
                .iter()
                .any(|item| item["reason"] == "uninterpretable"
                    && item["loc"]["language"] == language
                    && item["attribution"]["oracle_status"] == "excluded"
                    && item["attribution"]["capability_id"] == "runtime-boundary-model"
                    && item["attribution"]["missing_evidence"]
                        .as_array()
                        .is_some_and(|items| items
                            .iter()
                            .any(|value| value == "async-await-scheduling-contract"))),
            "expected {language} await exclusion attribution: {report}"
        );
    }
    assert!(
        report["by_obligation"]
            .as_array()
            .expect("by_obligation should be an array")
            .iter()
            .all(|item| item["obligation_subreason"]
                != "async-await-scheduling-contract-missing"),
        "oracle-excluded await rows should not be mixed into interpretable admission rollups: {report}"
    );
}
