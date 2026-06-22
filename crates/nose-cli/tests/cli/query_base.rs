use super::*;

#[path = "query_base/sarif.rs"]
mod sarif;
#[path = "query_base/semantic_packs.rs"]
mod semantic_packs;

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

fn nose_query_in(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["query", "."];
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
        .expect("run nose query")
}

fn nose_query_base(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["query", ".", "base=HEAD", "--min-size", "8"];
    for arg in extra {
        if *arg == "--fail" {
            args.extend_from_slice(&["--fail-on", "any"]);
        } else {
            args.push(arg);
        }
    }
    Command::new(bin())
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(&args)
        .output()
        .expect("run nose query base")
}

#[test]
fn query_base_flags_divergent_edits() {
    // `nose query . base=<ref>` is the query base pipeline surfaced under query: detect at the
    // ref, flag a clone changed in one copy but not its siblings, gate on the proven case.
    let dir = make_project("query_base");
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

    let out = nose_query_in(&dir, &["base=main", "--min-size", "8"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("divergent") && stdout.contains("a/f.py") && stdout.contains("b/f.py"),
        "base= names the changed copy and the un-updated sibling: {stdout}"
    );

    let jout = nose_query_in(&dir, &["base=main", "--min-size", "8", "--format", "json"]);
    let j: serde_json::Value = serde_json::from_slice(&jout.stdout).unwrap();
    assert_eq!(j["view"], "base", "query schema envelope, base view: {j}");
    assert_query_json_reports_semantic_packs(&j);
    assert_eq!(j["base"], "main");
    assert!(
        j["summary"]["divergences"].as_u64().unwrap() >= 1,
        "at least one divergence: {j}"
    );
    assert!(
        j["items"][0]["fire_eligible"].is_boolean(),
        "items carry the §BV fire verdict: {j}"
    );

    let sout = nose_query_in(&dir, &["base=main", "--min-size", "8", "--format", "sarif"]);
    assert!(
        sout.status.success(),
        "base= SARIF should succeed: {}",
        String::from_utf8_lossy(&sout.stderr)
    );
    let sarif: serde_json::Value = serde_json::from_slice(&sout.stdout).expect("query base SARIF");
    assert!(
        sarif["runs"][0]["results"]
            .as_array()
            .is_some_and(|r| !r.is_empty()),
        "query base= SARIF reuses query base findings: {sarif}"
    );
    let unsupported = nose_query_in(&dir, &["base=main", "path~a/f.py", "--min-size", "8"]);
    let stderr = String::from_utf8_lossy(&unsupported.stderr);
    assert!(
        !unsupported.status.success(),
        "base= should reject ignored query filters"
    );
    assert!(
        stderr.contains("combine it only with `top=N`"),
        "base= explains its supported term set: {stderr}"
    );
    for (args, needle, label) in [
        (
            &["base=main", "--min-members", "3"][..],
            "--min-members",
            "ignored query flags",
        ),
        (
            &[
                "base=main",
                "--baseline",
                "accepted.json",
                "--write-baseline",
            ][..],
            "--baseline",
            "baseline writes",
        ),
    ] {
        let out = nose_query_in(&dir, args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "base= should reject {label}");
        assert!(
            stderr.contains(needle),
            "base= names unsupported {label}: {stderr}"
        );
    }

    // `--fail-on any` over base= fires on the conservative (shared-logic) policy.
    let gated = nose_query_in(&dir, &["base=main", "--min-size", "8", "--fail-on", "any"]);
    assert!(
        !gated.status.success(),
        "base= --fail-on any exits non-zero on a proven divergence"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_matches_base_ref_findings() {
    // `base=HEAD` and `base=main` run the same detection against this fixture state, so they
    // report the same findings (family_id + fire verdict) on one diff.
    let dir = make_project("query_base_parity");
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

    let rev: serde_json::Value =
        serde_json::from_slice(&nose_query_base(&dir, &["--format", "json"]).stdout).unwrap();
    let qry: serde_json::Value = serde_json::from_slice(
        &nose_query_in(&dir, &["base=main", "--min-size", "8", "--format", "json"]).stdout,
    )
    .unwrap();

    let key = |v: &serde_json::Value, arr: &str| {
        let mut ks: Vec<(String, bool)> = v[arr]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (
                    f["family_id"].as_str().unwrap().to_string(),
                    f["fire_eligible"].as_bool().unwrap(),
                )
            })
            .collect();
        ks.sort();
        ks
    };
    let rev_keys = key(&rev, "items");
    assert!(!rev_keys.is_empty(), "query base found a divergence: {rev}");
    assert_eq!(
        rev_keys,
        key(&qry, "items"),
        "query base= reports the same family ids + fire verdicts as query base"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_pathspec_is_relative_to_invocation_dir() {
    let root = std::env::temp_dir().join(format!("nose_query_base_subdir_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let sub = root.join("sub");
    let src = sub.join("src");
    fs::create_dir_all(src.join("a")).unwrap();
    fs::create_dir_all(src.join("b")).unwrap();
    let body = "def process(items):\n    total = 0\n    for item in items:\n        total += item * 2\n    return total\n";
    fs::write(src.join("a/f.py"), body).unwrap();
    fs::write(src.join("b/f.py"), body).unwrap();
    init_git_repo(&root);

    fs::write(
        src.join("a/f.py"),
        body.replace(
            "    return total",
            "    total = total + 1\n    return total",
        ),
    )
    .unwrap();

    let out = Command::new(bin())
        .current_dir(&sub)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args([
            "query",
            "src",
            "base=main",
            "--min-size",
            "8",
            "--format",
            "json",
        ])
        .output()
        .expect("run nose query from subdir");
    assert!(
        out.status.success(),
        "query base from subdir should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    assert!(
        json["summary"]["divergences"].as_u64().unwrap_or(0) >= 1,
        "subdir-relative pathspec should find the divergent clone: {json}"
    );
    let rendered = json.to_string();
    assert!(
        rendered.contains("sub/src/a/f.py") && rendered.contains("sub/src/b/f.py"),
        "locations stay repo-relative to the actual analyzed subtree: {rendered}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn query_base_flags_a_clone_changed_in_one_copy_only() {
    let dir = make_project("query_base_flag");
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

    let out = nose_query_base(&dir, &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("divergent"),
        "should flag the divergent clone: {stdout}"
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
    let gated = nose_query_base(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "--fail should exit non-zero when flagged"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_json_includes_fragment_context() {
    let dir = make_fragment_project("query_base_json");
    init_git_repo(&dir);

    let a = dir.join("a/f.py");
    let src = fs::read_to_string(&a).unwrap();
    fs::write(&a, src.replace("return xs[0] + 1", "return xs[0] + 2")).unwrap();

    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = json["items"]
        .as_array()
        .and_then(|findings| findings.first())
        .expect("one fragment query base finding");
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
fn query_base_is_quiet_when_a_clone_changes_consistently() {
    let dir = make_project("query_base_consistent");
    init_git_repo(&dir);

    // Apply the *same* edit to every copy — a consistent change, nothing to flag.
    for sub in ["a", "b", "tests"] {
        let f = dir.join(sub).join("f.py");
        let src = fs::read_to_string(&f).unwrap();
        fs::write(&f, src.replace("    return", "    pass\n    return")).unwrap();
    }

    let out = nose_query_base(&dir, &[]);
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
fn query_base_needs_a_git_repository() {
    let dir = make_project("query_base_nogit");
    let out = Command::new(bin())
        .current_dir(&dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(["query", ".", "base=HEAD"])
        .output()
        .expect("run nose query base");
    assert!(
        !out.status.success(),
        "query base must fail outside a git repo"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("git repository"),
        "explains the git requirement: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_respects_structured_ignores() {
    let dir = make_project("query_base_ignore");
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
    let json_out = nose_query_base(&dir, &["--format", "json"]);
    let json: serde_json::Value =
        serde_json::from_slice(&json_out.stdout).expect("query base JSON");
    let findings = json["items"].as_array().expect("items");
    assert!(!findings.is_empty(), "expected a flagged family first");
    let fid = findings[0]["family_id"].as_str().unwrap();

    let ignore = dir.join("nose.ignore.json");
    fs::write(
        &ignore,
        format!(r#"{{"ignores":[{{"family_id":"{fid}","reason":"intentional"}}]}}"#),
    )
    .unwrap();

    let out = nose_query_base(&dir, &["--fail"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("not updated"),
        "the ignored family must be suppressed: {stdout}"
    );
    assert!(
        out.status.success(),
        "a fully-suppressed query base must not trip --fail"
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
    let out = query_min_json(&dir, "semantic");
    let json = query_json(&out);
    let families = query_families(&json);
    assert_eq!(
        families.len(),
        1,
        "byte-pack must be recognized in either operand order (+ and | should cluster): {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_base_machine_formats_emit_json_when_no_changes_exist() {
    let dir = make_project("query_base_empty_json");
    init_git_repo(&dir);
    // No working-tree changes vs HEAD: query base has nothing to flag, but the
    // machine formats must still print their contract, not a human sentence.
    let out = nose_query_base(&dir, &["--format", "json"]);
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("--format json must emit JSON even with no actionable changes");
    assert_eq!(json["summary"]["divergences"], 0);
    assert_eq!(json["items"].as_array().map(Vec::len), Some(0));
    assert_eq!(json["summary"]["changed_files"], 0);

    let sarif = nose_query_base(&dir, &["--format", "sarif"]);
    assert!(sarif.status.success());
    let doc: serde_json::Value = serde_json::from_slice(&sarif.stdout)
        .expect("--format sarif must emit JSON even with no actionable changes");
    assert!(doc["runs"].is_array(), "sarif keeps its runs envelope");
    let _ = fs::remove_dir_all(&dir);
}

/// #245 — the conservative `--fail` gate: a change INSIDE a member's varying
/// spot (the part that already differed from the sibling) is not a propagation
/// hazard and must not fire; a change to SHARED lines must.
#[test]
fn query_base_fail_fires_on_shared_logic_only() {
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

    let query_base = |dir: &Path, extra: &[&str]| {
        let mut args = vec![
            "query",
            ".",
            "base=HEAD",
            "--min-size",
            "8",
            "--mode",
            "syntax,semantic,near",
        ];
        for arg in extra {
            if *arg == "--fail" {
                args.extend_from_slice(&["--fail-on", "any"]);
            } else {
                args.push(arg);
            }
        }
        Command::new(bin())
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .env_remove("GIT_COMMON_DIR")
            .args(&args)
            .output()
            .expect("run nose query base")
    };

    // Scenario 1: edit only the varying spot ("alpha" → "gamma") — the line that
    // already differed from the sibling. Flagged for query base, but the gate stays quiet.
    fs::write(dir.join("a/f.py"), body("gamma")).unwrap();
    let out = query_base(&dir, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = json["items"]
        .as_array()
        .and_then(|f| f.first())
        .expect("the divergence is still flagged for query base");
    assert_eq!(
        finding["fire_eligible"], false,
        "a varying-spot-only change must not be gate-eligible: {json}"
    );
    let gated = query_base(&dir, &["--fail"]);
    assert!(
        gated.status.success(),
        "--fail must not fire on a varying-spot-only change"
    );
    // Scenario 2: edit a SHARED line (the computation both copies carry).
    fs::write(
        dir.join("a/f.py"),
        body("alpha").replace("item * 2 + 1", "item * 2 + 3"),
    )
    .unwrap();
    let out = query_base(&dir, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base JSON");
    let finding = json["items"]
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
    let gated = query_base(&dir, &["--fail"]);
    assert!(
        !gated.status.success(),
        "--fail fires when the change touches shared lines"
    );
    let _ = fs::remove_dir_all(&dir);
}
