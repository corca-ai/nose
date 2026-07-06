use super::*;

fn strict_base_project(tag: &str) -> PathBuf {
    let dir = make_project(tag);
    fs::remove_dir_all(dir.join("tests")).unwrap();
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
    dir
}

fn write_ignore(dir: &Path, body: &str) {
    fs::write(dir.join("nose.ignore.json"), body).unwrap();
}

fn query_base_json(dir: &Path) -> (std::process::Output, serde_json::Value) {
    let out = nose_query_base(dir, &["--format", "json"]);
    let json = serde_json::from_slice(&out.stdout).unwrap_or_else(|err| {
        panic!(
            "query base JSON parse failed: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out, json)
}

fn query_base_sarif(dir: &Path) -> (std::process::Output, serde_json::Value) {
    let out = nose_query_base(dir, &["--format", "sarif"]);
    let json = serde_json::from_slice(&out.stdout).unwrap_or_else(|err| {
        panic!(
            "query base SARIF parse failed: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out, json)
}

#[test]
fn query_base_honors_config_relative_ignore_file() {
    let dir = strict_base_project("query_base_config_ignore");
    fs::create_dir_all(dir.join("suppressions")).unwrap();
    fs::write(
        dir.join("suppressions/nose.ignore.json"),
        "{\"ignores\":[{\"paths\":[\"a/**\",\"b/**\"],\"reason\":\"accepted-divergence\"}]}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nose.toml"),
        "[query]\nignore-file = \"suppressions/nose.ignore.json\"\n",
    )
    .unwrap();

    let (out, json) = query_base_json(&dir);
    assert!(
        out.status.success(),
        "config ignore query should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        json["items"].as_array().expect("items").len(),
        0,
        "config ignore suppresses base findings: {json}"
    );
    assert_eq!(json["summary"]["divergences"], 0);

    let (sarif_out, sarif) = query_base_sarif(&dir);
    assert!(
        sarif_out.status.success(),
        "config ignore SARIF should succeed: {}",
        String::from_utf8_lossy(&sarif_out.stderr)
    );
    assert_eq!(
        sarif["runs"][0]["results"]
            .as_array()
            .expect("SARIF results")
            .len(),
        0,
        "config ignore suppresses SARIF findings: {sarif}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "config ignore suppresses before default gate evaluation"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_partial_path_ignore_does_not_hide_uncovered_members() {
    let dir = strict_base_project("query_base_partial_path_ignore");
    write_ignore(
        &dir,
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"reason\":\"vendor-copy\"}]}\n",
    );

    let (out, json) = query_base_json(&dir);
    assert!(
        out.status.success(),
        "partial ignore query should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let item = json["items"]
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or_else(|| panic!("partial ignore must leave a finding: {json}"));
    assert_eq!(item["tier"], "strict", "uncovered member remains strict");
    assert_eq!(item["gate"]["fail_default"], true);

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "partial path ignores must not hide strict base divergences"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_expired_ignore_warns_and_does_not_suppress() {
    let dir = strict_base_project("query_base_expired_ignore");
    write_ignore(
        &dir,
        "{\"ignores\":[{\"paths\":[\"a/**\",\"b/**\"],\"reason\":\"temporary-waiver\",\"owner\":\"platform\",\"expires_at\":\"2000-01-01\"}]}\n",
    );

    let (out, json) = query_base_json(&dir);
    assert!(
        out.status.success(),
        "expired ignore query should still succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !json["items"].as_array().expect("items").is_empty(),
        "expired ignore does not suppress: {json}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("expired on 2000-01-01") && stderr.contains("not applied"),
        "expired ignore warning: {stderr}"
    );

    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "expired ignore must not suppress the strict default gate"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_malformed_ignore_fails_hard() {
    let dir = strict_base_project("query_base_bad_ignore");
    write_ignore(
        &dir,
        "{\"ignores\":[{\"paths\":[\"a/**\",\"b/**\"],\"note\":\"missing reason\"}]}\n",
    );

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(!out.status.success(), "malformed ignore must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("parsing ignore file") || stderr.contains("validating ignore file"),
        "error should name the ignore problem: {stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_malformed_ignore_fails_even_when_diff_is_empty() {
    let dir = make_project("query_base_bad_ignore_empty");
    init_git_repo(&dir);
    write_ignore(
        &dir,
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"note\":\"missing reason\"}]}\n",
    );

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        !out.status.success(),
        "malformed ignore files should fail before empty-diff short-circuiting"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("parsing ignore file") || stderr.contains("validating ignore file"),
        "error should name the ignore problem: {stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}
