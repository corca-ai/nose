use super::*;

#[test]
fn verify_json_exposes_deterministic_soundness_cohort_fields() {
    let project = TempProject::new("verify-json-soundness-cohort");
    project.write("sample.py", "def increment(x):\n    return x + 1\n");
    let output = run_raw(&["verify", project.path().to_str().unwrap(), "--json"]);
    let report: serde_json::Value = serde_json::from_str(&output).expect("verify JSON");
    let unit = report["units"].as_array().unwrap().first().unwrap();

    assert!(unit["claimable"].is_boolean());
    assert!(unit["canon_exposed"].is_boolean());
    assert!(unit["symbolic"].is_boolean());
    assert_eq!(unit["domain_signature"].as_str().unwrap().len(), 16);
    assert!(!unit["value_fingerprint"].as_array().unwrap().is_empty());
    assert!(!unit["constructs"].as_array().unwrap().is_empty());
}

fn await_oracle_exclusion_report(project_name: &str) -> serde_json::Value {
    let project = TempProject::new(project_name);
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
    serde_json::from_str(&fs::read_to_string(report_path).expect("recall-loss report"))
        .expect("recall-loss report JSON")
}

#[path = "commands/baseline.rs"]
mod baseline;
#[path = "commands/capabilities_robustness.rs"]
mod capabilities_robustness;
#[path = "commands/config_packs.rs"]
mod config_packs;
#[path = "commands/ignores_sarif.rs"]
mod ignores_sarif;
#[path = "commands/proposal_query.rs"]
mod proposal_query;
#[path = "commands/query_reinvented.rs"]
mod query_reinvented;
#[path = "commands/query_roots.rs"]
mod query_roots;
#[path = "commands/recall_loss_report.rs"]
mod recall_loss_report;
#[path = "commands/recall_loss_report/java_completable_future.rs"]
mod recall_loss_report_java_completable_future;
#[path = "commands/recall_loss_report/oracle_exclusions/classification.rs"]
mod recall_loss_report_oracle_exclusion_classification;
#[path = "commands/recall_loss_report/oracle_exclusions.rs"]
mod recall_loss_report_oracle_exclusions;
#[path = "commands/recall_loss_report/promise_continuations.rs"]
mod recall_loss_report_promise_continuations;
#[path = "commands/semantic_pack_adoption_gates.rs"]
mod semantic_pack_adoption_gates;
#[path = "commands/semantic_pack_compatibility.rs"]
mod semantic_pack_compatibility;
#[path = "commands/semantic_pack_inventory.rs"]
mod semantic_pack_inventory;
