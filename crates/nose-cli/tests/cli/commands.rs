use super::*;

#[test]
fn scan_reports_the_clone_family() {
    let dir = make_project("fam");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--top",
        "10",
    ]);
    assert!(out.contains("clone"), "has a header: {out}");
    assert!(out.contains("copies"), "lists a family: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exclude_drops_matching_paths() {
    let dir = make_project("excl");
    let p = dir.to_str().unwrap();
    // The family spans a/, b/, tests/. Excluding `tests` must shrink it.
    let with = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    let without = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
        "--exclude",
        "tests",
    ]);
    let count = |s: &str| s.matches("\"file\"").count();
    assert!(
        count(&with) > count(&without),
        "excluding tests removes a site: {} vs {}",
        count(&with),
        count(&without)
    );
    assert!(
        !without.contains("/tests/"),
        "no tests/ path in excluded output"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exclude_supports_globs() {
    // `--exclude` takes gitignore-style globs, not just substrings: `a/**` excludes
    // the a/ directory specifically, leaving the family in b/ and tests/.
    let dir = make_project("glob");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--exclude",
        "a/**",
    ]);
    assert!(
        !out.contains("/a/f.py"),
        "glob `a/**` must exclude a/f.py: {out}"
    );
    assert!(out.contains("/b/f.py"), "b/ must remain: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn invalid_exclude_glob_fails_clearly() {
    let dir = make_project("bad_exclude_glob");
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--min-size",
            "12",
            "--exclude",
            "[",
        ])
        .output()
        .expect("run");
    assert!(
        !out.status.success(),
        "invalid exclude glob should fail instead of being ignored"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("invalid exclude glob"),
        "stderr should explain the bad exclude glob: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn min_value_filters_low_value_families() {
    let dir = make_project("minval");
    let p = dir.to_str().unwrap();
    // A value floor above any family's value hides them all; zero keeps them.
    let all = run(&["scan", p, "--min-size", "12", "--min-value", "0"]);
    let none = run(&["scan", p, "--min-size", "12", "--min-value", "100000"]);
    assert!(all.contains("copies"), "unfiltered shows a family: {all}");
    assert!(
        !none.contains("copies"),
        "a high value floor hides every family: {none}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn min_value_rejects_nan() {
    let dir = make_project("minval_nan");
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--min-size",
            "12",
            "--min-value",
            "NaN",
            "--fail-on",
            "any",
        ])
        .output()
        .expect("run scan");

    assert!(
        !out.status.success(),
        "--min-value NaN must be rejected instead of filtering every family out"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("min-value must be a finite non-negative number"),
        "stderr should explain the invalid value: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hotspots_lists_modules() {
    let dir = make_project("hot");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--show",
        "hotspots",
    ]);
    assert!(
        out.contains("duplication hotspots"),
        "hotspots section present: {out}"
    );
    assert!(out.contains("dup lines"), "hotspot rows present: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_corpus_warns_on_stderr() {
    // A path with no supported source must not silently look like "no duplication".
    let dir = std::env::temp_dir().join(format!("nose_empty_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.txt"), "no code here\n").unwrap();
    let out = Command::new(bin())
        .args(["scan", dir.to_str().unwrap()])
        .output()
        .expect("run nose");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("no supported source files found"),
        "expected an empty-corpus warning, got: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn output_is_independent_of_thread_count() {
    // Parallel lowering/detection must not affect output: a single-threaded run and
    // a multi-threaded run must produce byte-identical JSON. A release invariant.
    let dir = make_project("threads");
    let p = dir.to_str().unwrap();
    let run_with_threads = |n: &str| {
        let out = Command::new(bin())
            .args(["scan", p, "--min-size", "12", "--format", "json"])
            .env("RAYON_NUM_THREADS", n)
            .output()
            .expect("run nose");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    assert_eq!(
        run_with_threads("1"),
        run_with_threads("8"),
        "1-thread and 8-thread runs must produce identical output"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cache_output_matches_uncached_cold_and_warm() {
    // --cache-dir must not change results: cold (populating) and warm (reusing)
    // runs must both equal the non-cached output. This is the cache's correctness
    // contract — a stale/incorrect entry would diverge here.
    let dir = make_project("cache");
    // make_project's clones are *renamed* copies — matched only by the value-graph
    // channel. Add files with a *verbatim* duplicated block so the contiguous
    // (copy-paste) channel also produces a family: the cache once stored only units and
    // silently dropped that channel, so without the streams cached this run would diverge
    // here. (Identical variable names → identical raw token stream → a contiguous match,
    // unlike the renamed value-graph clones.)
    let block = "def handle(events):\n    out = []\n    for e in events:\n        if e.kind == 1:\n            out.append(e.payload)\n            record(e.id, e.kind)\n    return out\n";
    for name in ["dup_a", "dup_b", "dup_c"] {
        fs::write(dir.join(format!("{name}.py")), block).unwrap();
    }
    let p = dir.to_str().unwrap();
    let cache = std::env::temp_dir().join(format!("nose_cachedir_{}", std::process::id()));
    let _ = fs::remove_dir_all(&cache);
    let cd = cache.to_str().unwrap();

    let uncached = run(&["scan", p, "--min-size", "12", "--format", "json"]);
    let cold = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--cache-dir",
        cd,
    ]);
    let warm = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--cache-dir",
        cd,
    ]);

    assert_eq!(
        uncached, cold,
        "cold cache run must match the uncached output"
    );
    assert_eq!(cold, warm, "warm cache run must match the cold run");
    let _ = fs::remove_dir_all(&cache);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_flag_sets_exit_code_as_ci_gate() {
    let dir = make_project("fail");
    let p = dir.to_str().unwrap();
    // A family exists → --fail must exit non-zero.
    let found = Command::new(bin())
        .args(["scan", p, "--min-size", "12", "--fail-on", "any"])
        .output()
        .expect("run");
    assert!(
        !found.status.success(),
        "--fail must fail when a family is found"
    );
    // Filter everything out → nothing to fail on → success.
    let clean = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--min-value",
            "1e9",
            "--fail-on",
            "any",
        ])
        .output()
        .expect("run");
    assert!(
        clean.status.success(),
        "--fail must pass when no family survives filters"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn output_paths_are_relative_to_cwd() {
    // Even when scanned via an absolute path, reported paths should be relative to
    // the working directory (readable in CI logs / reviews).
    let dir = make_project("relpath");
    let abs = dir.canonicalize().unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            abs.to_str().unwrap(),
            "--min-size",
            "12",
            "--format",
            "json",
        ])
        .current_dir(&abs)
        .output()
        .expect("run");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("\"a/f.py\"") || stdout.contains("\"b/f.py\""),
        "paths should be relative to cwd, got: {stdout}"
    );
    assert!(
        !stdout.contains(abs.to_str().unwrap()),
        "no absolute path should leak: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn baseline_hides_accepted_families() {
    // Write a baseline accepting today's families, then a re-run reports nothing new.
    let dir = make_project("baseline");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!("nose_bl_{}.json", std::process::id()));
    let bls = bl.to_str().unwrap();

    // Sanity: without a baseline there IS a family.
    assert!(run(&["scan", p, "--min-size", "12"]).contains("copies"));

    // Accept current state…
    let _ = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--write-baseline",
        ])
        .output()
        .expect("run");
    assert!(bl.exists(), "baseline file should be written");
    let baseline_text = fs::read_to_string(&bl).expect("read baseline");
    assert!(
        baseline_text.contains("\"members\""),
        "baseline should record member identities for changed/resolved comparison: {baseline_text}"
    );
    assert!(
        baseline_text.contains("\"start_line\"") && baseline_text.contains("\"kind\""),
        "baseline member identities should include location fields for unique family ids: {baseline_text}"
    );

    // …then a re-run shows no *new* families.
    let after = run(&["scan", p, "--min-size", "12", "--baseline", bls]);
    assert!(
        !after.contains("sites"),
        "baselined families must be hidden, got: {after}"
    );
    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_baseline_file_fails_clearly() {
    let dir = make_project("baseline_missing");
    let p = dir.to_str().unwrap();
    let bl =
        std::env::temp_dir().join(format!("nose_missing_baseline_{}.json", std::process::id()));
    let _ = fs::remove_file(&bl);

    let out = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bl.to_str().unwrap(),
        ])
        .output()
        .expect("run scan");

    assert!(
        !out.status.success(),
        "an explicitly requested missing baseline must fail, not behave like an empty baseline"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("reading baseline"),
        "stderr should identify the missing baseline: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn malformed_baseline_file_fails_clearly() {
    let dir = make_project("baseline_malformed");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!(
        "nose_malformed_baseline_{}.json",
        std::process::id()
    ));
    fs::write(&bl, "{ not json\n").unwrap();

    let out = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bl.to_str().unwrap(),
        ])
        .output()
        .expect("run scan");

    assert!(
        !out.status.success(),
        "a malformed baseline must fail, not behave like an empty baseline"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("parsing baseline"),
        "stderr should identify the malformed baseline: {stderr}"
    );
    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn new_only_json_marks_new_families_against_baseline() {
    let dir = make_project("new_only");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!("nose_new_only_bl_{}.json", std::process::id()));
    let bls = bl.to_str().unwrap();

    let baseline = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--write-baseline",
        ])
        .output()
        .expect("write baseline");
    assert!(baseline.status.success());

    add_distinct_clone_family(&dir);
    let out = run(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--baseline",
        bls,
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    let families = scan_families(&json);
    assert!(
        !families.is_empty(),
        "new-only scan should report the introduced family: {out}"
    );
    assert!(
        out.contains("fresh_a.py") && out.contains("fresh_b.py") && !out.contains("a/f.py"),
        "new-only JSON should include new sites, not accepted baseline sites: {out}"
    );
    assert_eq!(json["baseline"]["mode"], "new-only");
    assert!(json["baseline"]["new_families"].as_u64().unwrap() >= 1);
    assert!(json["baseline"]["unchanged_families"].as_u64().unwrap() >= 1);
    assert_eq!(json["baseline"]["changed_families"], 0);
    assert_eq!(json["baseline"]["resolved_families"], 0);
    assert!(
        families
            .iter()
            .all(|f| f["baseline_status"].as_str() == Some("new")),
        "all reportable families should be marked new: {out}"
    );

    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_on_new_fails_for_changed_family_and_passes_when_clean() {
    let dir = make_project("fail_on_new");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!("nose_fail_on_new_bl_{}.json", std::process::id()));
    let bls = bl.to_str().unwrap();

    let baseline = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--write-baseline",
        ])
        .output()
        .expect("write baseline");
    assert!(baseline.status.success());

    let clean = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--fail-on",
            "new",
        ])
        .output()
        .expect("clean run");
    assert!(
        clean.status.success(),
        "--fail-on-new should pass when every family is accepted"
    );

    add_member_to_existing_family(&dir);
    let changed = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-size",
            "12",
            "--baseline",
            bls,
            "--fail-on",
            "new",
            "--format",
            "json",
            "--top",
            "0",
        ])
        .output()
        .expect("changed run");
    assert!(
        !changed.status.success(),
        "--fail-on-new should fail when a family changes"
    );
    let stdout = String::from_utf8(changed.stdout).unwrap();
    let stderr = String::from_utf8(changed.stderr).unwrap();
    let json = scan_json(&stdout);
    assert_eq!(json["baseline"]["new_families"], 0);
    assert_eq!(json["baseline"]["changed_families"], 1);
    assert_eq!(json["baseline"]["resolved_families"], 0);
    assert_eq!(scan_families(&json)[0]["baseline_status"], "changed");
    assert!(
        stderr.contains("--fail-on new"),
        "stderr should name the explicit gate: {stderr}"
    );

    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_file_supplies_defaults() {
    // A nose.toml in the working dir provides defaults (here: an exclude glob and
    // min-size); a CLI flag still overrides.
    let dir = make_project("cfg");
    fs::write(
        dir.join("nose.toml"),
        "[scan]\nexclude = [\"a/**\"]\nmin-size = 12\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args(["scan", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("a/f.py"),
        "config exclude a/** should drop a/f.py: {stdout}"
    );
    assert!(stdout.contains("b/f.py"), "b/ remains: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

fn semantic_pack_manifest(id: &str) -> String {
    format!(
        r#"{{
  "api_version": "nose.semantic-pack.v0",
  "pack": {{
    "id": "{id}",
    "kind": "LibraryPack",
    "version": "0.1.0",
    "display_name": "Example semantic pack",
    "trust": "external-opt-in",
    "enabled_by_default": false
  }},
  "provenance": {{
    "provider": {{ "name": "Example Packs" }},
    "license": "MIT",
    "repository": "https://example.invalid/semantic-pack"
  }},
  "compatibility": {{ "nose": ">=0.5.0 <0.6.0" }},
  "supported_languages": [{{ "id": "python" }}],
  "declares": {{
    "evidence_producers": [{{
      "id": "python.library-api.example",
      "kind": "LibraryApi.Contract",
      "anchors": ["node"],
      "channel": "exact-empirical",
      "stable_hash_inputs": ["pack.id", "producer.id", "call_span"],
      "conflict_policy": "fail-closed"
    }}],
    "contracts": [{{
      "id": "python.example.contract",
      "surface": {{ "kind": "function" }},
      "requires": [{{
        "ref": "python.library-api.example",
        "subject": "call",
        "required": true
      }}],
      "semantics": {{
        "operation": "Example",
        "demand": {{ "arguments": "eager-left-to-right" }},
        "effects": ["argument-effects-in-order"]
      }},
      "channel": "exact-empirical",
      "proof_status": "covered",
      "conformance_refs": ["positive", "negative"]
    }}],
    "value_laws": []
  }},
  "conformance": {{
    "positive_fixtures": [{{
      "id": "positive",
      "description": "positive",
      "path": "fixtures/positive.py",
      "expectation": "exact-contract-open"
    }}],
    "hard_negatives": [{{
      "id": "negative",
      "description": "negative",
      "path": "fixtures/negative.py",
      "expectation": "exact-contract-closed"
    }}],
    "known_unsupported": []
  }}
}}"#
    )
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
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["totals"]["manifests"], 1);
    assert_eq!(json["totals"]["positive_fixtures"], 1);
    assert_eq!(json["totals"]["hard_negatives"], 1);
    assert_eq!(json["totals"]["fixture_issues"], 0);
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

/// Compiled-in pack rows share the same provenance coordinates.
fn assert_compiled_first_party_pack(pack: &serde_json::Value, id: &str) {
    assert_eq!(pack["id"], id);
    assert_eq!(pack["source"], "compiled-first-party");
    assert_eq!(pack["influence"], "evidence-and-contracts");
}

#[test]
fn scan_json_reports_first_party_and_local_semantic_pack_provenance() {
    let dir = make_project("semantic_pack_cli");
    let pack = dir.join("pack.json");
    fs::write(&pack, semantic_pack_manifest("com.example.semantic-pack")).unwrap();

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--format",
        "json",
        "--min-size",
        "12",
        "--semantic-pack",
        pack.to_str().unwrap(),
    ]);
    let json = scan_json(&out);
    let packs = json["semantic_packs"]
        .as_array()
        .expect("semantic_packs array");
    assert_eq!(
        packs.len(),
        4,
        "first-party packs + local opt-in pack: {json}"
    );
    assert_compiled_first_party_pack(&packs[0], "nose.first_party");
    assert_compiled_first_party_pack(&packs[1], "nose.python.stdlib.type_domain");
    assert_eq!(packs[1]["kind"], "StdlibPack");
    assert_eq!(packs[1]["counts"]["evidence_producers"], 1);
    assert_eq!(packs[1]["counts"]["contracts"], 1);
    assert_compiled_first_party_pack(&packs[2], "nose.value_graph.laws");
    assert_eq!(packs[2]["kind"], "LawPack");
    assert_eq!(packs[2]["counts"]["value_laws"], 2);
    assert_eq!(packs[3]["id"], "com.example.semantic-pack");
    assert_eq!(packs[3]["trust"], "external-opt-in");
    assert_eq!(packs[3]["source"], "local-manifest");
    assert_eq!(packs[3]["influence"], "metadata-only");
    assert_eq!(packs[3]["counts"]["contracts"], 1);
    assert!(packs[3]["hash"]
        .as_str()
        .is_some_and(|hash| hash.len() == 16));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_json_reports_value_law_provenance_for_semantic_family() {
    let dir = std::env::temp_dir().join(format!("nose_cli_law_provenance_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("left.rs"),
        "fn f(a: i64, b: i64, c: i64) -> i64 {\n    a * c + b * c\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("right.rs"),
        "fn g(a: i64, b: i64, c: i64) -> i64 {\n    (a + b) * c\n}\n",
    )
    .unwrap();
    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--top",
        "0",
        "--format",
        "json",
    ]));
    let families = json["families"].as_array().expect("families array");
    assert_eq!(
        families.len(),
        1,
        "factor-distribution family expected: {json}"
    );
    let laws = families[0]["semantic_laws"]
        .as_array()
        .expect("semantic_laws array");
    assert_eq!(laws.len(), 1, "one law provenance row expected: {json}");
    assert_eq!(laws[0]["pack_id"], "nose.value_graph.laws");
    assert_eq!(
        laws[0]["law_id"],
        "value-graph.factor-distribute.numeric-common-factor"
    );
    assert_eq!(
        laws[0]["proof_obligation_id"],
        "normalize.value_graph.factor_distribute"
    );
    assert_eq!(laws[0]["proof_status"], "proven");
    assert_eq!(laws[0]["channel"], "exact-proven");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_json_keeps_python_repetition_out_of_numeric_law_provenance() {
    let dir =
        std::env::temp_dir().join(format!("nose_cli_law_hard_negative_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("repetition.py"),
        "def repeated(a, b):\n    return a * 2 + b * 2\n\n\ndef grouped(a, b):\n    return (a + b) * 2\n",
    )
    .unwrap();
    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--top",
        "0",
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
fn scan_human_reports_local_semantic_pack_opt_in() {
    let dir = make_project("semantic_pack_human");
    let pack = dir.join("pack.json");
    fs::write(&pack, semantic_pack_manifest("com.example.semantic-pack")).unwrap();

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--semantic-pack",
        pack.to_str().unwrap(),
    ]);
    assert!(
        out.contains(
            "semantic packs: 3 first-party default · 1 local opt-in: \
             com.example.semantic-pack@0.1.0 (metadata-only)"
        ),
        "human scan output should disclose local semantic pack opt-ins: {out}"
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
        "[scan]\nmin-size = 12\nsemantic-packs = [\"pack.json\"]\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args(["scan", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "scan should load config semantic pack: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = scan_json(&stdout);
    let packs = json["semantic_packs"]
        .as_array()
        .expect("semantic_packs array");
    assert!(packs.iter().any(
        |pack| pack["id"] == "com.example.config-pack" && pack["influence"] == "metadata-only"
    ));
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
        "[scan]\nmin-size = 12\nsemantic-packs = [\"pack.json\"]\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "scan",
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
        "scan should load config-relative semantic pack: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = scan_json(&stdout);
    let packs = json["semantic_packs"]
        .as_array()
        .expect("semantic_packs array");
    assert!(packs
        .iter()
        .any(|pack| pack["id"] == "com.example.explicit-config-pack"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn explicit_config_ignore_file_resolves_from_config_directory() {
    let dir = make_project("ignore_explicit_cfg");
    fs::write(
        dir.join("nose.ignore.json"),
        "{\"ignores\":[{\"paths\":[\"**/a/**\"],\"reason\":\"template-copy\"}]}\n",
    )
    .unwrap();
    let config = dir.join("nose.toml");
    fs::write(
        &config,
        "[scan]\nmin-size = 12\nignore-file = \"nose.ignore.json\"\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--mode",
            "semantic",
            "--format",
            "json",
            "--top",
            "0",
        ])
        .current_dir(dir.parent().expect("test project has a parent"))
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "scan should load config-relative ignore file: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = scan_json(&stdout);
    assert!(
        scan_families(&json).is_empty(),
        "config-relative ignore file should suppress the family: {stdout}"
    );
    assert_eq!(json["ignore"]["active_entries"], 1);
    assert_eq!(json["ignore"]["ignored_families"], 1);
    assert!(json["ignore"]["path"]
        .as_str()
        .is_some_and(|path| path.ends_with("nose.ignore.json")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn inline_nose_ignore_suppresses_a_site() {
    // A `nose-ignore` marker above a function drops that site; with only one copy
    // left there's no family.
    let dir = std::env::temp_dir().join(format!("nose_sup_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let body = "def f(items):\n    t = 0\n    for x in items:\n        if x > 0:\n            t = t + x * x\n    return t\n";
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(dir.join("a/f.py"), body).unwrap();
    fs::write(dir.join("b/f.py"), format!("# nose-ignore\n{body}")).unwrap();
    let p = dir.to_str().unwrap();
    assert!(
        run(&["scan", p, "--min-size", "12"]).contains("0 clone"),
        "the marked copy must be suppressed, leaving no family"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn structured_ignore_suppresses_family_id_with_metadata() {
    let dir = make_project("structured_ignore_id");
    let p = dir.to_str().unwrap();
    let before = run(&[
        "scan",
        p,
        "--mode",
        "semantic",
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let before_json = scan_json(&before);
    let family_id = scan_families(&before_json)[0]["family_id"]
        .as_str()
        .expect("family_id should be exposed")
        .to_string();
    let ignore_file = std::env::temp_dir().join(format!(
        "nose_structured_ignore_{}.json",
        std::process::id()
    ));
    fs::write(
        &ignore_file,
        format!(
            "{{\"ignores\":[{{\"family_id\":\"{family_id}\",\"reason\":\"generated-code\",\"note\":\"Generated from the same template.\",\"owner\":\"platform\",\"expires_at\":\"2099-01-01\"}}]}}\n"
        ),
    )
    .unwrap();

    let after = run(&[
        "scan",
        p,
        "--mode",
        "semantic",
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "0",
        "--ignore-file",
        ignore_file.to_str().unwrap(),
    ]);
    let after_json = scan_json(&after);
    assert!(
        scan_families(&after_json).is_empty(),
        "the ignored family should be absent from active findings: {after}"
    );
    assert_eq!(after_json["ignore"]["active_entries"], 1);
    assert_eq!(after_json["ignore"]["ignored_families"], 1);
    assert_eq!(after_json["ignored_families"][0]["family_id"], family_id);
    assert_eq!(
        after_json["ignored_families"][0]["ignore"]["reason"],
        "generated-code"
    );
    assert_eq!(
        after_json["ignored_families"][0]["ignore"]["owner"],
        "platform"
    );

    let _ = fs::remove_file(&ignore_file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn default_structured_ignore_file_matches_paths() {
    let dir = make_project("structured_ignore_paths");
    fs::write(
        dir.join("nose.ignore.json"),
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"reason\":\"template-copy\",\"note\":\"a/ is generated.\"}]}\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            ".",
            "--mode",
            "semantic",
            "--min-size",
            "12",
            "--format",
            "json",
            "--top",
            "0",
        ])
        .current_dir(&dir)
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "scan should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = scan_json(&stdout);
    assert!(
        scan_families(&json).is_empty(),
        "path ignore should suppress the family: {stdout}"
    );
    assert_eq!(json["ignore"]["active_entries"], 1);
    assert_eq!(json["ignore"]["ignored_families"], 1);
    assert_eq!(
        json["ignored_families"][0]["ignore"]["matched_paths"][0],
        "./a/f.py"
    );
    assert_eq!(
        json["ignored_families"][0]["ignore"]["selectors"]["paths"][0],
        "a/**"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn expired_structured_ignore_is_reported_but_not_applied() {
    let dir = make_project("structured_ignore_expired");
    fs::write(
        dir.join("nose.ignore.json"),
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"reason\":\"temporary-waiver\",\"owner\":\"platform\",\"expires_at\":\"2000-01-01\"}]}\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            ".",
            "--mode",
            "semantic",
            "--min-size",
            "12",
            "--format",
            "json",
            "--top",
            "0",
        ])
        .current_dir(&dir)
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    let json = scan_json(&stdout);
    assert!(
        !scan_families(&json).is_empty(),
        "expired ignore must not suppress the family: {stdout}"
    );
    assert_eq!(json["ignore"]["active_entries"], 0);
    assert_eq!(json["ignore"]["expired_entries"], 1);
    assert_eq!(json["ignore"]["ignored_families"], 0);
    assert_eq!(json["ignore"]["expired"][0]["reason"], "temporary-waiver");
    assert!(
        stderr.contains("expired on 2000-01-01") && stderr.contains("not applied"),
        "stderr should explain the expired entry: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn malformed_structured_ignore_file_fails_clearly() {
    let dir = make_project("structured_ignore_bad");
    let ignore_file = dir.join("bad-ignore.json");
    fs::write(
        &ignore_file,
        "{\"ignores\":[{\"paths\":[\"a/**\"],\"note\":\"missing reason\"}]}\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--min-size",
            "12",
            "--ignore-file",
            ignore_file.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(!out.status.success(), "malformed ignore files must fail");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("parsing ignore file") || stderr.contains("validating ignore file"),
        "error should name the ignore file problem: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sarif_output_is_well_formed() {
    let dir = make_project("sarif");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "sarif",
    ]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("SARIF must be valid JSON");
    assert_eq!(v["version"], "2.1.0");
    assert!(v["runs"][0]["tool"]["driver"]["name"] == "nose");
    assert!(
        !v["runs"][0]["results"].as_array().unwrap().is_empty(),
        "should have at least one result: {out}"
    );
    // The run records the full family count, so a consumer can tell a complete upload
    // from a truncated one. This single-family project is not truncated (default --top 30),
    // so total == shown and there is no truncation notification.
    let props = &v["runs"][0]["properties"];
    let total = props["total_families"].as_u64().expect("total_families");
    let shown = props["shown_families"].as_u64().expect("shown_families");
    assert_eq!(shown, total, "untruncated run: shown == total ({out})");
    assert_eq!(
        shown as usize,
        v["runs"][0]["results"].as_array().unwrap().len()
    );
    assert!(
        v["runs"][0].get("invocations").is_none(),
        "no truncation note when nothing is hidden: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// A project with several behaviorally-distinct duplicated functions, so `nose scan`
/// reports multiple clone families. `--top N` can then truncate the report.
fn make_multi_family_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_cli_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // Four distinct computations (distinct value fingerprints, so they don't merge into
    // one family), each duplicated across two directories → four families.
    let logics = [
        ("sq", "def f(items):\n    a = 0\n    for x in items:\n        if x > 0:\n            a = a + x * x\n    return a\n"),
        ("prod", "def f(items):\n    a = 1\n    for x in items:\n        a = a * x\n    return a\n"),
        ("cnt", "def f(items):\n    a = 0\n    for x in items:\n        if x < 0:\n            a = a + 1\n    return a\n"),
        ("join", "def f(items):\n    a = ''\n    for x in items:\n        a = a + x + ','\n    return a\n"),
    ];
    for sub in ["x", "y"] {
        for (name, src) in logics {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("{name}.py")), src).unwrap();
        }
    }
    dir
}

#[test]
fn sarif_records_and_notes_top_truncation() {
    let dir = make_multi_family_project("sarif_trunc");

    // --top 1: only one family is emitted, but the run must record the true total and
    // carry an explicit truncation note pointing at --top 0.
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "sarif",
        "--top",
        "1",
    ]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("SARIF must be valid JSON");
    let props = &v["runs"][0]["properties"];
    let total = props["total_families"].as_u64().expect("total_families");
    let shown = props["shown_families"].as_u64().expect("shown_families");
    assert!(total >= 2, "fixture should yield multiple families: {out}");
    assert_eq!(shown, 1, "--top 1 shows exactly one family: {out}");
    assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 1);
    let note = v["runs"][0]["invocations"][0]["toolExecutionNotifications"][0].clone();
    assert_eq!(note["level"], "note", "truncation note present: {out}");
    assert!(
        note["message"]["text"]
            .as_str()
            .unwrap()
            .contains("--top 0"),
        "note points the reader at --top 0: {out}"
    );

    // --top 0: the whole set is emitted, so there is no truncation note.
    let full = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-size",
        "12",
        "--format",
        "sarif",
        "--top",
        "0",
    ]);
    let fv: serde_json::Value = serde_json::from_str(&full).expect("SARIF must be valid JSON");
    let fp = &fv["runs"][0]["properties"];
    assert_eq!(
        fp["shown_families"], fp["total_families"],
        "--top 0 emits every family: {full}"
    );
    assert!(
        fv["runs"][0].get("invocations").is_none(),
        "no truncation note with --top 0: {full}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn diff_shows_the_differing_line() {
    // Two near-identical functions differing in one line -> --show diff marks it +/-.
    let dir = std::env::temp_dir().join(format!("nose_diff_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    // The two sum-loops differ only in a literal (the per-element weight), so they
    // are a clear near-duplicate family. (A `+`-loop vs a `*`-loop is deliberately
    // *not* used here: the value graph now treats those as distinct reductions — see
    // §AH — so they are no longer a single family.)
    fs::write(
        dir.join("a/f.py"),
        "def f(items):\n    t = 0\n    for x in items:\n        t = t + x * 2\n    return t\n",
    )
    .unwrap();
    fs::write(
        dir.join("b/f.py"),
        "def g(items):\n    t = 0\n    for x in items:\n        t = t + x * 3\n    return t\n",
    )
    .unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near:0.5",
        "--min-size",
        "10",
        "--show",
        "diff",
    ]);
    assert!(out.contains("diff  "), "should print a diff header: {out}");
    assert!(
        out.contains("-         t = t + x * 2") && out.contains("+         t = t + x * 3"),
        "diff should mark the changed line: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn version_flag_works() {
    let out = run(&["--version"]);
    assert!(out.starts_with("nose "), "version line: {out}");
}

#[test]
fn capabilities_command_emits_machine_readable_contract() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["tool"]["name"], "nose");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        json["platform"]["os"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "platform.os should be non-empty: {out}"
    );
    assert!(
        json["platform"]["arch"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "platform.arch should be non-empty: {out}"
    );
    assert_eq!(json["interfaces"]["capabilities_json"], true);
    assert_eq!(json["interfaces"]["version_json"], false);
    assert_eq!(json["interfaces"]["doctor_json"], false);
}

#[test]
fn capabilities_command_lists_stable_commands_and_schemas() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(
        json_array_strings(&json["commands"], "stable"),
        vec![
            "capabilities",
            "il",
            "review",
            "scan",
            "semantic-pack",
            "stats"
        ]
    );
    assert_eq!(json["schemas"]["capabilities"][0], 1);
    assert_eq!(json["schemas"]["scan_json"][0], 1);
    assert_eq!(
        json["schemas"]["semantic_packs"][0],
        "nose.semantic-pack.v0"
    );
    assert_eq!(json["schemas"]["semantic_pack_conformance"][0], 1);
}

#[test]
fn capabilities_command_reports_scan_surface() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(
        json_array_strings(&json["scan"], "modes"),
        vec!["syntax", "semantic", "near"]
    );
    assert_eq!(
        json_array_strings(&json["scan"], "default_modes"),
        vec!["syntax", "semantic"]
    );
    assert_eq!(
        json_array_strings(&json["scan"], "output_formats"),
        vec!["human", "json", "markdown", "sarif"]
    );
    assert_eq!(
        json_array_strings(&json["scan"], "sort_keys"),
        vec!["extractability", "value", "sites", "hazard"]
    );
    assert_eq!(json["scan"]["capabilities"]["baseline"], true);
    assert_eq!(json["scan"]["capabilities"]["semantic_pack_loading"], true);
    assert_eq!(json["scan"]["capabilities"]["structured_ignores"], true);
}

#[test]
fn capabilities_command_reports_semantic_pack_il_and_stats_surfaces() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(
        json["semantic_packs"]["api_versions"][0],
        "nose.semantic-pack.v0"
    );
    assert_eq!(
        json["semantic_packs"]["external_pack_influence"],
        "metadata-only"
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "conformance"),
        vec!["local-manifest-file", "local-manifest-directory"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "conformance_output_formats"),
        vec!["human", "json"]
    );
    assert_eq!(
        json["semantic_packs"]["external_packs_enabled_by_default"],
        false
    );
    assert_eq!(
        json_array_strings(&json["il"], "output_formats"),
        vec!["sexpr", "json"]
    );
    assert_eq!(
        json_array_strings(&json["stats"], "output_formats"),
        vec!["human", "json"]
    );
}

#[test]
fn broken_pipe_exits_cleanly() {
    // `nose scan … | head` (or quitting a pager) closes the read end of stdout while
    // nose is still writing. That write then fails with `BrokenPipe`, which `println!`
    // turns into a panic — the `failed printing to stdout` crash users hit. The Unix
    // convention is to stop quietly; main()'s panic hook must catch this and exit 0.
    // We reproduce it by closing the pipe's read end outright, so every write to the
    // now-readerless pipe gets EPIPE.
    let dir = std::env::temp_dir().join(format!("nose_pipe_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // A handful of large, distinct clone families -> plenty of `--show diff` output, so the
    // child keeps writing past any buffering and reliably hits the dead pipe.
    for i in 0..40 {
        let body = |delta: &str| {
            let mut s = format!("def fam{i}(xs):\n    acc{i} = 0\n");
            for k in 0..120 {
                s.push_str(&format!("    acc{i} = acc{i} + xs[{k}] * {i}\n"));
            }
            s.push_str(&format!("    acc{i} = acc{i} {delta}\n    return acc{i}\n"));
            s
        };
        for (sub, delta) in [("a", "+ 1"), ("b", "- 1")] {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("f{i}.py")), body(delta)).unwrap();
        }
    }

    let mut child = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "near:0.5",
            "--show",
            "diff",
            "--top",
            "40",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nose");
    // Close the read end immediately — the moral equivalent of `| head` exiting early.
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait for nose");
    let _ = fs::remove_dir_all(&dir);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "broken pipe must not panic, got stderr:\n{stderr}"
    );
    assert!(
        out.status.success(),
        "broken pipe should exit cleanly (0), got {:?}\nstderr:\n{stderr}",
        out.status
    );
}

#[test]
fn deeply_nested_file_does_not_overflow() {
    // A pathologically deep expression (minified bundle / generated code) must not
    // crash the recursive lowering on rayon's small worker stack. Regression for the
    // stack-overflow on real repos (prettier test fixtures); main() sizes the pool's
    // stack so this completes instead of aborting (`run` asserts a clean exit).
    let dir = std::env::temp_dir().join(format!("nose_deep_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let depth = 40_000;
    let body = format!("const x = {}1{};\n", "[".repeat(depth), "]".repeat(depth));
    fs::write(dir.join("deep.js"), body).unwrap();
    let _ = run(&["scan", dir.to_str().unwrap()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn recursive_hof_callback_fragment_does_not_overflow() {
    // Regression for the rxjs scanner abort: when extracting sub-function units inside a
    // recursive helper, the value graph must not register the enclosing function as a pure inline
    // target and inline the helper through its own reduce callback forever.
    let dir = std::env::temp_dir().join(format!("nose_recursive_hof_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("recursive.ts"),
        "export function recInLambda(xs: any[]): number {\n  return xs.reduce((acc, x) => recInLambda(x), 0);\n}\n",
    )
    .unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let json = scan_json(&out);
    assert_eq!(json["scope"]["files"], 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn proposal_shows_shared_skeleton_and_parameters() {
    // Two near-identical functions differing in one line → an extraction proposal:
    // the shared skeleton plus a ⟨param⟩ for the varying spot.
    let dir = std::env::temp_dir().join(format!("nose_prop_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    let mk = |op: &str| {
        format!("def f(xs):\n    t = 0\n    for x in xs:\n        if x > 0:\n            t = t {op} x\n    return t\n")
    };
    fs::write(dir.join("a/m.py"), mk("+")).unwrap();
    fs::write(dir.join("b/m.py"), mk("*")).unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near:0.5",
        "--show",
        "proposal",
        "--min-size",
        "12",
    ]);
    assert!(out.contains("proposal"), "should print a proposal: {out}");
    assert!(
        out.contains("parameter(s) vary"),
        "should report parameters: {out}"
    );
    assert!(
        out.contains("⟨param 1⟩"),
        "should show a parameter placeholder: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sort_keys_label_the_ranking_and_reject_garbage() {
    let dir = make_project("sort");
    let p = dir.to_str().unwrap();
    // Default ranking is extractability — the header says so, in plain language.
    let def = run(&["scan", p, "--min-size", "12"]);
    assert!(
        def.contains("ranked by extractability"),
        "default header names the ranking: {def}"
    );
    // --sort hazard is available and switches the header.
    let byhaz = run(&["scan", p, "--min-size", "12", "--sort", "hazard"]);
    assert!(
        byhaz.contains("ranked by divergent-edit hazard"),
        "--sort hazard names the ranking: {byhaz}"
    );
    // Families are described in plain language (copies + removable lines), not a
    // wall of internal metrics.
    assert!(
        def.contains("copies") && def.contains("lines removable"),
        "family summary is plain-language: {def}"
    );
    // --sort value switches the header.
    let byval = run(&["scan", p, "--min-size", "12", "--sort", "value"]);
    assert!(
        byval.contains("raw duplicated volume"),
        "--sort value names the ranking: {byval}"
    );
    // An unknown sort key is rejected by clap.
    let bad = Command::new(bin())
        .args(["scan", p, "--sort", "bogus"])
        .output()
        .expect("run nose");
    assert!(!bad.status.success(), "unknown --sort value is rejected");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_reports_what_it_scanned() {
    let dir = make_project("scope");
    let p = dir.to_str().unwrap();
    // The scope line states the file count and per-language breakdown (make_project
    // writes four Python files), so a `.gitignore`-pruned scope is visible, not silent.
    let out = run(&["scan", p, "--min-size", "12"]);
    assert!(
        out.contains("scanned 4 files") && out.contains("python 4"),
        "header reports scanned count and languages: {out}"
    );
    // The scope line must not corrupt machine-readable output.
    let json = run(&["scan", p, "--min-size", "12", "--format", "json"]);
    let report = scan_json(&json);
    assert!(
        json.trim_start().starts_with('{') && !json.contains("scanned"),
        "json output stays a pure object without human text: {json}"
    );
    assert_eq!(report["scope"]["files"], 4);
    assert_eq!(report["scope"]["languages"][0]["language"], "python");
    assert_eq!(report["scope"]["languages"][0]["files"], 4);
    let _ = fs::remove_dir_all(&dir);
}

/// A determinism guard with teeth: the toy `make_project` is too small to exercise the
/// order-sensitive paths (shared-line IDF summation, RANSAC offset ties, cross-family
/// ordering) where byte-identical output has actually broken before. Generate a project
/// with many same-language near-duplicate functions — several clone families whose copies
/// differ in a few lines — and assert the JSON report is identical across a range of
/// thread counts (each a distinct process, so this also covers `HashMap` seed variation).
#[test]
fn output_is_byte_identical_across_thread_counts_on_a_rich_project() {
    let dir = std::env::temp_dir().join(format!("nose_determinism_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // 8 families × 5 near-duplicate copies. Each copy renames variables and tweaks a
    // couple of lines, so families form, RANSAC alignment runs, and the honest
    // shared-line counter compares several members.
    for fam in 0..8 {
        for copy in 0..5 {
            let v = format!("v{copy}");
            let src = format!(
                "def family{fam}_{copy}(items):\n    \
                 {v}_total = {copy}\n    \
                 {v}_seen = []\n    \
                 for {v}_x in items:\n        \
                 if {v}_x > {fam}:\n            \
                 {v}_total = {v}_total + {v}_x * {fam}\n            \
                 {v}_seen.append({v}_x)\n        \
                 else:\n            \
                 {v}_total = {v}_total - {copy}\n    \
                 return ({v}_total, {v}_seen)\n"
            );
            fs::write(dir.join(format!("f{fam}_{copy}.py")), src).unwrap();
        }
    }
    let p = dir.to_str().unwrap();
    let run_threads = |n: &str| {
        let out = Command::new(bin())
            .args([
                "scan",
                p,
                "--mode",
                "near:0.5",
                "--min-size",
                "12",
                "--format",
                "json",
                "--top",
                "1000",
            ])
            .env("RAYON_NUM_THREADS", n)
            .output()
            .expect("run nose");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    let baseline = run_threads("1");
    assert!(
        baseline.contains("\"members\""),
        "the fixture forms families: {baseline}"
    );
    for n in ["2", "3", "4", "8"] {
        assert_eq!(
            run_threads(n),
            baseline,
            "{n}-thread output must be byte-identical to the 1-thread run"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn top_zero_shows_all_families() {
    // `--top 0` is documented as "no limit" (docs/usage.md). It must return every
    // family, not an empty set. Regression: `.take(0)` used to silently drop all.
    let dir = std::env::temp_dir().join(format!("nose_top0_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // Build several *structurally distinct* clone families so "all" is clearly more
    // than "top 1". nose finds semantic (Type-4) clones, so each family must have a
    // different shape (statement count / control flow) or they collapse into one.
    let bodies = [
        "def f0(items):\n    acc = 0\n    for x in items:\n        acc = acc + x\n    return acc\n",
        "def f1(items):\n    acc = 1\n    for x in items:\n        if x > 0:\n            acc = acc * x\n    return acc\n",
        "def f2(s):\n    out = []\n    for c in s:\n        out.append(c)\n        out.append(c)\n    return out\n",
        "def f3(a, b):\n    r = 0\n    while a < b:\n        r = r + a\n        a = a + 1\n    return r\n",
        "def f4(d):\n    keys = []\n    for k in d:\n        if k is not None:\n            keys.append(k)\n    keys.sort()\n    return keys\n",
        "def f5(n):\n    total = 0\n    i = 0\n    while i < n:\n        total = total + i * i\n        i = i + 1\n    return total\n",
    ];
    for (i, body) in bodies.iter().enumerate() {
        for sub in ["a", "b"] {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("f{i}.py")), body).unwrap();
        }
    }
    let p = dir.to_str().unwrap();
    let count = |args: &[&str]| -> usize {
        let out = run(args);
        let v = scan_json(&out);
        scan_families(&v).len()
    };

    let all = count(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let one = count(&[
        "scan",
        p,
        "--min-size",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    assert!(all > 1, "--top 0 must show all families, got {all}");
    assert_eq!(one, 1, "--top 1 must still cap at one family, got {one}");
    assert!(
        all >= 6,
        "--top 0 should include every distinct family (expected >=6, got {all})"
    );
    let _ = fs::remove_dir_all(&dir);
}
