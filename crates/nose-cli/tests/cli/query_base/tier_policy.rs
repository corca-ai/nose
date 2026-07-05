use super::*;

#[test]
fn query_base_mixed_scope_is_report_only_not_strict() {
    let dir = make_project("query_base_mixed_report_only");
    init_git_repo(&dir);

    // The family spans prod copies and a test copy. Editing only the test copy is useful
    // review context, but v2 must not let this mixed/test-scaffolding lane fail default CI.
    let tests = dir.join("tests/f.py");
    let src = fs::read_to_string(&tests).unwrap();
    fs::write(
        &tests,
        src.replace("    return s", "    s = s + 1\n    return s"),
    )
    .unwrap();

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = json["items"]
        .as_array()
        .and_then(|items| items.first())
        .expect("mixed-scope divergent finding");
    assert_eq!(
        finding["scope"], "mixed",
        "fixture should be mixed-scope: {json}"
    );
    assert_eq!(
        finding["fire_eligible"], true,
        "legacy v1 verdict still records the shared-logic touch: {json}"
    );
    assert_eq!(
        finding["tier"], "report-only",
        "mixed scope is not strict: {json}"
    );
    assert_eq!(
        finding["taxonomy_hint"], "test_scaffolding",
        "taxonomy: {json}"
    );
    assert_eq!(
        finding["gate"]["fail_default"], false,
        "report-only never fails: {json}"
    );
    assert!(
        finding["tier_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "test_scope"),
        "report-only finding explains the test-scope reason: {json}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must stay quiet for report-only mixed/test evidence"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_all_test_scope_is_report_only_not_strict() {
    let dir = make_temp_dir("query_base_all_test_report_only");
    let body = |name: &str, acc: &str, it: &str| {
        format!(
            "def {name}(items):\n    {acc} = 0\n    for {it} in items:\n        if {it} > 0:\n            {acc} = {acc} + {it} * {it}\n    return {acc}\n"
        )
    };
    write_files(
        &dir,
        &[
            ("tests/a/f.py", &body("first", "total", "x")),
            ("tests/b/f.py", &body("second", "acc", "v")),
        ],
    );
    init_git_repo(&dir);

    let a = dir.join("tests/a/f.py");
    let src = fs::read_to_string(&a).unwrap();
    fs::write(
        &a,
        src.replace(
            "    return total",
            "    total = total + 1\n    return total",
        ),
    )
    .unwrap();

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = json["items"]
        .as_array()
        .and_then(|items| items.first())
        .expect("all-test divergent finding");
    assert_eq!(
        finding["scope"], "test",
        "fixture should be all-test: {json}"
    );
    assert_eq!(
        finding["fire_eligible"], false,
        "legacy v1 verdict excludes all-test findings: {json}"
    );
    assert_eq!(
        finding["tier"], "report-only",
        "all-test scope stays visible but non-strict: {json}"
    );
    assert_eq!(
        finding["gate"]["fail_default"], false,
        "all-test report-only finding does not fail: {json}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must stay quiet for all-test evidence"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_exact_renamed_twin_remains_strict() {
    let dir = make_mode_project("query_base_exact_strict");
    init_git_repo(&dir);

    let a = dir.join("renamed_a.py");
    let src = fs::read_to_string(&a).unwrap();
    fs::write(
        &a,
        src.replace(
            "total = total + item * item",
            "total = total + item * item + 1",
        ),
    )
    .unwrap();

    let out = nose_query_base(&dir, &["--mode", "semantic", "--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["changed"].to_string().contains("renamed_a.py")
                && item["not_updated"].to_string().contains("renamed_b.py")
        })
        .unwrap_or_else(|| panic!("expected renamed-twin divergence: {json}"));
    assert_eq!(
        finding["witness_kind"], "exact-value-graph",
        "exact witness: {json}"
    );
    assert_eq!(
        finding["fire_eligible"], true,
        "exact twin touches shared logic: {json}"
    );
    assert_eq!(
        finding["tier"], "strict",
        "exact renamed twin stays strict: {json}"
    );
    assert_eq!(finding["gate"]["fail_default"], true, "strict gate: {json}");

    let gated = nose_query_base(&dir, &["--mode", "semantic", "--fail"]);
    assert!(
        !gated.status.success(),
        "--fail must fire for an exact renamed-twin strict divergence"
    );

    let _ = fs::remove_dir_all(&dir);
}

fn fire_policy_body(tag: &str) -> String {
    format!(
        "def process(items, flag):\n    out = []\n    for item in items:\n        if item > 0:\n            out.append(item * 2 + 1)\n    log_result(out, \"{tag}\")\n    return out\n"
    )
}

fn make_fire_policy_project() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_fire_policy_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(dir.join("a/f.py"), fire_policy_body("alpha")).unwrap();
    fs::write(dir.join("b/f.py"), fire_policy_body("beta")).unwrap();
    init_git_repo(&dir);
    dir
}

fn fire_policy_query_base(dir: &Path, extra: &[&str]) -> std::process::Output {
    nose_query_base_with_mode(dir, Some("syntax,semantic,near"), extra)
}

fn first_fire_policy_finding(dir: &Path) -> serde_json::Value {
    let out = fire_policy_query_base(dir, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    json["items"]
        .as_array()
        .and_then(|f| f.first())
        .expect("the divergence is flagged for query base")
        .clone()
}

fn assert_varying_spot_is_review(dir: &Path) {
    let finding = first_fire_policy_finding(dir);
    assert_eq!(
        finding["fire_eligible"], false,
        "a varying-spot-only change must not be gate-eligible: {finding}"
    );
    assert_eq!(
        finding["tier"], "review",
        "a varying-spot-only change remains visible for review: {finding}"
    );
    assert_eq!(
        finding["taxonomy_hint"], "no_propagation_needed",
        "v2 explains the non-strict taxonomy: {finding}"
    );
    assert_eq!(
        finding["gate"]["fail_default"], false,
        "review findings do not fail default CI: {finding}"
    );
    let gated = fire_policy_query_base(dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must not fire on a varying-spot-only change"
    );
}

fn assert_shared_logic_is_strict(dir: &Path) {
    let finding = first_fire_policy_finding(dir);
    assert_eq!(
        finding["fire_eligible"], true,
        "a shared-line change is gate-eligible: {finding}"
    );
    assert_eq!(
        finding["tier"], "strict",
        "a prod shared-line change is strict: {finding}"
    );
    assert_eq!(
        finding["taxonomy_hint"], "missed_propagation",
        "strict findings carry the missed-propagation taxonomy: {finding}"
    );
    assert_eq!(
        finding["gate"]["fail_default"], true,
        "strict findings fail default CI: {finding}"
    );
    assert_eq!(
        finding["changed"][0]["touches_shared"], true,
        "the changed site carries the per-site verdict: {finding}"
    );
    let gated = fire_policy_query_base(dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "--fail fires when the change touches shared lines"
    );
}

/// #245 — the conservative `--fail` gate: varying-spot edits stay review-only,
/// while shared-line edits become strict.
#[test]
fn query_base_fail_fires_on_shared_logic_only() {
    let dir = make_fire_policy_project();

    fs::write(dir.join("a/f.py"), fire_policy_body("gamma")).unwrap();
    assert_varying_spot_is_review(&dir);

    fs::write(
        dir.join("a/f.py"),
        fire_policy_body("alpha").replace("item * 2 + 1", "item * 2 + 3"),
    )
    .unwrap();
    assert_shared_logic_is_strict(&dir);

    let _ = fs::remove_dir_all(&dir);
}
