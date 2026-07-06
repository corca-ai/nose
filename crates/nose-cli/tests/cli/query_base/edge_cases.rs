use super::*;

fn edge_body(name: &str, acc: &str, it: &str) -> String {
    format!(
        "def {name}(items):\n    {acc} = 0\n    for {it} in items:\n        if {it} > 0:\n            {acc} = {acc} + {it} * {it}\n    return {acc}\n"
    )
}

fn edge_project(tag: &str) -> PathBuf {
    let dir = make_temp_dir(tag);
    write_files(
        &dir,
        &[
            ("a/f.py", &edge_body("first", "total", "x")),
            ("b/f.py", &edge_body("second", "acc", "v")),
        ],
    );
    init_git_repo(&dir);
    dir
}

fn query_base_json(dir: &Path) -> serde_json::Value {
    let out = nose_query_base(dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("query base JSON")
}

#[test]
fn query_base_pure_rename_without_content_change_stays_quiet() {
    let dir = edge_project("query_base_pure_rename");
    git_in(&dir, &["mv", "a/f.py", "a/moved.py"]);

    let json = query_base_json(&dir);
    assert_eq!(
        json["summary"]["divergences"], 0,
        "pure moves do not create base or new-copy gate findings: {json}"
    );
    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "pure moves must not trip the default gate"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_deleted_copy_is_strict_base_divergence() {
    let dir = edge_project("query_base_deleted_copy");
    fs::remove_file(dir.join("a/f.py")).unwrap();

    let json = query_base_json(&dir);
    let item = json["items"]
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or_else(|| panic!("deleted copy should leave a base divergence: {json}"));
    assert_eq!(item["lane"], "base-divergence");
    assert_eq!(item["tier"], "strict");
    assert_eq!(item["gate"]["fail_default"], true);
    assert!(
        item["changed"].to_string().contains("a/f.py")
            && item["not_updated"].to_string().contains("b/f.py"),
        "deleted member is the changed side and sibling remains not-updated: {json}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "deleting one prod copy is a strict base divergence under the current policy"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_pure_insertion_after_member_boundary_stays_quiet() {
    let dir = edge_project("query_base_boundary_insertion");
    let a = dir.join("a/f.py");
    let mut src = fs::read_to_string(&a).unwrap();
    src.push_str("\n# trailing file comment outside the member\n");
    fs::write(&a, src).unwrap();

    let json = query_base_json(&dir);
    assert_eq!(
        json["summary"]["divergences"], 0,
        "an insertion after the member span should not mark the member changed: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_pure_insertion_inside_shared_logic_is_strict() {
    let dir = edge_project("query_base_inner_insertion");
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

    let json = query_base_json(&dir);
    let item = json["items"]
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or_else(|| panic!("inner insertion should be a divergence: {json}"));
    assert_eq!(item["tier"], "strict");
    assert_eq!(item["changed"][0]["touches_shared"], true);
    assert_eq!(item["gate"]["fail_default"], true);

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "inserting inside shared logic trips the strict default gate"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_missing_base_ref_fails_clearly() {
    let dir = edge_project("query_base_missing_base_ref");
    let out = Command::new(bin())
        .current_dir(&dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args([
            "query",
            ".",
            "base=HEAD~1",
            "--min-size",
            "8",
            "--format",
            "json",
        ])
        .output()
        .expect("run query base with missing ref");
    assert!(!out.status.success(), "missing base ref should fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("`git diff HEAD~1` failed") || stderr.contains("unknown revision"),
        "missing base-ref error should be actionable: {stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}
