use super::*;

#[path = "query_base/core_contract.rs"]
mod core_contract;
#[path = "query_base/edge_cases.rs"]
mod edge_cases;
#[path = "query_base/new_copy.rs"]
mod new_copy;
#[path = "query_base/region_matches.rs"]
mod region_matches;
#[path = "query_base/sarif.rs"]
mod sarif;
#[path = "query_base/semantic_packs.rs"]
mod semantic_packs;
#[path = "query_base/suppression_edges.rs"]
mod suppression_edges;
#[path = "query_base/tier_policy.rs"]
mod tier_policy;
#[path = "query_base/variant_evidence.rs"]
mod variant_evidence;

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

struct GitProject {
    project: TempProject,
}

impl GitProject {
    fn new(tag: &str) -> Self {
        Self {
            project: TempProject::new(tag),
        }
    }

    fn path(&self) -> &Path {
        self.project.path()
    }

    fn write(&self, name: &str, src: &str) {
        self.project.write(name, src);
    }

    fn init(&self) {
        init_git_repo(self.path());
    }
}

fn nose_query_in(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["query", "."];
    args.extend_from_slice(extra);
    run_nose_query(dir, &args, "run nose query")
}

fn run_nose_query(dir: &Path, args: &[&str], context: &str) -> std::process::Output {
    Command::new(bin())
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .args(args)
        .output()
        .expect(context)
}

fn nose_query_base(dir: &Path, extra: &[&str]) -> std::process::Output {
    nose_query_base_with_mode(dir, None, extra)
}

fn query_base_json_value(dir: &Path, extra: &[&str]) -> serde_json::Value {
    let mut args = vec!["--format", "json"];
    args.extend_from_slice(extra);
    let out = nose_query_base(dir, &args);
    assert!(
        out.status.success(),
        "query base JSON should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|err| {
        panic!(
            "query base JSON parse failed: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn query_base_items(json: &serde_json::Value) -> &[serde_json::Value] {
    json["items"]
        .as_array()
        .unwrap_or_else(|| panic!("items should be an array: {json}"))
}

fn first_query_base_item(json: &serde_json::Value) -> &serde_json::Value {
    query_base_items(json)
        .first()
        .unwrap_or_else(|| panic!("expected at least one query-base item: {json}"))
}

fn site_files(item: &serde_json::Value, key: &str) -> Vec<String> {
    let mut files = item[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} should be a site array: {item}"))
        .iter()
        .map(|site| {
            site["file"]
                .as_str()
                .unwrap_or_else(|| panic!("{key} site should carry file: {site}"))
                .to_string()
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn site_trees(item: &serde_json::Value, key: &str) -> Vec<String> {
    let mut trees = item[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} should be a site array: {item}"))
        .iter()
        .map(|site| {
            site["tree"]
                .as_str()
                .unwrap_or_else(|| panic!("{key} site should carry tree: {site}"))
                .to_string()
        })
        .collect::<Vec<_>>();
    trees.sort();
    trees
}

fn assert_site_files(item: &serde_json::Value, key: &str, expected: &[&str]) {
    let mut expected = expected
        .iter()
        .map(|path| (*path).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(site_files(item, key), expected, "{key} files: {item}");
}

fn find_item_by_lane_and_files<'a>(
    json: &'a serde_json::Value,
    lane: &str,
    site_key: &str,
    expected_files: &[&str],
) -> &'a serde_json::Value {
    query_base_items(json)
        .iter()
        .find(|item| {
            item["lane"] == lane
                && site_files(item, site_key) == {
                    let mut expected = expected_files
                        .iter()
                        .map(|path| (*path).to_string())
                        .collect::<Vec<_>>();
                    expected.sort();
                    expected
                }
        })
        .unwrap_or_else(|| {
            panic!("expected {lane} item with {site_key} files {expected_files:?}: {json}")
        })
}

fn nose_query_base_with_mode(
    dir: &Path,
    mode: Option<&str>,
    extra: &[&str],
) -> std::process::Output {
    let mut args = vec!["query", ".", "base=HEAD", "--min-size", "8"];
    if let Some(mode) = mode {
        args.extend_from_slice(&["--mode", mode]);
    }
    for arg in extra {
        if *arg == "--fail" {
            args.extend_from_slice(&["--fail-on", "any"]);
        } else {
            args.push(arg);
        }
    }
    run_nose_query(dir, &args, "run nose query base")
}

#[test]
fn query_base_matches_base_ref_findings() {
    // `base=HEAD` and `base=main` run the same detection against this fixture state, so they
    // report the same findings (family_id + legacy fire verdict + v2 tier) on one diff.
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
        let mut ks: Vec<(String, bool, String)> = v[arr]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (
                    f["family_id"].as_str().unwrap().to_string(),
                    f["fire_eligible"].as_bool().unwrap(),
                    f["tier"].as_str().unwrap().to_string(),
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
        "query base= reports the same family ids + fire verdicts + tiers as query base"
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
    fs::remove_dir_all(dir.join("tests")).unwrap();
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
    let finding = first_query_base_item(&json);
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
    let fid = first_query_base_item(&json)["family_id"]
        .as_str()
        .expect("family_id should be exposed");

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

    let ignored_json_out = nose_query_base(&dir, &["--format", "json"]);
    assert!(
        ignored_json_out.status.success(),
        "suppressed JSON query should succeed: {}",
        String::from_utf8_lossy(&ignored_json_out.stderr)
    );
    let ignored_json: serde_json::Value =
        serde_json::from_slice(&ignored_json_out.stdout).expect("suppressed query base JSON");
    assert_eq!(
        ignored_json["items"].as_array().expect("items").len(),
        0,
        "structured ignores suppress JSON findings: {ignored_json}"
    );

    let ignored_sarif_out = nose_query_base(&dir, &["--format", "sarif"]);
    assert!(
        ignored_sarif_out.status.success(),
        "suppressed SARIF query should succeed: {}",
        String::from_utf8_lossy(&ignored_sarif_out.stderr)
    );
    let ignored_sarif: serde_json::Value =
        serde_json::from_slice(&ignored_sarif_out.stdout).expect("suppressed query base SARIF");
    assert_eq!(
        ignored_sarif["runs"][0]["results"]
            .as_array()
            .expect("SARIF results")
            .len(),
        0,
        "structured ignores suppress SARIF findings: {ignored_sarif}"
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
    assert_eq!(doc["version"], "2.1.0", "SARIF version: {doc}");
    assert_eq!(
        doc["runs"][0]["results"].as_array().map(Vec::len),
        Some(0),
        "empty diff emits no SARIF results: {doc}"
    );
    assert_eq!(
        doc["runs"][0]["properties"]["inconsistent_families"], 0,
        "empty diff records total divergent families: {doc}"
    );
    assert_eq!(
        doc["runs"][0]["properties"]["total_families"], 0,
        "empty diff records total families: {doc}"
    );
    assert_eq!(
        doc["runs"][0]["properties"]["shown_families"], 0,
        "empty diff records shown families: {doc}"
    );
    assert!(
        doc["runs"][0]["invocations"].is_null(),
        "empty diff should not emit truncation notifications: {doc}"
    );
    let _ = fs::remove_dir_all(&dir);
}
