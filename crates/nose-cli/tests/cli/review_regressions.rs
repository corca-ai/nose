use super::*;

const BODY: &str = "def f(x):\n    a = x + 1\n    b = a * 2\n    c = b - 3\n    return c\n";

#[test]
fn recursive_oracle_inputs_report_a_budget_instead_of_overflowing_the_stack() {
    let project = TempProject::new("oracle_call_depth");
    project.write(
        "recursive.py",
        "def fac(n):\n    if n == 0:\n        return 1\n    return n * fac(n - 1)\n",
    );
    let census = project.path().join("census.json");
    let output = Command::new(bin())
        .args([
            "verify",
            project.path().to_str().unwrap(),
            "--exclusion-census",
        ])
        .arg(&census)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let census = fs::read_to_string(census).unwrap();
    assert!(census.contains("budget.interpreter-call-depth"), "{census}");
}

#[test]
fn exact_mode_does_not_spend_budget_on_unequal_value_fingerprints() {
    let project = TempProject::new("exact_candidate_budget");
    for i in 0..256 {
        project.write(
            &format!("f{i}.py"),
            &format!("def value_{i}(x):\n    return x + {i}\n"),
        );
    }
    let cache = project.path().join(".cache");
    let clean = json(query(project.path(), &["--mode", "semantic"]));
    for _ in 0..2 {
        let output = Command::new(bin())
            .args([
                "query",
                project.path().to_str().unwrap(),
                "all",
                "--format",
                "json",
                "--mode",
                "semantic",
                "--min-size",
                "1",
                "--min-lines",
                "1",
                "--cache-dir",
                cache.to_str().unwrap(),
            ])
            .env("NOSE_MAX_CANDIDATE_PAIRS", "1024")
            .output()
            .unwrap();
        assert_eq!(clean, json(output));
    }
}

#[test]
fn long_flat_operator_and_type_chains_remain_analyzable() {
    let project = TempProject::new("long_flat_chains");
    let types = (0..5_000)
        .map(|i| format!("'{i}'"))
        .collect::<Vec<_>>()
        .join(" | ");
    project.write("types.js", &format!("type T = {types};\n"));
    project.write(
        "formula.py",
        &format!("def formula(x):\n    return x{}\n", " + x".repeat(600)),
    );
    let cache = project.path().join(".cache");
    let clean = json(query(project.path(), &[]));
    for _ in 0..2 {
        assert_eq!(
            clean,
            json(query(
                project.path(),
                &["--cache-dir", cache.to_str().unwrap()]
            ))
        );
    }
}

fn query(root: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(bin())
        .args([
            "query",
            root.to_str().unwrap(),
            "all",
            "--format",
            "json",
            "--min-size",
            "1",
            "--min-lines",
            "1",
        ])
        .args(extra)
        .output()
        .unwrap()
}

fn json(output: std::process::Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cache_tracks_frontend_dialect_after_extension_change() {
    let project = TempProject::new("dialect_cache");
    let cache = project.path().join(".cache");
    let source = "function f(x) {\n const a = <number>x;\n const b = a + 1;\n const c = b * 2;\n return c;\n}\n";
    for name in ["a.ts", "b.ts"] {
        project.write(name, source);
    }
    let args = ["--cache-dir", cache.to_str().unwrap(), "--mode", "near:0.5"];
    json(query(project.path(), &args));
    for name in ["a", "b"] {
        fs::rename(
            project.path().join(format!("{name}.ts")),
            project.path().join(format!("{name}.tsx")),
        )
        .unwrap();
    }
    assert_eq!(
        json(query(project.path(), &args)),
        json(query(project.path(), &["--mode", "near:0.5"]))
    );
}

#[test]
fn cache_reads_worktree_despite_git_index_flags() {
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let project = TempProject::new("index_flag_cache");
        project.write("a.py", BODY);
        project.write("b.py", BODY);
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        ] {
            assert!(Command::new("git")
                .current_dir(project.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        }
        let cache = project.path().join(".cache");
        let args = ["--cache-dir", cache.to_str().unwrap()];
        json(query(project.path(), &args));
        assert!(Command::new("git")
            .current_dir(project.path())
            .args(["update-index", flag, "b.py"])
            .status()
            .unwrap()
            .success());
        project.write("b.py", "def different(x):\n    return 999\n");
        assert_eq!(
            json(query(project.path(), &args)),
            json(query(project.path(), &[]))
        );
    }
}

#[test]
fn c_identifiers_do_not_make_a_header_cpp() {
    let project = TempProject::new("c_header_identifier");
    let source = "int namespace(int x) {\n int a = x + 1;\n int b = a * 2;\n int c = b - 3;\n return c;\n}\n";
    for name in ["a.h", "b.h"] {
        project.write(name, source);
    }
    let data = json(query(project.path(), &[]));
    assert!(!data["families"].as_array().unwrap().is_empty());
}

#[test]
fn missing_and_empty_family_ids_fail_with_json() {
    let project = TempProject::new("family_id_error");
    project.write("a.py", BODY);
    project.write("b.py", BODY);
    for term in ["id=doesnotexist", "id="] {
        let output = query(project.path(), &[term]);
        assert!(!output.status.success());
        let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(data["family"].is_null());
        assert!(data["error"]["message"].is_string());
    }
}

#[test]
fn ambiguous_family_prefix_fails_in_every_output_format() {
    let project = TempProject::new("ambiguous_family_id");
    for index in 0..17 {
        let source = BODY.replace("x + 1", &format!("x + {}", index + 10));
        for copy in ["a", "b"] {
            project.write(&format!("{index}_{copy}.py"), &source);
        }
    }
    let data = json(query(project.path(), &["--mode", "syntax"]));
    let mut prefixes = std::collections::BTreeSet::new();
    let prefix = data["families"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|family| {
            let prefix = &family["id"].as_str().unwrap()[..1];
            (!prefixes.insert(prefix)).then_some(prefix)
        })
        .expect("17 distinct families must share a hexadecimal ID prefix");
    let term = format!("id={prefix}");
    for format in ["json", "human", "markdown", "sarif"] {
        let output = Command::new(bin())
            .args([
                "query",
                project.path().to_str().unwrap(),
                "all",
                &term,
                "--format",
                format,
                "--min-size",
                "1",
                "--min-lines",
                "1",
                "--mode",
                "syntax",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("ambiguous family id"));
        if format == "json" {
            let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(data["family"].is_null());
            assert!(data["error"]["message"]
                .as_str()
                .unwrap()
                .contains("ambiguous"));
        }
    }
}

#[test]
fn root_config_reports_effective_cli_overrides() {
    let project = TempProject::new("root_config_report");
    project.write("nose.toml", "[query]\nmin-size = 88\nmin-members = 3\n");
    let data = json(query(project.path(), &["--config-root", "--show-config"]));
    assert!(data["config_file"].as_str().unwrap().ends_with("nose.toml"));
    assert_eq!(data["query"]["min-size"], 1);
    assert_eq!(data["query"]["min-members"], 3);
}

#[test]
fn markdown_cache_hits_and_invalidates_without_changing_output() {
    let project = TempProject::new("markdown_cached");
    let prose = "# Cache contract\n\nEvery changed document is read from disk and compared with the complete source digest before any cached result can be used again.\n";
    project.write("a.md", prose);
    project.write("b.md", prose);
    let cache = project.path().join(".cache");
    let run = || {
        Command::new(bin())
            .args([
                "query",
                project.path().to_str().unwrap(),
                "--format",
                "json",
                "--cache-dir",
                cache.to_str().unwrap(),
            ])
            .env("NOSE_CACHE_STATS", "1")
            .output()
            .unwrap()
    };
    let first = json(run());
    let warm = run();
    assert!(String::from_utf8_lossy(&warm.stderr).contains("[markdown-cache] report_hit=true"));
    assert_eq!(first, json(warm));
    project.write("b.md", "# Other\n\nUnrelated short document.\n");
    let changed = run();
    assert!(String::from_utf8_lossy(&changed.stderr).contains("document_hits=1"));
    let clean = Command::new(bin())
        .args([
            "query",
            project.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(json(changed), json(clean));
}

#[test]
fn candidate_budget_fails_instead_of_truncating_results() {
    let project = TempProject::new("candidate_budget");
    for name in ["a.py", "b.py", "c.py"] {
        project.write(name, BODY);
    }
    let output = Command::new(bin())
        .args([
            "query",
            project.path().to_str().unwrap(),
            "--format",
            "json",
            "--min-size",
            "1",
        ])
        .env("NOSE_MAX_CANDIDATE_PAIRS", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("candidate work exceeds limit"));
    assert!(output.stdout.is_empty());
}

#[test]
fn excluded_source_reasons_survive_cold_and_warm_dashboard() {
    let project = TempProject::new("source_diagnostics");
    project.write("cpp.h", "namespace library { class Thing {}; }\n");
    let cache = project.path().join(".cache");
    let run = |cached: bool| {
        let mut command = Command::new(bin());
        command.args([
            "query",
            project.path().to_str().unwrap(),
            "--format",
            "json",
        ]);
        if cached {
            command.args(["--cache-dir", cache.to_str().unwrap()]);
        }
        json(command.output().unwrap())
    };
    let clean = run(false);
    assert_eq!(
        clean["summary"]["skipped_sources"][0]["reason"],
        "unsupported-cpp-header"
    );
    assert_eq!(clean, run(true));
    assert_eq!(clean, run(true));
}
