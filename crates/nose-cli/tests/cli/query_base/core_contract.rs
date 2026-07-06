use super::*;

fn make_divergent_project() -> PathBuf {
    let dir = make_project("query_base");
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

fn assert_human_report_names_changed_and_skipped(dir: &Path) {
    let out = nose_query_in(dir, &["base=main", "--min-size", "8"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("divergent") && stdout.contains("a/f.py") && stdout.contains("b/f.py"),
        "base= names the changed copy and the un-updated sibling: {stdout}"
    );
}

fn assert_json_contract(dir: &Path) {
    let out = nose_query_in(dir, &["base=main", "--min-size", "8", "--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["schema_version"], 8,
        "base view emits v2 schema: {json}"
    );
    assert_eq!(
        json["view"], "base",
        "query schema envelope, base view: {json}"
    );
    assert_query_json_reports_semantic_packs(&json);
    assert_eq!(json["base"], "main");
    assert!(
        json["summary"]["divergences"].as_u64().unwrap() >= 1,
        "at least one divergence: {json}"
    );
    assert!(
        json["items"][0]["fire_eligible"].is_boolean(),
        "items carry the §BV fire verdict: {json}"
    );
    let item = &json["items"][0];
    assert_eq!(item["lane"], "base-divergence", "v2 lane: {json}");
    assert_eq!(
        item["base_family_id"], item["family_id"],
        "base family id: {json}"
    );
    assert_eq!(item["tier"], "strict", "shared prod edit is strict: {json}");
    assert_eq!(
        item["taxonomy_hint"], "missed_propagation",
        "strict taxonomy: {json}"
    );
    assert_eq!(
        item["gate"]["fail_default"], true,
        "strict fails default gate: {json}"
    );
    assert_eq!(
        item["gate"]["policy"], "divergent-edit-v2-strict",
        "policy: {json}"
    );
    assert_eq!(
        item["changed"][0]["tree"], "base",
        "site coordinate origin: {json}"
    );
    assert_eq!(
        item["not_updated"][0]["tree"], "base",
        "skipped sibling coordinate origin: {json}"
    );
    for site in item["changed"]
        .as_array()
        .unwrap()
        .iter()
        .chain(item["not_updated"].as_array().unwrap())
    {
        for key in [
            "tree",
            "file",
            "start_line",
            "end_line",
            "lang",
            "kind",
            "span_lines",
            "span_tokens",
            "is_fragment",
            "touches_shared",
        ] {
            assert!(
                site.get(key).is_some(),
                "base v8 site carries required `{key}` coordinate field: {json}"
            );
        }
    }
}

fn assert_sarif_contract(dir: &Path) {
    let out = nose_query_in(dir, &["base=main", "--min-size", "8", "--format", "sarif"]);
    assert!(
        out.status.success(),
        "base= SARIF should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sarif: serde_json::Value = serde_json::from_slice(&out.stdout).expect("query base SARIF");
    assert!(
        sarif["runs"][0]["results"]
            .as_array()
            .is_some_and(|r| !r.is_empty()),
        "query base= SARIF reuses query base findings: {sarif}"
    );
    let result = &sarif["runs"][0]["results"][0];
    assert_eq!(result["ruleId"], "nose.divergent.strict");
    assert_eq!(result["level"], "error");
    assert!(
        result["message"]["text"]
            .as_str()
            .is_some_and(|message| message.starts_with("Strict divergent edit:")),
        "strict SARIF message names the tier: {sarif}"
    );
    assert!(
        result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("b/f.py")),
        "base-divergence SARIF anchors the skipped sibling: {sarif}"
    );
    assert_eq!(result["properties"]["tier"], "strict");
    assert_eq!(result["properties"]["lane"], "base-divergence");
    assert_eq!(
        result["properties"]["base_family_id"],
        result["properties"]["family_id"]
    );
    assert_eq!(result["properties"]["taxonomy_hint"], "missed_propagation");
    assert!(
        result["properties"]["tier_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason == "shared_logic_touched"),
        "strict SARIF carries tier reasons: {sarif}"
    );
    assert_eq!(result["properties"]["gate"]["eligible"], true);
    assert_eq!(result["properties"]["gate"]["fail_default"], true);
    assert_eq!(
        result["properties"]["gate"]["policy"],
        "divergent-edit-v2-strict"
    );
    assert_eq!(result["properties"]["policy"], "divergent-edit-v2-strict");
    assert_eq!(result["properties"]["fire_eligible"], true);
    assert_eq!(
        result["locations"][0]["physicalLocation"]["region"]["startLine"], 1,
        "SARIF skipped-sibling location carries a region: {sarif}"
    );
    assert!(
        result["relatedLocations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("a/f.py")),
        "base-divergence SARIF relates the changed copy: {sarif}"
    );
}

#[test]
fn query_base_v8_docs_cover_wrapper_fields() {
    let docs_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .join("docs/query-json.md");
    let markdown = fs::read_to_string(&docs_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", docs_path.display()));
    let section = markdown
        .split_once("**`base`**")
        .and_then(|(_, rest)| rest.split_once("## The family object").map(|(s, _)| s))
        .unwrap_or_else(|| panic!("docs/query-json.md missing base view section"));
    for key in [
        "lane",
        "tier",
        "tier_reasons",
        "taxonomy_hint",
        "gate.fail_default",
        "gate.policy",
        "suppression.kind",
    ] {
        assert!(
            section.contains(&format!("`{key}`")) || section.contains(&format!("`{key}[]`")),
            "docs/query-json.md base section should document `{key}`"
        );
    }
}

fn assert_unsupported_terms_are_rejected(dir: &Path) {
    let unsupported = nose_query_in(dir, &["base=main", "path~a/f.py", "--min-size", "8"]);
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
        let out = nose_query_in(dir, args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "base= should reject {label}");
        assert!(
            stderr.contains(needle),
            "base= names unsupported {label}: {stderr}"
        );
    }
}

#[test]
fn query_base_flags_divergent_edits() {
    let dir = make_divergent_project();
    assert_human_report_names_changed_and_skipped(&dir);
    assert_json_contract(&dir);
    assert_sarif_contract(&dir);
    assert_unsupported_terms_are_rejected(&dir);

    let gated = nose_query_in(&dir, &["base=main", "--min-size", "8", "--fail-on", "any"]);
    assert!(
        !gated.status.success(),
        "base= --fail-on any exits non-zero on a strict divergence"
    );

    let _ = fs::remove_dir_all(&dir);
}
