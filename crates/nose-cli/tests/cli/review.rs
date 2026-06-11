use super::*;

fn git_in(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Turn a fixture dir into a committed git repo.
fn init_git_repo(dir: &Path) {
    git_in(dir, &["init", "-q", "-b", "main"]);
    git_in(dir, &["config", "user.email", "t@example.com"]);
    git_in(dir, &["config", "user.name", "Test"]);
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", "init"]);
}

fn nose_review(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["review", ".", "--min-size", "8"];
    args.extend_from_slice(extra);
    Command::new(bin())
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(&args)
        .output()
        .expect("run nose review")
}

#[test]
fn review_flags_a_clone_changed_in_one_copy_only() {
    let dir = make_project("review_flag");
    init_git_repo(&dir);

    // Edit ONE copy of the clone family (a/f.py) — a fix not propagated to b/f.py.
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

    let out = nose_review(&dir, &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("changed inconsistently"),
        "should flag the inconsistently-changed clone: {stdout}"
    );
    assert!(
        stdout.contains("a/f.py"),
        "names the changed copy: {stdout}"
    );
    assert!(
        stdout.contains("b/f.py"),
        "lists the un-updated sibling: {stdout}"
    );

    // --fail turns it into a non-zero CI gate.
    let gated = nose_review(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "--fail should exit non-zero when flagged"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_json_includes_fragment_context() {
    let dir = make_fragment_project("review_json");
    init_git_repo(&dir);

    let a = dir.join("a/f.py");
    let src = fs::read_to_string(&a).unwrap();
    fs::write(&a, src.replace("return xs[0] + 1", "return xs[0] + 2")).unwrap();

    let out = nose_review(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "review JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("review JSON");
    let finding = json["findings"]
        .as_array()
        .and_then(|findings| findings.first())
        .expect("one fragment review finding");
    for key in ["changed", "not_updated"] {
        let site = finding[key]
            .as_array()
            .and_then(|sites| sites.first())
            .unwrap_or_else(|| panic!("{key} should contain a site: {finding}"));
        assert_eq!(site["is_fragment"], true);
        assert_eq!(site["fragment_kind"], "conditional-guard");
        assert_eq!(site["reason_code"], "exact-conditional-guard");
        assert_eq!(site["span_lines"], 2);
        assert_eq!(site["enclosing_unit"]["kind"], "Function");
        assert!(site["enclosing_unit"]["unit_key"]
            .as_str()
            .is_some_and(|key| key.contains(":Function:1-5:")));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_is_quiet_when_a_clone_changes_consistently() {
    let dir = make_project("review_consistent");
    init_git_repo(&dir);

    // Apply the *same* edit to every copy — a consistent change, nothing to flag.
    for sub in ["a", "b", "tests"] {
        let f = dir.join(sub).join("f.py");
        let src = fs::read_to_string(&f).unwrap();
        fs::write(&f, src.replace("    return", "    pass\n    return")).unwrap();
    }

    let out = nose_review(&dir, &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("not updated"),
        "a consistent change must not be flagged: {stdout}"
    );
    assert!(
        out.status.success(),
        "no --fail trip on a consistent change"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_needs_a_git_repository() {
    let dir = make_project("review_nogit");
    let out = Command::new(bin())
        .current_dir(&dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(["review", "."])
        .output()
        .expect("run nose review");
    assert!(!out.status.success(), "review must fail outside a git repo");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("git repository"),
        "explains the git requirement: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_respects_structured_ignores() {
    let dir = make_project("review_ignore");
    init_git_repo(&dir);

    // Edit one copy so a family is flagged.
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

    // Grab the stable family_id from JSON, then suppress it.
    let json_out = nose_review(&dir, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&json_out.stdout).expect("review JSON");
    let findings = json["findings"].as_array().expect("findings");
    assert!(!findings.is_empty(), "expected a flagged family first");
    let fid = findings[0]["family_id"].as_str().unwrap();

    let ignore = dir.join("nose.ignore.json");
    fs::write(
        &ignore,
        format!(r#"{{"ignores":[{{"family_id":"{fid}","reason":"intentional"}}]}}"#),
    )
    .unwrap();

    let out = nose_review(&dir, &["--fail"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("not updated"),
        "the ignored family must be suppressed: {stdout}"
    );
    assert!(
        out.status.success(),
        "a fully-suppressed review must not trip --fail"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_on_new_requires_a_baseline() {
    let dir = make_project("fail_on_new_nobaseline");
    let p = dir.to_str().unwrap();
    // `--fail-on new` compares against a baseline; with no --baseline the gate can never
    // fire, so it must error rather than silently pass (a CI gate that always passes).
    let out = Command::new(bin())
        .args(["scan", p, "--min-size", "12", "--fail-on", "new"])
        .output()
        .expect("run scan");
    assert!(
        !out.status.success(),
        "`--fail-on new` without --baseline must error, not silently pass: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn c_u16_byte_pack_recognized_in_either_operand_order() {
    // The byte-pack idiom must be recognized whichever way its commutative operands sort by
    // value-hash. With the base at param 1 the shifted lane sorts second; a `+` form and a
    // `|` form then cluster into one Type-4 family only if both normalize to the byte-pack op.
    let dir = std::env::temp_dir().join(format!("nose_bytepack_order_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("add2.c"),
        "unsigned int add2(int d, const unsigned char *a) {\n  return (a[0] << 8) + a[1];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("or2.c"),
        "unsigned int or2(int d, unsigned char *a) {\n  return (a[0] << 8) | a[1];\n}\n",
    )
    .unwrap();
    let out = scan_min_json(&dir, "semantic");
    let json = scan_json(&out);
    let families = scan_families(&json);
    assert_eq!(
        families.len(),
        1,
        "byte-pack must be recognized in either operand order (+ and | should cluster): {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn review_machine_formats_emit_json_when_nothing_to_review() {
    let dir = make_project("review_empty_json");
    init_git_repo(&dir);
    // No working-tree changes vs HEAD: review has nothing to flag, but the
    // machine formats must still print their contract, not a human sentence.
    let out = nose_review(&dir, &["--format", "json"]);
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("--format json must emit JSON even with no reviewable changes");
    assert_eq!(json["inconsistent_families"], 0);
    assert_eq!(json["findings"].as_array().map(Vec::len), Some(0));
    assert_eq!(json["changed_files"], 0);

    let sarif = nose_review(&dir, &["--format", "sarif"]);
    assert!(sarif.status.success());
    let doc: serde_json::Value = serde_json::from_slice(&sarif.stdout)
        .expect("--format sarif must emit JSON even with no reviewable changes");
    assert!(doc["runs"].is_array(), "sarif keeps its runs envelope");
    let _ = fs::remove_dir_all(&dir);
}

/// #245 — the conservative `--fail` gate: a change INSIDE a member's varying
/// spot (the part that already differed from the sibling) is not a propagation
/// hazard and must not fire; a change to SHARED lines must. `--fail-on any`
/// restores span-overlap firing.
#[test]
fn review_fail_fires_on_shared_logic_only() {
    let body = |tag: &str| {
        format!(
            "def process(items, flag):\n    out = []\n    for item in items:\n        if item > 0:\n            out.append(item * 2 + 1)\n    log_result(out, \"{tag}\")\n    return out\n"
        )
    };
    let dir = std::env::temp_dir().join(format!("nose_fire_policy_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(dir.join("a/f.py"), body("alpha")).unwrap();
    fs::write(dir.join("b/f.py"), body("beta")).unwrap();
    init_git_repo(&dir);

    let review = |dir: &Path, extra: &[&str]| {
        let mut args = vec![
            "review",
            ".",
            "--min-size",
            "8",
            "--mode",
            "syntax,semantic,near",
        ];
        args.extend_from_slice(extra);
        Command::new(bin())
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .env_remove("GIT_COMMON_DIR")
            .args(&args)
            .output()
            .expect("run nose review")
    };

    // Scenario 1: edit only the varying spot ("alpha" → "gamma") — the line that
    // already differed from the sibling. Flagged for review, but the gate stays quiet.
    fs::write(dir.join("a/f.py"), body("gamma")).unwrap();
    let out = review(&dir, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("review JSON");
    let finding = json["findings"]
        .as_array()
        .and_then(|f| f.first())
        .expect("the divergence is still flagged for review");
    assert_eq!(
        finding["fire_eligible"], false,
        "a varying-spot-only change must not be gate-eligible: {json}"
    );
    let gated = review(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must not fire on a varying-spot-only change"
    );
    let any = review(&dir, &["--fail", "--fail-on", "any"]);
    assert!(
        !any.status.success(),
        "--fail-on any restores span-overlap firing"
    );

    // Scenario 2: edit a SHARED line (the computation both copies carry).
    fs::write(
        dir.join("a/f.py"),
        body("alpha").replace("item * 2 + 1", "item * 2 + 3"),
    )
    .unwrap();
    let out = review(&dir, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("review JSON");
    let finding = json["findings"]
        .as_array()
        .and_then(|f| f.first())
        .expect("the shared-line divergence is flagged");
    assert_eq!(
        finding["fire_eligible"], true,
        "a shared-line change is gate-eligible: {json}"
    );
    assert_eq!(
        finding["changed"][0]["touches_shared"], true,
        "the changed site carries the per-site verdict: {json}"
    );
    let gated = review(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "--fail fires when the change touches shared lines"
    );
    let _ = fs::remove_dir_all(&dir);
}
