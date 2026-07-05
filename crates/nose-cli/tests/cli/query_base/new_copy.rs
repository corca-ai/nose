use super::*;

fn added_clone_body(name: &str, acc: &str, it: &str) -> String {
    format!(
        "def {name}(items):\n    {acc} = 0\n    for {it} in items:\n        if {it} > 0:\n            {acc} = {acc} + {it} * {it}\n    return {acc}\n"
    )
}

fn new_copy_finding(json: &serde_json::Value) -> &serde_json::Value {
    json["items"]
        .as_array()
        .unwrap_or_else(|| panic!("items should be an array: {json}"))
        .iter()
        .find(|item| item["lane"] == "new-copy")
        .unwrap_or_else(|| panic!("expected new-copy finding: {json}"))
}

#[test]
fn query_base_added_clone_is_report_only_new_copy_lane() {
    let dir = make_temp_dir("query_base_added_clone");
    write_files(
        &dir,
        &[(
            "src/original.py",
            &added_clone_body("original", "total", "x"),
        )],
    );
    init_git_repo(&dir);

    write_files(
        &dir,
        &[("src/new_copy.py", &added_clone_body("new_copy", "acc", "v"))],
    );
    git_in(&dir, &["add", "src/new_copy.py"]);

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = new_copy_finding(&json);
    assert_eq!(
        finding["tier"], "report-only",
        "new-copy is advisory: {json}"
    );
    assert_eq!(
        finding["base_family_id"],
        serde_json::Value::Null,
        "new-copy has no base family id: {json}"
    );
    assert_eq!(
        finding["fire_eligible"], false,
        "new-copy does not reuse the legacy fire gate: {json}"
    );
    assert_eq!(
        finding["gate"]["fail_default"], false,
        "new-copy must not fail default CI: {json}"
    );
    assert!(
        finding["tier_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason == "new_copy_no_base_member"),
        "new-copy explains the non-fire reason: {json}"
    );
    let current_only = finding["current_only"]
        .as_array()
        .expect("new-copy current_only sites");
    assert!(
        current_only
            .iter()
            .all(|site| site["tree"].as_str() == Some("current")),
        "new-copy sites are current-tree coordinates: {json}"
    );
    assert!(
        finding.to_string().contains("src/new_copy.py")
            && finding.to_string().contains("src/original.py"),
        "new-copy includes the added copy and sibling: {json}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must stay quiet for report-only new-copy evidence"
    );

    let sarif = nose_query_base(&dir, &["--format", "sarif"]);
    assert!(
        sarif.status.success(),
        "query base SARIF should succeed: {}",
        String::from_utf8_lossy(&sarif.stderr)
    );
    let sarif_json: serde_json::Value =
        serde_json::from_slice(&sarif.stdout).expect("query base SARIF");
    let result = sarif_json["runs"][0]["results"]
        .as_array()
        .and_then(|results| {
            results
                .iter()
                .find(|result| result["properties"]["lane"] == "new-copy")
        })
        .unwrap_or_else(|| panic!("new-copy SARIF result: {sarif_json}"));
    assert_eq!(result["ruleId"], "nose.divergent.report-only");
    assert_eq!(result["level"], "note");
    assert!(
        result["message"]["text"]
            .as_str()
            .is_some_and(|message| message.starts_with("Report-only new-copy evidence:")),
        "new-copy SARIF message names the report-only lane: {sarif_json}"
    );
    assert_eq!(result["properties"]["gate"]["fail_default"], false);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_moved_clone_is_report_only_new_copy_lane() {
    let dir = make_temp_dir("query_base_moved_clone");
    write_files(
        &dir,
        &[
            ("src/original.py", &added_clone_body("original", "total", "x")),
            (
                "scratch/template.py",
                "def template(items):\n    seen = []\n    for item in items:\n        seen.append(str(item))\n    return ','.join(seen)\n",
            ),
        ],
    );
    init_git_repo(&dir);

    fs::create_dir_all(dir.join("src")).unwrap();
    git_in(&dir, &["mv", "scratch/template.py", "src/moved_clone.py"]);
    fs::write(
        dir.join("src/moved_clone.py"),
        added_clone_body("moved_clone", "acc", "v"),
    )
    .unwrap();

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = new_copy_finding(&json);
    assert_eq!(
        finding["tier"], "report-only",
        "moved current-tree copy is advisory: {json}"
    );
    assert!(
        finding.to_string().contains("src/moved_clone.py")
            && finding.to_string().contains("src/original.py"),
        "moved-copy report uses clone evidence, not path similarity alone: {json}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must stay quiet for moved new-copy evidence"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_new_copy_pathspec_is_relative_from_subdir() {
    let root = make_temp_dir("query_base_new_copy_subdir");
    write_files(
        &root,
        &[(
            "sub/src/original.py",
            &added_clone_body("original", "total", "x"),
        )],
    );
    init_git_repo(&root);
    write_files(
        &root,
        &[(
            "sub/src/new_copy.py",
            &added_clone_body("new_copy", "acc", "v"),
        )],
    );
    git_in(&root, &["add", "sub/src/new_copy.py"]);

    let out = Command::new(bin())
        .current_dir(root.join("sub"))
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args([
            "query",
            ".",
            "base=main",
            "--min-size",
            "8",
            "--format",
            "json",
        ])
        .output()
        .expect("run nose query base from subdir");
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = new_copy_finding(&json);
    assert_eq!(
        finding["lane"], "new-copy",
        "subdir pathspec should still find current-tree copies: {json}"
    );
    assert!(
        finding.to_string().contains("sub/src/new_copy.py")
            && finding.to_string().contains("sub/src/original.py"),
        "locations stay repo-relative from nested cwd: {json}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn query_base_new_copy_handles_paths_with_spaces() {
    let dir = make_temp_dir("query_base_new_copy_spaces");
    write_files(
        &dir,
        &[(
            "src dir/original copy.py",
            &added_clone_body("original", "total", "x"),
        )],
    );
    init_git_repo(&dir);
    write_files(
        &dir,
        &[(
            "src dir/new copy.py",
            &added_clone_body("new_copy", "acc", "v"),
        )],
    );
    git_in(&dir, &["add", "src dir/new copy.py"]);

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = new_copy_finding(&json);
    assert_eq!(
        finding["lane"], "new-copy",
        "space-containing source paths trigger the lane: {json}"
    );
    assert!(
        finding.to_string().contains("src dir/new copy.py")
            && finding.to_string().contains("src dir/original copy.py"),
        "locations preserve path spaces: {json}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must stay quiet for report-only new-copy evidence"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_new_copy_uses_supported_language_extensions() {
    let dir = make_temp_dir("query_base_new_copy_swift");
    let body = |name: &str, acc: &str, item: &str| {
        format!(
            "func {name}(_ items: [Int]) -> Int {{\n    var {acc} = 0\n    for {item} in items {{\n        if {item} > 0 {{\n            {acc} = {acc} + {item} * {item}\n        }}\n    }}\n    return {acc}\n}}\n"
        )
    };
    write_files(
        &dir,
        &[("src/original.swift", &body("original", "total", "x"))],
    );
    init_git_repo(&dir);
    write_files(
        &dir,
        &[("src/new_copy.swift", &body("newCopy", "acc", "v"))],
    );
    git_in(&dir, &["add", "src/new_copy.swift"]);

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = new_copy_finding(&json);
    assert_eq!(
        finding["lane"], "new-copy",
        "supported non-Python extensions trigger the lane: {json}"
    );
    assert!(
        finding.to_string().contains("src/new_copy.swift")
            && finding.to_string().contains("src/original.swift"),
        "Swift clone sites are reported: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_unrelated_added_code_does_not_emit_new_copy_lane() {
    let dir = make_temp_dir("query_base_unrelated_added");
    write_files(
        &dir,
        &[(
            "src/original.py",
            &added_clone_body("original", "total", "x"),
        )],
    );
    init_git_repo(&dir);

    write_files(
        &dir,
        &[(
            "src/unrelated.py",
            "def unrelated(values):\n    out = []\n    for value in values:\n        out.append(str(value).upper())\n    return '|'.join(out)\n",
        )],
    );
    git_in(&dir, &["add", "src/unrelated.py"]);

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    assert_eq!(
        json["summary"]["divergences"], 0,
        "unrelated added code should stay quiet: {json}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must stay quiet with no new-copy finding"
    );

    let _ = fs::remove_dir_all(&dir);
}
