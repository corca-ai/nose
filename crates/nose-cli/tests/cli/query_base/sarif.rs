use super::*;

#[test]
fn query_base_sarif_top_records_truncation() {
    let dir = make_project("query_base_sarif_top");
    add_distinct_clone_family(&dir);
    init_git_repo(&dir);

    let a = dir.join("a/f.py");
    let src = fs::read_to_string(&a).unwrap();
    fs::write(
        &a,
        src.replace(
            "    return total",
            "    total = total + 1\n    return total",
        ),
    )
    .unwrap();
    let fresh = dir.join("new/fresh_a.py");
    let fresh_src = fs::read_to_string(&fresh).unwrap();
    fs::write(
        &fresh,
        fresh_src.replace(
            "    return total",
            "    total = total + 2\n    return total",
        ),
    )
    .unwrap();

    assert_sarif_truncated(
        nose_query_base(&dir, &["--format", "sarif", "top=1"]),
        "query base",
        "top=0",
    );
    assert_sarif_truncated(
        nose_query_in(&dir, &["base=main", "--format", "sarif", "top=1"]),
        "query base",
        "top=0",
    );
    assert_query_base_json_truncated(nose_query_base(&dir, &["--format", "json", "top=1"]));
    assert_query_base_json_truncated(nose_query_in(
        &dir,
        &["base=main", "--format", "json", "top=1"],
    ));
    assert_sarif_complete(nose_query_base(&dir, &["--format", "sarif", "top=0"]));
    assert_query_base_json_complete(nose_query_base(&dir, &["--format", "json", "top=0"]));

    let _ = fs::remove_dir_all(&dir);
}

fn assert_sarif_truncated(out: std::process::Output, label: &str, top_zero_hint: &str) {
    assert!(
        out.status.success(),
        "{label} should emit SARIF: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("query base SARIF JSON");
    let run = &doc["runs"][0];
    assert_eq!(
        run["results"].as_array().map(Vec::len),
        Some(1),
        "{label} --top 1 emits one result: {doc}"
    );
    let props = &run["properties"];
    assert!(
        props["total_families"].as_u64().unwrap_or(0) >= 2,
        "{label} records the full divergent-family count: {doc}"
    );
    assert_eq!(props["shown_families"], 1, "{label} records shown count");
    assert!(
        run["invocations"][0]["toolExecutionNotifications"][0]["message"]["text"]
            .as_str()
            .is_some_and(|m| m.contains(top_zero_hint)),
        "{label} explains how to emit the full SARIF set: {doc}"
    );
}

fn assert_query_base_json_truncated(out: std::process::Output) {
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    assert_eq!(
        doc["items"].as_array().map(Vec::len),
        Some(1),
        "query base JSON top=1 emits one item: {doc}"
    );
    assert!(
        doc["summary"]["divergences"].as_u64().unwrap_or(0) >= 2,
        "query base JSON records the total before truncation: {doc}"
    );
    assert_eq!(doc["summary"]["shown_divergences"], 1);
    assert_eq!(doc["summary"]["limit"], 1);
}

fn assert_sarif_complete(out: std::process::Output) {
    assert!(
        out.status.success(),
        "query base complete SARIF should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("query base SARIF JSON");
    let run = &doc["runs"][0];
    let results = run["results"].as_array().expect("SARIF results");
    let total = run["properties"]["total_families"].as_u64().unwrap();
    assert!(
        total >= 2,
        "fixture should have multiple divergences: {doc}"
    );
    assert_eq!(results.len() as u64, total, "top=0 emits all results");
    assert_eq!(run["properties"]["shown_families"], total);
    assert!(
        run["invocations"].is_null(),
        "top=0 complete SARIF should not include truncation note: {doc}"
    );
}

fn assert_query_base_json_complete(out: std::process::Output) {
    assert!(
        out.status.success(),
        "query base complete JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let items = doc["items"].as_array().expect("query base items");
    let total = doc["summary"]["divergences"].as_u64().unwrap();
    assert!(
        total >= 2,
        "fixture should have multiple divergences: {doc}"
    );
    assert_eq!(items.len() as u64, total, "top=0 emits all JSON items");
    assert_eq!(doc["summary"]["shown_divergences"], total);
    assert_eq!(doc["summary"]["limit"], serde_json::Value::Null);
}
