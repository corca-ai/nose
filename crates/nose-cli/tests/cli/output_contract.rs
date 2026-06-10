use super::*;

#[test]
fn checked_in_scan_json_v1_example_matches_contract() {
    let json = scan_json(include_str!("../fixtures/scan-json-v1.json"));
    assert_scan_json_v1_contract(&json);
}

#[test]
fn scan_json_report_has_versioned_contract() {
    let dir = make_project("json_contract");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    let json = scan_json(&out);
    assert_scan_json_v1_contract(&json);
    assert_eq!(json["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["scope"]["files"], 4);
    assert_eq!(json["scope"]["languages"][0]["language"], "python");
    assert_eq!(json["ranking"]["sort"], "extractability");
    assert_eq!(json["ranking"]["shown_families"], 1);
    assert_eq!(json["ranking"]["limit"], 1);
    assert_eq!(json["ranking"]["surface_counts"]["default"], 1);
    assert_eq!(json["ranking"]["surface_counts"]["review"], 0);
    assert_eq!(json["ranking"]["surface_counts"]["hidden"], 0);
    assert_eq!(json["ranking"]["surface_counts"]["fragments"]["total"], 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_json_exposes_exact_fragment_metadata() {
    let dir = make_fragment_project("json");

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
    ]);
    let json = scan_json(&out);
    assert_eq!(json["ranking"]["surface_counts"]["default"], 0);
    assert_eq!(json["ranking"]["surface_counts"]["hidden"], 1);
    assert_eq!(json["ranking"]["surface_counts"]["fragments"]["total"], 1);
    assert_eq!(json["ranking"]["surface_counts"]["fragments"]["hidden"], 1);
    let family = scan_families(&json)
        .iter()
        .find(|family| family["recommended_surface"] == "hidden")
        .expect("tiny exact fragment family should be present");
    assert_eq!(family["mean_lines"], 2);
    let locs = family["locations"].as_array().expect("locations");
    assert_eq!(locs.len(), 2);
    for loc in locs {
        assert_eq!(loc["kind"], "Block");
        assert_eq!(loc["span_lines"], 2);
        assert!(loc["span_tokens"].as_u64().is_some_and(|n| n > 0));
        assert_eq!(loc["is_fragment"], true);
        assert_eq!(loc["fragment_kind"], "conditional-guard");
        assert_eq!(loc["reason_code"], "exact-conditional-guard");
        assert!(
            loc.get("proof_facts").is_none(),
            "proof facts must not be stable scan JSON: {loc}"
        );
        let parent = &loc["enclosing_unit"];
        assert_eq!(parent["kind"], "Function");
        assert!(parent["name"]
            .as_str()
            .is_some_and(|name| { name == "first" || name == "second" }));
        assert!(parent["unit_key"]
            .as_str()
            .is_some_and(|key| { key.contains(":Function:1-5:") }));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_human_hides_hidden_exact_fragments() {
    let dir = make_fragment_project("human_hidden");

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
    ]);
    assert!(
        out.contains("0 semantic clone families"),
        "hidden proof fragments should not be top-level human findings: {out}"
    );
    assert!(
        out.contains("omitted 1 hidden proof-only family"),
        "human report should explain the omitted diagnostic family: {out}"
    );
    assert!(
        !out.contains("a/f.py") && !out.contains("b/f.py"),
        "hidden proof fragments must not expose report locations: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_on_any_ignores_hidden_exact_fragments() {
    let dir = make_fragment_project("fail_hidden");

    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--fail-on",
            "any",
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "hidden proof-only fragments should not trip the default CI gate: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}
