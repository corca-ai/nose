use super::*;

#[test]
fn recall_loss_report_classifies_oracle_exclusions_by_actionable_bucket() {
    let project = TempProject::new("recall_loss_oracle_exclusion_classification");
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
    let classifications = report["oracle_exclusions"]["by_classification"]
        .as_array()
        .expect("oracle_exclusions.by_classification should be an array");
    let units = report["oracle_exclusions"]["units"]
        .as_array()
        .expect("oracle_exclusions.units should be an array");
    let classified: u64 = classifications
        .iter()
        .map(|item| item["count"].as_u64().unwrap_or(0))
        .sum();
    assert_eq!(
        classified,
        units.len() as u64,
        "classification rollups should cover every excluded unit: {report}"
    );
    assert!(
        classifications
            .iter()
            .any(|item| item["exclusion_reason"] == "uninterpretable"
                && item["classification"] == "semantic-boundary-attributed"
                && item["oracle_excluded"].as_u64().unwrap_or(0) >= 5
                && item["attributed_units"].as_u64().unwrap_or(0) >= 5
                && item["unattributed_units"].as_u64().unwrap_or(0) == 0),
        "expected semantic-boundary-attributed oracle exclusions: {report}"
    );
}
