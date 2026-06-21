use super::*;

#[test]
fn query_json_reports_cli_semantic_pack_metadata_without_changing_families() {
    let dir = make_project("semantic_pack_cli_report");
    let pack = dir.join("pack.json");
    fs::write(&pack, semantic_pack_manifest("com.example.cli-pack")).unwrap();

    let without_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    let with_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--semantic-pack",
        pack.to_str().unwrap(),
        "--format",
        "json",
    ]));

    assert_eq!(
        query_families(&with_pack),
        query_families(&without_pack),
        "metadata-only external packs must not change reported families"
    );
    let reported = semantic_pack_by_id(&with_pack, "com.example.cli-pack");
    assert_example_external_pack(reported, "com.example.cli-pack");
    assert_eq!(
        reported["path"].as_str(),
        Some(pack.canonicalize().unwrap().to_str().unwrap())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_json_keeps_executable_gated_pack_metadata_only() {
    let dir = make_project("semantic_pack_cli_executable_metadata");
    let pack = dir.join("pack.json");
    fs::write(
        &pack,
        semantic_pack_manifest_with_executable_gates("com.example.gated-cli-pack"),
    )
    .unwrap();

    let without_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    let with_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--semantic-pack",
        pack.to_str().unwrap(),
        "--format",
        "json",
    ]));

    assert_eq!(
        query_families(&with_pack),
        query_families(&without_pack),
        "executable-gated external packs must remain metadata-only in query results"
    );
    let reported = semantic_pack_by_id(&with_pack, "com.example.gated-cli-pack");
    assert_example_external_pack(reported, "com.example.gated-cli-pack");
    assert_eq!(
        reported["path"].as_str(),
        Some(pack.canonicalize().unwrap().to_str().unwrap())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn external_value_law_pack_does_not_add_semantic_law_provenance() {
    let dir = make_project("semantic_pack_external_law_metadata");
    fs::write(
        dir.join("repetition.py"),
        "def repeated(a, b):\n    return a * 2 + b * 2\n\n\ndef grouped(a, b):\n    return (a + b) * 2\n",
    )
    .unwrap();
    let pack = dir.join("law-pack.json");
    fs::write(
        &pack,
        semantic_pack_manifest_with_value_law("com.example.external-value-laws"),
    )
    .unwrap();

    let without_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "top=0",
        "--format",
        "json",
    ]));
    let with_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--semantic-pack",
        pack.to_str().unwrap(),
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "top=0",
        "--format",
        "json",
    ]));

    assert_eq!(
        query_families(&with_pack),
        query_families(&without_pack),
        "external value-law manifests must not change reported families"
    );
    assert!(
        query_families(&with_pack)
            .iter()
            .all(|family| family["semantic_laws"].is_null()),
        "external value-law manifests must not add semantic_laws provenance: {with_pack}"
    );
    let reported = semantic_pack_by_id(&with_pack, "com.example.external-value-laws");
    assert_eq!(reported["source"], "local-manifest");
    assert_eq!(reported["influence"], "metadata-only");
    assert_eq!(reported["counts"]["value_laws"], 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn external_pack_mirroring_builtin_type_domain_vocabulary_stays_metadata_only() {
    let dir = make_project("semantic_pack_type_domain_mirror");
    let pack = dir.join("pack.json");
    let mirror = semantic_pack_manifest("com.example.python-stdlib-type-domain-mirror")
        .replace(
            "python.library-api.example",
            "python.stdlib.type-domain-alias-domain",
        )
        .replace("LibraryApi.Contract", "Domain.TypeAlias")
        .replace(
            "python.example.contract",
            "python.stdlib.type-domain-alias.contract",
        )
        .replace("Example", "PythonStdlibTypeDomainAlias");
    fs::write(&pack, mirror).unwrap();

    let without_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    let with_pack = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--semantic-pack",
        pack.to_str().unwrap(),
        "--format",
        "json",
    ]));

    assert_eq!(
        query_families(&with_pack),
        query_families(&without_pack),
        "a local external pack mirroring builtin type-domain row ids must stay metadata-only"
    );
    let reported = semantic_pack_by_id(&with_pack, "com.example.python-stdlib-type-domain-mirror");
    assert_eq!(reported["source"], "local-manifest");
    assert_eq!(reported["influence"], "metadata-only");
    assert_eq!(reported["counts"]["evidence_producers"], 1);
    assert_eq!(reported["counts"]["contracts"], 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_pack_check_json_reports_conformance_success() {
    let dir = make_project("semantic_pack_check_ok");
    let fixture_dir = dir.join("fixtures");
    fs::create_dir_all(&fixture_dir).unwrap();
    fs::write(
        fixture_dir.join("positive.py"),
        "def positive(xs):\n    return sum(xs)\n",
    )
    .unwrap();
    fs::write(
        fixture_dir.join("negative.py"),
        "def negative(xs):\n    return list(xs)\n",
    )
    .unwrap();
    let pack = dir.join("pack.json");
    fs::write(&pack, semantic_pack_manifest("com.example.semantic-pack")).unwrap();

    let out = Command::new(bin())
        .args([
            "semantic-pack",
            "check",
            pack.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("semantic pack check");

    assert!(
        out.status.success(),
        "semantic-pack check should pass: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("check should emit JSON");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["totals"]["manifests"], 1);
    assert_eq!(json["totals"]["positive_fixtures"], 1);
    assert_eq!(json["totals"]["hard_negatives"], 1);
    assert_eq!(json["totals"]["fixture_issues"], 0);
    assert_eq!(json["totals"]["executable_conformance_rows"], 0);
    assert_eq!(json["totals"]["passed_executable_conformance_rows"], 0);
    assert_eq!(json["totals"]["executable_conformance_issues"], 0);
    assert_eq!(json["totals"]["influence_rows"], 2);
    assert_eq!(json["totals"]["blocked_influence_rows"], 2);
    assert_eq!(json["executable_conformance"]["status"], "unavailable");
    assert_eq!(json["influence_preflight"]["status"], "blocked");
    let influence_rows = json["influence_preflight"]["rows"].as_array().unwrap();
    assert_eq!(influence_rows.len(), 2);
    for row in influence_rows {
        assert_eq!(row["pack_id"], "com.example.semantic-pack");
        assert_eq!(row["passed"], false);
        assert_hex_hash(&row["row_hash"]);
        assert_hex_hash(&row["pack_hash"]);
        assert!(
            row["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|blocker| blocker == "data-only-registration"),
            "preflight row should report data-only blocker: {row}"
        );
        assert!(
            row["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|blocker| blocker == "dependency-backed-evidence-unavailable"),
            "preflight row should report dependency-backed evidence blocker: {row}"
        );
        assert!(
            row["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|blocker| blocker == "explicit-influence-trust-gate-missing"),
            "preflight row should report explicit trust gate blocker: {row}"
        );
        assert!(
            row["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|blocker| blocker == "executable-conformance-unavailable"),
            "exact-capable preflight row should report executable conformance blocker: {row}"
        );
    }
    assert_eq!(json["manifests"][0]["id"], "com.example.semantic-pack");
    assert_eq!(
        json["manifests"][0]["fixtures"][0]["issues"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_pack_check_json_reports_passed_executable_gates_without_opening_influence() {
    let dir = make_project("semantic_pack_check_executable_ok");
    let fixture_dir = dir.join("fixtures");
    fs::create_dir_all(&fixture_dir).unwrap();
    fs::write(
        fixture_dir.join("positive.py"),
        "def positive(xs):\n    return sum(xs)\n",
    )
    .unwrap();
    fs::write(
        fixture_dir.join("negative.py"),
        "def negative(xs):\n    return list(xs)\n",
    )
    .unwrap();
    let pack = dir.join("pack.json");
    fs::write(
        &pack,
        semantic_pack_manifest_with_executable_gates("com.example.semantic-pack"),
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "semantic-pack",
            "check",
            pack.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("semantic pack check");

    assert!(
        out.status.success(),
        "semantic-pack check should pass: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("check should emit JSON");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["totals"]["executable_conformance_rows"], 2);
    assert_eq!(json["totals"]["passed_executable_conformance_rows"], 2);
    assert_eq!(json["totals"]["executable_conformance_issues"], 0);
    assert_eq!(json["executable_conformance"]["status"], "ok");
    let gates = json["executable_conformance"]["rows"].as_array().unwrap();
    assert_eq!(gates.len(), 2);
    assert!(gates.iter().all(|gate| {
        gate["passed"] == true
            && gate["oracle"] == "fixture-expectations"
            && gate["issues"].as_array().unwrap().is_empty()
    }));

    let influence_rows = json["influence_preflight"]["rows"].as_array().unwrap();
    assert_eq!(influence_rows.len(), 2);
    for row in influence_rows {
        assert_eq!(row["passed"], false);
        let blockers = row["blockers"].as_array().unwrap();
        assert!(blockers
            .iter()
            .any(|blocker| blocker == "data-only-registration"));
        assert!(blockers
            .iter()
            .any(|blocker| blocker == "dependency-backed-evidence-unavailable"));
        assert!(blockers
            .iter()
            .any(|blocker| blocker == "explicit-influence-trust-gate-missing"));
        assert!(!blockers
            .iter()
            .any(|blocker| blocker == "executable-conformance-unavailable"));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn semantic_pack_check_fails_on_missing_fixture_files() {
    let dir = make_project("semantic_pack_check_missing");
    let pack = dir.join("pack.json");
    fs::write(&pack, semantic_pack_manifest("com.example.semantic-pack")).unwrap();

    let out = Command::new(bin())
        .args([
            "semantic-pack",
            "check",
            pack.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("semantic pack check");

    assert!(
        !out.status.success(),
        "semantic-pack check should fail when declared fixtures are missing"
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("failed check should still emit JSON");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["totals"]["fixture_issues"], 2);
    assert_eq!(json["totals"]["influence_rows"], 2);
    assert_eq!(json["totals"]["blocked_influence_rows"], 2);
    assert_eq!(json["influence_preflight"]["status"], "blocked");
    assert_eq!(
        json["influence_preflight"]["rows"]
            .as_array()
            .expect("preflight rows should be present")
            .len(),
        2
    );
    assert_eq!(
        json["manifests"][0]["fixtures"][0]["issues"][0],
        "missing-file"
    );
    assert!(
        stderr.contains("semantic pack conformance failed"),
        "stderr should name the conformance failure: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_json_keeps_python_repetition_out_of_numeric_law_provenance() {
    let dir =
        std::env::temp_dir().join(format!("nose_cli_law_hard_negative_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("repetition.py"),
        "def repeated(a, b):\n    return a * 2 + b * 2\n\n\ndef grouped(a, b):\n    return (a + b) * 2\n",
    )
    .unwrap();
    let json = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "top=0",
        "--format",
        "json",
    ]));
    let families = json["families"].as_array().expect("families array");
    assert!(
        families
            .iter()
            .all(|family| family["semantic_laws"].is_null()),
        "Python repetition must not report numeric factor-distribution provenance: {json}"
    );
    assert!(
        families.is_empty(),
        "Python repetition must fail closed for the semantic exact channel: {json}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_semantic_packs_are_explicit_opt_ins() {
    let dir = make_project("semantic_pack_cfg");
    fs::write(
        dir.join("pack.json"),
        semantic_pack_manifest("com.example.config-pack"),
    )
    .unwrap();
    fs::write(
        dir.join("nose.toml"),
        "[query]\nmin-size = 12\nsemantic-packs = [\"pack.json\"]\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args(["query", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "query should load config semantic pack: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = query_json(&stdout);
    let reported = semantic_pack_by_id(&json, "com.example.config-pack");
    assert_example_external_pack(reported, "com.example.config-pack");
    assert_eq!(
        reported["path"].as_str(),
        Some(
            dir.join("pack.json")
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
        )
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn explicit_config_semantic_pack_paths_resolve_from_config_directory() {
    let dir = make_project("semantic_pack_explicit_cfg");
    fs::write(
        dir.join("pack.json"),
        semantic_pack_manifest("com.example.explicit-config-pack"),
    )
    .unwrap();
    let config = dir.join("nose.toml");
    fs::write(
        &config,
        "[query]\nmin-size = 12\nsemantic-packs = [\"pack.json\"]\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "query",
            dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--format",
            "json",
        ])
        .current_dir(dir.parent().expect("test project has a parent"))
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "query should load config-relative semantic pack: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = query_json(&stdout);
    let reported = semantic_pack_by_id(&json, "com.example.explicit-config-pack");
    assert_example_external_pack(reported, "com.example.explicit-config-pack");
    assert_eq!(
        reported["path"].as_str(),
        Some(
            dir.join("pack.json")
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
        )
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn explicit_config_ignore_file_resolves_from_config_directory() {
    let dir = make_project("ignore_explicit_cfg");
    fs::write(
        dir.join("nose.ignore.json"),
        "{\"ignores\":[{\"paths\":[\"**/a/**\",\"**/b/**\",\"**/tests/**\"],\"reason\":\"template-copy\"}]}\n",
    )
    .unwrap();
    let config = dir.join("nose.toml");
    fs::write(
        &config,
        "[query]\nmin-size = 12\nignore-file = \"nose.ignore.json\"\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "query",
            dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--mode",
            "semantic",
            "--format",
            "json",
            "top=0",
        ])
        .current_dir(dir.parent().expect("test project has a parent"))
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "query should load config-relative ignore file: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = query_json(&stdout);
    assert!(
        query_families(&json).is_empty(),
        "config-relative ignore file should suppress the family: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}
