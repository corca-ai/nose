use super::*;

const BODY: &str = "def compute(x):\n    a = x * x\n    b = a + 7\n    c = b // 3\n    return c\n";
const OTHER: &str = "def other(y):\n    return y + 999\n";

fn project() -> GitProject {
    let p = GitProject::new("region-movement");
    p.write("a.py", BODY);
    p.write("b.py", BODY);
    p.write("c.py", OTHER);
    p.write("d.py", OTHER);
    p.init();
    p
}
fn report(p: &GitProject) -> serde_json::Value {
    let out = nose_query_in(
        p.path(),
        &[
            "base=HEAD",
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "--format",
            "json",
            "top=0",
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn changed(report: &serde_json::Value) -> &serde_json::Value {
    &report["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["changed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["file"] == "a.py" && s["name"] == "compute")
        })
        .expect("base compute divergence")["changed"][0]
}

#[test]
fn moved_function_exposes_original_byte_match_and_downgrades_wrong_range_alignment() {
    let p = project();
    p.write("a.py", "def replacement(z):\n    return z - 100\n");
    p.write("c.py", &format!("{OTHER}\n{BODY}"));
    let result = report(&p);
    let evidence = &changed(&result)["semantic_change"];
    assert_eq!(evidence["alignment"], "changed-range", "{result}");
    assert_eq!(evidence["status"], "advisory", "{result}");
    let regions = &evidence["region_matches"];
    assert_eq!(regions["schema"], "nose.changed-region-candidates/v1");
    assert_eq!(regions["status"], "unique-content-candidate");
    assert_eq!(regions["candidates"][0]["file"], "c.py");
    assert_eq!(
        regions["base"]["content_digest"],
        regions["candidates"][0]["source"]["content_digest"]
    );
    assert!(result["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["gate"]["fail_default"] == true));
    let human = nose_query_in(
        p.path(),
        &[
            "base=HEAD",
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--min-lines",
            "1",
        ],
    );
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("c.py:4"));
}

#[test]
fn actual_named_edit_keeps_its_semantic_evidence_even_when_the_old_body_was_copied() {
    let p = project();
    p.write("a.py", &BODY.replace("+ 7", "+ 8"));
    p.write("c.py", &format!("{OTHER}\n{BODY}"));
    let result = report(&p);
    let evidence = &changed(&result)["semantic_change"];
    assert_eq!(evidence["alignment"], "exact-span");
    assert_eq!(evidence["status"], "complete");
    assert_eq!(evidence["change_kind"], "replacement");
    assert_eq!(evidence["region_matches"]["candidates"][0]["file"], "c.py");
}

#[test]
fn competing_moved_copies_are_not_resolved_by_file_order() {
    let p = project();
    p.write("a.py", "def replacement(z):\n    return z - 100\n");
    for name in ["c.py", "d.py"] {
        p.write(name, &format!("{OTHER}\n{BODY}"));
    }
    let result = report(&p);
    let regions = &changed(&result)["semantic_change"]["region_matches"];
    assert_eq!(regions["status"], "ambiguous");
    assert_eq!(regions["candidates"].as_array().unwrap().len(), 2);
}

#[test]
fn incomplete_changed_file_coverage_cannot_claim_a_unique_candidate() {
    let p = project();
    for index in 0..65 {
        p.write(&format!("extra{index:02}.py"), OTHER);
    }
    git_in(p.path(), &["add", "-A"]);
    git_in(p.path(), &["commit", "-qm", "extra files"]);
    for index in 0..65 {
        p.write(
            &format!("extra{index:02}.py"),
            &format!("# header\n{OTHER}"),
        );
    }
    p.write("a.py", "def replacement(z):\n    return z - 100\n");
    p.write("c.py", &format!("{OTHER}\n{BODY}"));
    let result = report(&p);
    let regions = &changed(&result)["semantic_change"]["region_matches"];
    assert_eq!(regions["status"], "partial", "{result}");
    assert_eq!(regions["complete"], false);
    assert_eq!(regions["files_examined"], 64);
    assert_eq!(regions["files_in_scope"], 67);
    assert_eq!(regions["candidates"].as_array().unwrap().len(), 1);
}

#[test]
fn candidate_cap_never_exposes_a_truncated_unique_match() {
    let p = project();
    p.write("a.py", "def replacement(z):\n    return z - 100\n");
    p.write("c.py", &format!("{OTHER}\n{}", BODY.repeat(65)));
    let result = report(&p);
    let regions = &changed(&result)["semantic_change"]["region_matches"];
    assert_eq!(regions["status"], "budget-exceeded", "{result}");
    assert_eq!(regions["complete"], false);
    assert_eq!(regions["max_candidates"], 64);
    assert!(regions["candidates"].as_array().unwrap().is_empty());
}

#[test]
fn moved_function_keeps_candidates_when_original_unit_or_file_is_missing() {
    for deleted in [false, true] {
        let p = project();
        if deleted {
            fs::remove_file(p.path().join("a.py")).unwrap();
        } else {
            p.write("a.py", "# compute moved to c.py\n");
        }
        p.write("c.py", &format!("{OTHER}\n{BODY}"));
        let result = report(&p);
        let evidence = &changed(&result)["semantic_change"];
        assert_eq!(evidence["status"], "unavailable");
        let regions = &evidence["region_matches"];
        assert_eq!(regions["status"], "unique-content-candidate", "{result}");
        assert_eq!(regions["candidates"][0]["file"], "c.py");
        assert_eq!(
            regions["base"]["content_digest"],
            regions["candidates"][0]["source"]["content_digest"]
        );
        assert!(result["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["gate"]["fail_default"] == true));
        for target in result["items"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|item| item["targets"].as_array().unwrap())
        {
            if target["changed"]["file"] == "a.py" {
                assert_eq!(
                    target["changed"]["semantic_change"]["region_matches"],
                    *regions
                );
            }
        }
    }
}

#[test]
fn all_candidate_evidence_is_reachable_with_original_analysis_context() {
    let p = project();
    for name in ["e.py", "f.py"] {
        p.write(name, OTHER);
    }
    p.write(
        "review ' config.toml",
        "# explicit config with quoted path\n",
    );
    git_in(p.path(), &["add", "-A"]);
    git_in(p.path(), &["commit", "-qm", "extra candidate locations"]);
    p.write("a.py", "# compute moved\n");
    for name in ["c.py", "d.py", "e.py", "f.py"] {
        p.write(name, &format!("{OTHER}\n{BODY}"));
    }
    let args = [
        "base=HEAD",
        "--root",
        ".",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--config",
        "review ' config.toml",
        "--fail-on",
        "any",
    ];
    // Use the explicit-root form; a leading positional '.' would become a query term.
    let mut command = vec!["query"];
    command.extend(args);
    let human = run_nose_query(p.path(), &command, "base candidates human");
    assert!(!human.status.success(), "original gate should still fail");
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(
        text.contains("4 source-region match(es), 3 shown"),
        "{text}"
    );
    assert!(text.contains("already-projected changed-file units"));
    let next = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("cd ") && line.contains("'json'"))
        .expect("JSON evidence next");
    assert!(
        next.contains("--config")
            && next.contains("--min-size")
            && next.contains("--mode")
            && next.contains("--root")
    );
    assert!(!next.contains("--fail-on"));
    let binary = PathBuf::from(bin());
    let search_path = std::env::join_paths(
        std::iter::once(binary.parent().unwrap().to_path_buf()).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )),
    )
    .unwrap();
    let out = Command::new("sh")
        .current_dir(std::env::temp_dir())
        .env("PATH", search_path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args(["-c", next])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let regions = &changed(&json)["semantic_change"]["region_matches"];
    assert_eq!(regions["candidates"].as_array().unwrap().len(), 4);
    assert_eq!(
        json["summary"]["shown_divergences"],
        json["summary"]["divergences"]
    );
}
